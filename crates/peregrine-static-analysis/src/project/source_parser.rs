use super::{
    relative_path, MoveFunctionSignature, MoveModule, MoveStructField, MoveStructSignature,
};
use move_command_line_common::files::FileHash;
use move_compiler::{
    editions::Flavor,
    parser::{
        ast::{
            Ability_, Definition, Function, FunctionBody_, LeadingNameAccess, LeadingNameAccess_,
            ModuleDefinition, ModuleMember, StructDefinition, StructFields, Type, Visibility,
        },
        syntax::parse_file_string,
    },
    shared::{CompilationEnv, PackageConfig},
    Flags,
};
use std::{collections::BTreeMap, fs, path::Path};

pub(crate) fn discover_modules(root: &Path, package_root: &Path) -> Vec<MoveModule> {
    let sources = package_root.join("sources");
    let mut modules = Vec::new();

    collect_move_modules(root, &sources, &mut modules);
    modules
}

fn collect_move_modules(root: &Path, directory: &Path, modules: &mut Vec<MoveModule>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_move_modules(root, &path, modules);
            continue;
        }

        if !file_type.is_file() || !is_move_file(&path) {
            continue;
        }

        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };

        modules.extend(parse_module_declarations(&source, root, &path));
    }
}

pub(crate) fn parse_module_declarations(source: &str, root: &Path, path: &Path) -> Vec<MoveModule> {
    let package_config = PackageConfig {
        flavor: Flavor::Sui,
        ..PackageConfig::default()
    };
    let env = CompilationEnv::new(
        Flags::empty().set_silence_warnings(true),
        Vec::new(),
        Vec::new(),
        None,
        BTreeMap::new(),
        Some(package_config),
        None,
    );
    let Ok(definitions) = parse_file_string(&env, FileHash::new(source), source, None) else {
        return Vec::new();
    };
    let Some(file_path) = relative_path(root, path) else {
        return Vec::new();
    };
    let mut modules = Vec::new();

    for definition in &definitions {
        collect_ast_modules(definition, source, &file_path, None, &mut modules);
    }

    modules
}

fn collect_ast_modules(
    definition: &Definition,
    source: &str,
    file_path: &str,
    inherited_address: Option<&str>,
    modules: &mut Vec<MoveModule>,
) {
    match definition {
        Definition::Module(module) => {
            modules.push(convert_ast_module(
                module,
                source,
                file_path,
                inherited_address,
            ));
        }
        Definition::Address(address) => {
            let address_name = leading_name_access_to_string(&address.addr);

            for module in &address.modules {
                modules.push(convert_ast_module(
                    module,
                    source,
                    file_path,
                    Some(&address_name),
                ));
            }
        }
    }
}

fn convert_ast_module(
    module: &ModuleDefinition,
    source: &str,
    file_path: &str,
    inherited_address: Option<&str>,
) -> MoveModule {
    let name = module.name.0.value.to_string();
    let address = module
        .address
        .as_ref()
        .map(leading_name_access_to_string)
        .or_else(|| inherited_address.map(str::to_string));
    let structs = module
        .members
        .iter()
        .filter_map(|member| match member {
            ModuleMember::Struct(move_struct) => Some(convert_ast_struct(move_struct, source)),
            _ => None,
        })
        .collect();
    let functions = module
        .members
        .iter()
        .filter_map(|member| match member {
            ModuleMember::Function(function) => Some(convert_ast_function(function, source)),
            _ => None,
        })
        .collect();

    MoveModule {
        name,
        address,
        file_path: file_path.to_string(),
        structs,
        functions,
    }
}

fn convert_ast_struct(move_struct: &StructDefinition, source: &str) -> MoveStructSignature {
    MoveStructSignature {
        name: move_struct.name.0.value.to_string(),
        abilities: move_struct
            .abilities
            .iter()
            .map(|ability| ability_name(&ability.value).to_string())
            .collect(),
        fields: ast_struct_fields(&move_struct.fields, source),
        signature: ast_struct_signature(move_struct, source),
    }
}

fn ast_struct_fields(fields: &StructFields, source: &str) -> Vec<MoveStructField> {
    match fields {
        StructFields::Named(fields) => fields
            .iter()
            .map(|(_, field, field_type)| MoveStructField {
                name: field.0.value.to_string(),
                type_name: ast_type_source(field_type, source),
            })
            .collect(),
        StructFields::Positional(fields) => fields
            .iter()
            .enumerate()
            .map(|(index, (_, field_type))| MoveStructField {
                name: index.to_string(),
                type_name: ast_type_source(field_type, source),
            })
            .collect(),
        StructFields::Native(_) => Vec::new(),
    }
}

fn ast_type_source(field_type: &Type, source: &str) -> String {
    source_for_range(
        source,
        field_type.loc.start() as usize,
        field_type.loc.end() as usize,
    )
    .unwrap_or_default()
    .trim()
    .to_string()
}

fn ast_struct_signature(move_struct: &StructDefinition, source: &str) -> String {
    let full = source_for_range(
        source,
        move_struct.loc.start() as usize,
        move_struct.loc.end() as usize,
    )
    .unwrap_or_default();

    full.split('{')
        .next()
        .unwrap_or(&full)
        .split(';')
        .next()
        .unwrap_or(&full)
        .trim()
        .to_string()
}

fn convert_ast_function(function: &Function, source: &str) -> MoveFunctionSignature {
    let visibility = function_visibility_name(&function.visibility);
    let is_entry = function.entry.is_some();

    MoveFunctionSignature {
        name: function.name.0.value.to_string(),
        is_transaction_callable: is_entry || visibility == "public",
        visibility,
        is_entry,
        signature: ast_function_signature(function, source),
        body: ast_function_body(function, source),
    }
}

fn ast_function_signature(function: &Function, source: &str) -> String {
    let start = function.loc.start() as usize;
    let end = match &function.body.value {
        FunctionBody_::Defined(_) => function.body.loc.start() as usize,
        FunctionBody_::Native => function.loc.end() as usize,
    };

    source_for_range(source, start, end)
        .unwrap_or_default()
        .trim()
        .trim_end_matches('{')
        .trim_end_matches(';')
        .trim()
        .to_string()
}

fn ast_function_body(function: &Function, source: &str) -> Option<String> {
    match &function.body.value {
        FunctionBody_::Defined(_) => source_for_range(
            source,
            function.loc.start() as usize,
            function.loc.end() as usize,
        ),
        FunctionBody_::Native => None,
    }
}

fn source_for_range(source: &str, start: usize, end: usize) -> Option<String> {
    if start <= end && end <= source.len() {
        Some(source[start..end].to_string())
    } else {
        None
    }
}

fn leading_name_access_to_string(access: &LeadingNameAccess) -> String {
    match &access.value {
        LeadingNameAccess_::AnonymousAddress(address) => format!("{address}"),
        LeadingNameAccess_::GlobalAddress(name) | LeadingNameAccess_::Name(name) => {
            name.value.to_string()
        }
    }
}

fn function_visibility_name(visibility: &Visibility) -> String {
    match visibility {
        Visibility::Public(_) => "public",
        Visibility::Friend(_) => "public(friend)",
        Visibility::Package(_) => "public(package)",
        Visibility::Internal => "private",
    }
    .to_string()
}

fn ability_name(ability: &Ability_) -> &'static str {
    match ability {
        Ability_::Copy => "copy",
        Ability_::Drop => "drop",
        Ability_::Store => "store",
        Ability_::Key => "key",
    }
}

fn is_move_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("move"))
}
