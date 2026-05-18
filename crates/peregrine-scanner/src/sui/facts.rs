use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use move_binary_format::file_format::{
    Ability, AbilitySet, Bytecode, CompiledModule, DatatypeHandleIndex, FunctionHandleIndex,
    SignatureToken, StructDefinitionIndex, StructFieldInformation, Visibility,
};
use walkdir::WalkDir;

use crate::core::{EvidenceSource, ScannerDiagnostic};

pub const BYTECODE_PROVIDER_ID: &str = "sui.bytecode";

#[derive(Clone, Debug, Default)]
pub struct BytecodePackageFacts {
    pub modules: Vec<BytecodeModuleFact>,
}

impl BytecodePackageFacts {
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    pub fn struct_facts_by_qualified_name(&self) -> BTreeMap<String, BytecodeStructFact> {
        self.modules
            .iter()
            .flat_map(|module| module.structs.iter().cloned())
            .map(|struct_fact| (struct_fact.qualified_name.clone(), struct_fact))
            .collect()
    }

    pub fn function_facts_by_qualified_name(&self) -> BTreeMap<String, BytecodeFunctionFact> {
        self.modules
            .iter()
            .flat_map(|module| module.functions.iter().cloned())
            .map(|function| (function.qualified_name.clone(), function))
            .collect()
    }
}

#[derive(Clone, Debug)]
pub struct BytecodeModuleFact {
    pub address: String,
    pub name: String,
    pub path: PathBuf,
    pub structs: Vec<BytecodeStructFact>,
    pub functions: Vec<BytecodeFunctionFact>,
}

#[derive(Clone, Debug)]
pub struct BytecodeStructFact {
    pub address: String,
    pub module_name: String,
    pub type_name: String,
    pub qualified_name: String,
    pub full_name: String,
    pub abilities: Vec<String>,
    pub fields: Vec<BytecodeFieldFact>,
}

#[derive(Clone, Debug)]
pub struct BytecodeFieldFact {
    pub name: String,
    pub type_name: String,
}

#[derive(Clone, Debug)]
pub struct BytecodeFunctionFact {
    pub address: String,
    pub module_name: String,
    pub function_name: String,
    pub qualified_name: String,
    pub full_name: String,
    pub visibility: String,
    pub is_entry: bool,
    pub is_transaction_callable: bool,
    pub parameter_types: Vec<String>,
    pub return_types: Vec<String>,
    pub calls: Vec<String>,
    pub packs: Vec<String>,
    pub unpacks: Vec<String>,
}

pub fn load_bytecode_package_facts(
    roots: &[PathBuf],
) -> (BytecodePackageFacts, Vec<ScannerDiagnostic>) {
    let mut diagnostics = Vec::new();
    let module_paths = bytecode_module_paths(roots);

    if module_paths.is_empty() {
        diagnostics.push(ScannerDiagnostic::warning(
            BYTECODE_PROVIDER_ID,
            EvidenceSource::Bytecode,
            "no compiled bytecode modules found",
        ));
        return (BytecodePackageFacts::default(), diagnostics);
    }

    let mut modules = Vec::new();
    for path in module_paths {
        let Ok(bytes) = fs::read(&path) else {
            diagnostics.push(ScannerDiagnostic::warning(
                BYTECODE_PROVIDER_ID,
                EvidenceSource::Bytecode,
                format!("could not read bytecode module {}", path.display()),
            ));
            continue;
        };

        let Ok(module) = CompiledModule::deserialize_with_defaults(&bytes) else {
            diagnostics.push(ScannerDiagnostic::warning(
                BYTECODE_PROVIDER_ID,
                EvidenceSource::Bytecode,
                format!("could not deserialize bytecode module {}", path.display()),
            ));
            continue;
        };

        modules.push(module_fact(&path, &module));
    }

    diagnostics.push(ScannerDiagnostic::info(
        BYTECODE_PROVIDER_ID,
        EvidenceSource::Bytecode,
        format!("loaded {} compiled bytecode module(s)", modules.len()),
    ));

    (BytecodePackageFacts { modules }, diagnostics)
}

fn bytecode_module_paths(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut roots = roots
        .iter()
        .flat_map(|root| bytecode_roots(root))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    roots.sort();

    let mut paths = roots
        .into_iter()
        .flat_map(|root| {
            WalkDir::new(root)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file())
                .map(|entry| entry.into_path())
                .filter(|path| {
                    path.extension().and_then(|extension| extension.to_str()) == Some("mv")
                })
                .filter(|path| {
                    !path
                        .components()
                        .any(|component| component.as_os_str().to_str() == Some("dependencies"))
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn bytecode_roots(root: &Path) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if root.file_name().and_then(|name| name.to_str()) == Some("bytecode_modules") {
        roots.push(root.to_path_buf());
    }

    let direct = root.join("bytecode_modules");
    if direct.is_dir() {
        roots.push(direct);
    }

    let build = root.join("build");
    if let Ok(entries) = fs::read_dir(build) {
        for entry in entries.filter_map(Result::ok) {
            let path = entry.path().join("bytecode_modules");
            if path.is_dir() {
                roots.push(path);
            }
        }
    }

    roots
}

fn module_fact(path: &Path, module: &CompiledModule) -> BytecodeModuleFact {
    let self_id = module.self_id();
    let address = self_id.address().short_str_lossless();
    let name = self_id.name().to_string();
    let structs = module
        .struct_defs()
        .iter()
        .enumerate()
        .filter_map(|(index, definition)| {
            let index = StructDefinitionIndex(index as u16);
            let handle = module.datatype_handle_at(definition.struct_handle);
            let type_name = module.identifier_at(handle.name).to_string();
            let full_name = datatype_label(module, definition.struct_handle);
            let parts = full_name.split("::").collect::<Vec<_>>();
            let module_name = parts
                .get(parts.len().saturating_sub(2))
                .copied()
                .unwrap_or(&name)
                .to_string();
            let qualified_name = format!("{module_name}::{type_name}");
            Some(BytecodeStructFact {
                address: address.clone(),
                module_name,
                type_name,
                qualified_name,
                full_name,
                abilities: ability_labels(handle.abilities),
                fields: struct_fields(module, index),
            })
        })
        .collect();
    let functions = module
        .function_defs()
        .iter()
        .map(|definition| function_fact(module, definition))
        .collect();

    BytecodeModuleFact {
        address,
        name,
        path: path.to_path_buf(),
        structs,
        functions,
    }
}

fn struct_fields(module: &CompiledModule, index: StructDefinitionIndex) -> Vec<BytecodeFieldFact> {
    let definition = module.struct_def_at(index);

    match &definition.field_information {
        StructFieldInformation::Native => Vec::new(),
        StructFieldInformation::Declared(fields) => fields
            .iter()
            .map(|field| BytecodeFieldFact {
                name: module.identifier_at(field.name).to_string(),
                type_name: signature_token_label(module, &field.signature.0),
            })
            .collect(),
    }
}

fn function_fact(
    module: &CompiledModule,
    definition: &move_binary_format::file_format::FunctionDefinition,
) -> BytecodeFunctionFact {
    let handle = module.function_handle_at(definition.function);
    let module_id = module.module_id_for_handle(module.module_handle_at(handle.module));
    let address = module_id.address().short_str_lossless();
    let module_name = module_id.name().to_string();
    let function_name = module.identifier_at(handle.name).to_string();
    let parameter_types = module
        .signature_at(handle.parameters)
        .0
        .iter()
        .map(|token| signature_token_label(module, token))
        .collect::<Vec<_>>();
    let return_types = module
        .signature_at(handle.return_)
        .0
        .iter()
        .map(|token| signature_token_label(module, token))
        .collect::<Vec<_>>();
    let (calls, packs, unpacks) = definition
        .code
        .as_ref()
        .map(|code| bytecode_operations(module, &code.code))
        .unwrap_or_default();

    BytecodeFunctionFact {
        address: address.clone(),
        module_name: module_name.clone(),
        function_name: function_name.clone(),
        qualified_name: format!("{module_name}::{function_name}"),
        full_name: format!("{address}::{module_name}::{function_name}"),
        visibility: visibility_label(definition.visibility).to_string(),
        is_entry: definition.is_entry,
        is_transaction_callable: definition.is_entry || definition.visibility == Visibility::Public,
        parameter_types,
        return_types,
        calls,
        packs,
        unpacks,
    }
}

fn bytecode_operations(
    module: &CompiledModule,
    code: &[Bytecode],
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut calls = Vec::new();
    let mut packs = Vec::new();
    let mut unpacks = Vec::new();

    for bytecode in code {
        match bytecode {
            Bytecode::Call(handle) => {
                if let Some(target) = call_target(module, *handle) {
                    calls.push(target);
                }
            }
            Bytecode::CallGeneric(index) => {
                let instantiation = module.function_instantiation_at(*index);
                if let Some(target) = call_target(module, instantiation.handle) {
                    calls.push(target);
                }
            }
            Bytecode::Pack(index) => {
                if let Some(target) = struct_name(module, *index) {
                    packs.push(target);
                }
            }
            Bytecode::PackGeneric(index) => {
                let instantiation = module.struct_instantiation_at(*index);
                if let Some(target) = struct_name(module, instantiation.def) {
                    packs.push(target);
                }
            }
            Bytecode::Unpack(index) => {
                if let Some(target) = struct_name(module, *index) {
                    unpacks.push(target);
                }
            }
            Bytecode::UnpackGeneric(index) => {
                let instantiation = module.struct_instantiation_at(*index);
                if let Some(target) = struct_name(module, instantiation.def) {
                    unpacks.push(target);
                }
            }
            _ => {}
        }
    }

    calls.sort();
    calls.dedup();
    packs.sort();
    packs.dedup();
    unpacks.sort();
    unpacks.dedup();
    (calls, packs, unpacks)
}

fn call_target(module: &CompiledModule, handle_index: FunctionHandleIndex) -> Option<String> {
    let handle = module.function_handle_at(handle_index);
    let module_id = module.module_id_for_handle(module.module_handle_at(handle.module));
    Some(format!(
        "{}::{}::{}",
        module_id.address().short_str_lossless(),
        module_id.name(),
        module.identifier_at(handle.name)
    ))
}

fn struct_name(module: &CompiledModule, index: StructDefinitionIndex) -> Option<String> {
    let definition = module.struct_def_at(index);
    Some(datatype_label(module, definition.struct_handle))
}

fn signature_token_label(module: &CompiledModule, token: &SignatureToken) -> String {
    match token {
        SignatureToken::Bool => "bool".to_string(),
        SignatureToken::U8 => "u8".to_string(),
        SignatureToken::U16 => "u16".to_string(),
        SignatureToken::U32 => "u32".to_string(),
        SignatureToken::U64 => "u64".to_string(),
        SignatureToken::U128 => "u128".to_string(),
        SignatureToken::U256 => "u256".to_string(),
        SignatureToken::Address => "address".to_string(),
        SignatureToken::Signer => "signer".to_string(),
        SignatureToken::Vector(inner) => {
            format!("vector<{}>", signature_token_label(module, inner))
        }
        SignatureToken::Reference(inner) => format!("&{}", signature_token_label(module, inner)),
        SignatureToken::MutableReference(inner) => {
            format!("&mut {}", signature_token_label(module, inner))
        }
        SignatureToken::TypeParameter(index) => format!("T{index}"),
        SignatureToken::Datatype(index) => datatype_label(module, *index),
        SignatureToken::DatatypeInstantiation(instantiation) => {
            let (index, arguments) = &**instantiation;
            let arguments = arguments
                .iter()
                .map(|argument| signature_token_label(module, argument))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}<{arguments}>", datatype_label(module, *index))
        }
    }
}

fn datatype_label(module: &CompiledModule, index: DatatypeHandleIndex) -> String {
    let handle = module.datatype_handle_at(index);
    let module_id = module.module_id_for_handle(module.module_handle_at(handle.module));
    format!(
        "{}::{}::{}",
        module_id.address().short_str_lossless(),
        module_id.name(),
        module.identifier_at(handle.name)
    )
}

fn ability_labels(abilities: AbilitySet) -> Vec<String> {
    let mut labels = Vec::new();

    for (ability, label) in [
        (Ability::Copy, "copy"),
        (Ability::Drop, "drop"),
        (Ability::Store, "store"),
        (Ability::Key, "key"),
    ] {
        if abilities.has_ability(ability) {
            labels.push(label.to_string());
        }
    }

    labels
}

fn visibility_label(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Private => "private",
        Visibility::Public => "public",
        Visibility::Friend => "public(friend)",
    }
}
