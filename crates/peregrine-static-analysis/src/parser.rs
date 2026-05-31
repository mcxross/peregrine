use globset::{Glob, GlobSet, GlobSetBuilder};
use move_command_line_common::files::FileHash;
use move_compiler::{
    Flags,
    editions::Flavor,
    parser::{
        ast::{
            Definition, Function, FunctionBody_, LeadingNameAccess, LeadingNameAccess_,
            ModuleDefinition, ModuleMember, Visibility,
        },
        syntax::parse_file_string,
    },
    shared::{CompilationEnv, PackageConfig},
};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

use crate::{
    config::AnalysisConfig,
    model::{AnalysisContext, ParsedFunction, ParsedModule, SourceFile, Span},
};

pub fn parse_package(
    package_path: &Path,
    config: AnalysisConfig,
) -> Result<AnalysisContext, String> {
    if !package_path.is_dir() {
        return Err(format!("{} is not a directory", package_path.display()));
    }

    if !package_path.join("Move.toml").is_file() {
        return Err(format!(
            "{} does not contain a Move.toml file",
            package_path.display()
        ));
    }

    let include = build_glob_set(&config.analysis.include)?;
    let exclude = build_glob_set(&config.analysis.exclude)?;
    let mut source_files = Vec::new();
    let mut modules = Vec::new();

    for entry in WalkDir::new(package_path)
        .into_iter()
        .filter_entry(|entry| !is_hidden_build_dir(entry.path(), package_path))
        .filter_map(Result::ok)
    {
        if !entry.file_type().is_file()
            || entry.path().extension().and_then(|ext| ext.to_str()) != Some("move")
        {
            continue;
        }

        let relative = relative_path(package_path, entry.path())?;

        if !include.is_match(&relative) || exclude.is_match(&relative) {
            continue;
        }

        let contents = fs::read_to_string(entry.path())
            .map_err(|error| format!("Could not read {}: {error}", entry.path().display()))?;

        modules.extend(parse_modules(&contents, &relative)?);

        source_files.push(SourceFile {
            path: relative,
            contents,
        });
    }

    modules.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then_with(|| left.name.cmp(&right.name))
    });

    Ok(AnalysisContext {
        package_path: package_path.to_path_buf(),
        source_files,
        modules,
        config,
    })
}

fn build_glob_set(patterns: &[String]) -> Result<GlobSet, String> {
    let mut builder = GlobSetBuilder::new();

    for pattern in patterns {
        let glob = Glob::new(pattern)
            .map_err(|error| format!("Invalid analysis glob pattern `{pattern}`: {error}"))?;
        builder.add(glob);
    }

    builder
        .build()
        .map_err(|error| format!("Could not compile analysis glob patterns: {error}"))
}

fn is_hidden_build_dir(path: &Path, package_path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(package_path) else {
        return false;
    };

    relative.components().any(|component| {
        let name = component.as_os_str().to_string_lossy();
        name == "build" || name == ".move"
    })
}

fn relative_path(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|error| format!("Could not make {} relative: {error}", path.display()))?;

    relative
        .components()
        .map(|component| {
            component
                .as_os_str()
                .to_str()
                .map(str::to_string)
                .ok_or_else(|| format!("{} is not valid UTF-8", path.display()))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join("/"))
}

fn parse_modules(source: &str, file: &str) -> Result<Vec<ParsedModule>, String> {
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
    let file_hash = FileHash::new(source);
    let definitions = parse_file_string(&env, file_hash, source, None).map_err(|diagnostics| {
        format!("Sui Move parser could not parse {file}: {diagnostics:?}")
    })?;
    let mut modules = Vec::new();

    for definition in &definitions {
        collect_definition_modules(definition, source, file, None, &mut modules);
    }

    Ok(modules)
}

fn collect_definition_modules(
    definition: &Definition,
    source: &str,
    file: &str,
    inherited_address: Option<&str>,
    modules: &mut Vec<ParsedModule>,
) {
    match definition {
        Definition::Module(module) => {
            modules.push(convert_module(module, source, file, inherited_address));
        }
        Definition::Address(address) => {
            let address_name = leading_name_access_to_string(&address.addr);
            for module in &address.modules {
                modules.push(convert_module(module, source, file, Some(&address_name)));
            }
        }
    }
}

fn convert_module(
    module: &ModuleDefinition,
    source: &str,
    file: &str,
    inherited_address: Option<&str>,
) -> ParsedModule {
    let name = module.name.0.value.to_string();
    let address = module
        .address
        .as_ref()
        .map(leading_name_access_to_string)
        .or_else(|| inherited_address.map(str::to_string));
    let functions = module
        .members
        .iter()
        .filter_map(|member| match member {
            ModuleMember::Function(function) => {
                Some(convert_function(function, source, file, &name))
            }
            _ => None,
        })
        .collect();

    ParsedModule {
        name,
        address,
        file: file.to_string(),
        functions,
    }
}

fn convert_function(
    function: &Function,
    source: &str,
    file: &str,
    module_name: &str,
) -> ParsedFunction {
    let body = source_for_range(
        source,
        function.loc.start() as usize,
        function.loc.end() as usize,
    )
    .unwrap_or_default();
    let signature = signature_source(function, source);
    let visibility = visibility_name(&function.visibility);
    let is_entry = function.entry.is_some();

    ParsedFunction {
        module_name: module_name.to_string(),
        name: function.name.0.value.to_string(),
        is_transaction_callable: is_entry || visibility == "public",
        visibility,
        is_entry,
        signature,
        body,
        file: file.to_string(),
        span: Some(Span {
            start_line: line_number_at(source, function.loc.start() as usize),
            end_line: line_number_at(source, function.loc.end() as usize),
        }),
        type_parameter_count: function.signature.type_parameters.len() as u32,
    }
}

fn signature_source(function: &Function, source: &str) -> String {
    let start = function.loc.start() as usize;
    let end = match &function.body.value {
        FunctionBody_::Defined(_) => function.body.loc.start() as usize,
        FunctionBody_::Native => function.loc.end() as usize,
    };

    source
        .get(start..end)
        .unwrap_or_default()
        .trim()
        .trim_end_matches('{')
        .trim_end_matches(';')
        .trim()
        .to_string()
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

fn visibility_name(visibility: &Visibility) -> String {
    match visibility {
        Visibility::Public(_) => "public",
        Visibility::Friend(_) => "public(friend)",
        Visibility::Package(_) => "public(package)",
        Visibility::Internal => "private",
    }
    .to_string()
}

fn line_number_at(source: &str, offset: usize) -> usize {
    source
        .as_bytes()
        .iter()
        .take(offset.min(source.len()))
        .filter(|byte| **byte == b'\n')
        .count()
        + 1
}

#[allow(dead_code)]
fn _normalize_path(path: &Path) -> PathBuf {
    path.components().collect()
}
