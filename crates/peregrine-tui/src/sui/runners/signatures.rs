use crate::{
    output::{CliDiagnostic, CliDiagnosticSeverity, CliStatus, CliStep, EXIT_SUCCESS, elapsed_ms},
    sui::{args::SignaturesArgs, project::CliContext},
};
use peregrine_mcp_protocol::SignatureEntry;
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
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
    let (package_name, signatures) = match fetch_signatures(context, args) {
        Ok(result) => result,
        Err(error) => return CliStep::failed("signatures", started_at, error),
    };
    if signatures.is_empty() {
        return CliStep::failed(
            "signatures",
            started_at,
            CliDiagnostic {
                severity: CliDiagnosticSeverity::Error,
                source: "signatures".to_string(),
                code: Some("NoSignatureTargets".to_string()),
                message: "No Move modules matched the requested signature target.".to_string(),
                file: args.file.clone(),
                span: None,
            },
        );
    }

    let module_count = signatures
        .iter()
        .map(|signature| &signature.module_name)
        .collect::<BTreeSet<_>>()
        .len();
    let stdout = render_signature_tree(&package_name, &signatures);

    CliStep {
        name: "signatures".to_string(),
        status: CliStatus::Passed,
        duration_ms: elapsed_ms(started_at),
        exit_code: EXIT_SUCCESS,
        command: Some(display_command(args)),
        diagnostics: Vec::new(),
        metadata: BTreeMap::from([
            ("package".to_string(), json!(package_name)),
            ("moduleCount".to_string(), json!(module_count)),
            ("functionCount".to_string(), json!(signatures.len())),
        ]),
        stdout,
        stderr: String::new(),
        details: json!({
            "package": package_name,
            "modules": module_details(&signatures),
        }),
    }
}

fn fetch_signatures(
    context: &CliContext,
    args: &SignaturesArgs,
) -> Result<(String, Vec<SignatureEntry>), CliDiagnostic> {
    let file = args
        .file
        .as_deref()
        .map(|file| normalize_requested_file(context, file));
    crate::session::fetch_signatures(
        &context.project_root,
        &context.package_path,
        args.modules.clone(),
        file,
    )
    .map(|(package, signatures)| (package.package_name, signatures))
    .map_err(|error| CliDiagnostic::error("mcp:peregrine", error))
}

fn render_signature_tree(package_name: &str, signatures: &[SignatureEntry]) -> String {
    let mut lines = vec![format!("{PACKAGE}package{RESET} {package_name}")];
    let mut current_module = None;

    for signature in signatures {
        if current_module.as_deref() != Some(signature.module_name.as_str()) {
            current_module = Some(signature.module_name.clone());
            lines.push(format!(
                "{DIM}|--{RESET} {MODULE}module{RESET} {}",
                signature.module_name
            ));
        }
        lines.push(format!(
            "{DIM}|   |--{RESET} {}",
            render_function_signature(&signature.function_name, &signature.signature)
        ));
    }

    lines.join("\n")
}

fn render_function_signature(function_name: &str, raw_signature: &str) -> String {
    let signature = normalized_signature(raw_signature);
    let Some(rest) = signature.strip_prefix("fun ") else {
        return format!("{DIM}fun{RESET} {FUNCTION}{function_name}{RESET}");
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

fn module_details(signatures: &[SignatureEntry]) -> Vec<Value> {
    let mut modules = BTreeMap::<&str, Vec<&SignatureEntry>>::new();
    for signature in signatures {
        modules
            .entry(&signature.module_name)
            .or_default()
            .push(signature);
    }

    modules
        .into_iter()
        .map(|(module_name, functions)| {
            let module = functions[0];
            json!({
                "name": module_name,
                "address": module.module_address,
                "filePath": module.file_path,
                "functionCount": functions.len(),
                "functions": functions.into_iter().map(|function| json!({
                    "name": function.function_name,
                    "visibility": function.visibility,
                    "isEntry": function.is_entry,
                    "isTransactionCallable": function.is_transaction_callable,
                    "signature": normalized_signature(&function.signature),
                })).collect::<Vec<_>>(),
            })
        })
        .collect()
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
    if path.is_absolute()
        && let Ok(relative) = path.strip_prefix(&context.project_root)
    {
        return normalize_path_label(relative);
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
        let rendered =
            render_function_signature("balance", "public fun balance(account: &Account): u64");

        assert!(rendered.contains("\x1b[92mbalance\x1b[0m"));
        assert!(rendered.contains("\x1b[95m: u64\x1b[0m"));
    }

    #[test]
    fn function_signature_renderer_colors_implicit_unit_return() {
        let rendered = render_function_signature(
            "transfer",
            "public entry fun transfer<T>(coin: Coin<T>, recipient: address)",
        );

        assert!(rendered.contains("\x1b[93m<T>(coin: Coin<T>, recipient: address)\x1b[0m"));
        assert!(rendered.contains("\x1b[95m: ()\x1b[0m"));
    }
}
