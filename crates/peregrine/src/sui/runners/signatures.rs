use crate::{
    output::{elapsed_ms, CliDiagnostic, CliDiagnosticSeverity, CliStatus, CliStep, EXIT_SUCCESS},
    sui::{args::SignaturesArgs, project::CliContext},
};
use peregrine_move_model::{MoveFunctionSignature, MoveModule};
use peregrine_static_analysis::{discover_move_project_fast, MovePackage};
use serde_json::{json, Value};
use std::{
    collections::BTreeMap,
    ffi::OsStr,
    path::{Component, Path},
    time::Instant,
};

const RESET: &str = "\x1b[0m";
const DIM: &str = "\x1b[90m";
const PACKAGE: &str = "\x1b[95m";
const MODULE: &str = "\x1b[96m";
const FUNCTION: &str = "\x1b[92m";
const TYPE: &str = "\x1b[93m";
const RETURN: &str = "\x1b[95m";

pub fn run_signatures(context: &CliContext, args: &SignaturesArgs) -> CliStep {
    let started_at = Instant::now();
    let package = match selected_package(context) {
        Ok(package) => package,
        Err(error) => return CliStep::failed("signatures", started_at, error),
    };
    let modules = match selected_modules(context, &package, args) {
        Ok(modules) => modules,
        Err(error) => return CliStep::failed("signatures", started_at, error),
    };
    let function_count = modules
        .iter()
        .map(|module| module.functions.len())
        .sum::<usize>();
    let stdout = render_signature_tree(&package, &modules);

    CliStep {
        name: "signatures".to_string(),
        status: CliStatus::Passed,
        duration_ms: elapsed_ms(started_at),
        exit_code: EXIT_SUCCESS,
        command: Some(display_command(args)),
        diagnostics: Vec::new(),
        metadata: BTreeMap::from([
            ("package".to_string(), json!(package.name)),
            ("moduleCount".to_string(), json!(modules.len())),
            ("functionCount".to_string(), json!(function_count)),
        ]),
        stdout,
        stderr: String::new(),
        details: json!({
            "package": package.name,
            "modules": modules.iter().map(module_details).collect::<Vec<_>>(),
        }),
    }
}

fn selected_package(context: &CliContext) -> Result<MovePackage, CliDiagnostic> {
    let project = discover_move_project_fast(&context.project_root);
    project
        .packages
        .into_iter()
        .find(|package| normalize_path_label(Path::new(&package.path)) == context.package_path)
        .ok_or_else(|| {
            CliDiagnostic::error(
                "signatures",
                format!(
                    "Could not discover package `{}` under {}.",
                    context.package_path,
                    context.project_root.display()
                ),
            )
        })
}

fn selected_modules(
    context: &CliContext,
    package: &MovePackage,
    args: &SignaturesArgs,
) -> Result<Vec<MoveModule>, CliDiagnostic> {
    let requested_modules = args
        .modules
        .iter()
        .map(|module| module.trim())
        .filter(|module| !module.is_empty())
        .collect::<Vec<_>>();
    let requested_file = args
        .file
        .as_deref()
        .map(|file| normalize_requested_file(context, file))
        .filter(|file| !file.is_empty());

    let mut modules = package
        .modules
        .iter()
        .filter(|module| {
            requested_file.as_deref().map_or(true, |file| {
                normalize_path_label(Path::new(&module.file_path)) == file
            })
        })
        .filter(|module| {
            requested_modules.is_empty()
                || requested_modules
                    .iter()
                    .any(|requested| module_matches(requested, module))
        })
        .cloned()
        .collect::<Vec<_>>();

    modules.sort_by(|left, right| left.name.cmp(&right.name));
    for module in &mut modules {
        module
            .functions
            .sort_by(|left, right| left.name.cmp(&right.name));
    }

    if modules.is_empty() {
        return Err(CliDiagnostic {
            severity: CliDiagnosticSeverity::Error,
            source: "signatures".to_string(),
            code: Some("NoSignatureTargets".to_string()),
            message: "No Move modules matched the requested signature target.".to_string(),
            file: requested_file,
            span: None,
        });
    }

    Ok(modules)
}

fn render_signature_tree(package: &MovePackage, modules: &[MoveModule]) -> String {
    let mut lines = vec![format!("{PACKAGE}package{RESET} {}", package.name)];

    for module in modules {
        lines.push(format!(
            "{DIM}|--{RESET} {MODULE}module{RESET} {}",
            module.name
        ));

        for function in &module.functions {
            lines.push(format!(
                "{DIM}|   |--{RESET} {}",
                render_function_signature(function)
            ));
        }
    }

    lines.join("\n")
}

fn render_function_signature(function: &MoveFunctionSignature) -> String {
    let signature = normalized_signature(&function.signature);
    let Some(rest) = signature.strip_prefix("fun ") else {
        return format!("{DIM}fun{RESET} {FUNCTION}{}{RESET}", function.name);
    };
    let name_end = rest
        .char_indices()
        .find_map(|(index, character)| matches!(character, '<' | '(').then_some(index))
        .unwrap_or(rest.len());
    let name = &rest[..name_end];
    let remainder = &rest[name_end..];
    let (middle, return_type) = split_return_type(remainder);

    format!(
        "{DIM}fun{RESET} {FUNCTION}{name}{RESET}{TYPE}{middle}{RESET}{RETURN}{return_type}{RESET}"
    )
}

fn normalized_signature(signature: &str) -> String {
    let compact = signature.split_whitespace().collect::<Vec<_>>().join(" ");
    let signature = compact
        .find("fun ")
        .map(|index| compact[index..].to_string())
        .unwrap_or(compact);

    if has_return_type(&signature) {
        signature
    } else {
        format!("{signature}: ()")
    }
}

fn has_return_type(signature: &str) -> bool {
    match parameter_close_index(signature) {
        Some(index) => signature[index + 1..].trim_start().starts_with(':'),
        None => true,
    }
}

fn split_return_type(remainder: &str) -> (&str, &str) {
    let Some(close_index) = parameter_close_index(remainder) else {
        return (remainder, "");
    };
    let suffix = &remainder[close_index + 1..];

    if suffix.trim_start().starts_with(':') {
        remainder.split_at(close_index + 1)
    } else {
        (remainder, "")
    }
}

fn parameter_close_index(signature: &str) -> Option<usize> {
    let open_index = signature.find('(')?;
    signature[open_index..]
        .find(')')
        .map(|close_index| open_index + close_index)
}

fn module_details(module: &MoveModule) -> Value {
    json!({
        "name": module.name,
        "address": module.address,
        "filePath": module.file_path,
        "functionCount": module.functions.len(),
        "functions": module.functions.iter().map(function_details).collect::<Vec<_>>(),
    })
}

fn function_details(function: &MoveFunctionSignature) -> Value {
    json!({
        "name": function.name,
        "visibility": function.visibility,
        "isEntry": function.is_entry,
        "isTransactionCallable": function.is_transaction_callable,
        "signature": normalized_signature(&function.signature),
    })
}

fn display_command(args: &SignaturesArgs) -> String {
    let mut command = "peregrine signatures".to_string();

    for module in &args.modules {
        command.push_str(&format!(" --module {module}"));
    }

    if let Some(file) = &args.file {
        command.push_str(&format!(" --file {file}"));
    }

    command
}

fn normalize_requested_file(context: &CliContext, file: &str) -> String {
    let path = Path::new(file.trim());

    if path.is_absolute() {
        if let Ok(relative) = path.strip_prefix(&context.project_root) {
            return normalize_path_label(relative);
        }
    }

    normalize_path_label(path)
}

fn normalize_path_label(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();

    if path.as_os_str().is_empty() || path == Path::new(".") {
        return ".".to_string();
    }

    path.components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value),
            Component::CurDir => None,
            _ => Some(component.as_os_str()),
        })
        .map(OsStr::to_string_lossy)
        .collect::<Vec<_>>()
        .join("/")
}

fn module_matches(requested: &str, module: &MoveModule) -> bool {
    if requested == module.name {
        return true;
    }

    module
        .address
        .as_deref()
        .is_some_and(|address| requested == format!("{address}::{}", module.name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signature_display_strips_visibility_and_adds_unit_return() {
        let signature =
            normalized_signature("public entry fun transfer<T>(coin: Coin<T>, recipient: address)");

        assert_eq!(
            signature,
            "fun transfer<T>(coin: Coin<T>, recipient: address): ()"
        );
    }

    #[test]
    fn signature_display_preserves_explicit_return_type() {
        let signature = normalized_signature("public fun balance(account: &Account): u64");

        assert_eq!(signature, "fun balance(account: &Account): u64");
    }

    #[test]
    fn function_signature_renderer_is_colored() {
        let rendered = render_function_signature(&MoveFunctionSignature {
            name: "balance".to_string(),
            visibility: "public".to_string(),
            is_entry: false,
            is_transaction_callable: true,
            signature: "public fun balance(account: &Account): u64".to_string(),
            body: None,
            attributes: Vec::new(),
        });

        assert!(rendered.contains("\x1b[92mbalance\x1b[0m"));
        assert!(rendered.contains("\x1b[95m: u64\x1b[0m"));
    }

    #[test]
    fn function_signature_renderer_colors_implicit_unit_return() {
        let rendered = render_function_signature(&MoveFunctionSignature {
            name: "transfer".to_string(),
            visibility: "public".to_string(),
            is_entry: true,
            is_transaction_callable: true,
            signature: "public entry fun transfer<T>(coin: Coin<T>, recipient: address)"
                .to_string(),
            body: None,
            attributes: Vec::new(),
        });

        assert!(rendered.contains("\x1b[93m<T>(coin: Coin<T>, recipient: address)\x1b[0m"));
        assert!(rendered.contains("\x1b[95m: ()\x1b[0m"));
    }
}
