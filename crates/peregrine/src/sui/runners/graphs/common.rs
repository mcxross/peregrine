use crate::{
    output::{elapsed_ms, CliDiagnostic, CliStatus, CliStep, EXIT_SUCCESS},
    sui::{
        args::GraphOutputArgs,
        project::{resolve_output_path, CliContext},
    },
};
use serde_json::Value;
use std::{collections::BTreeMap, fs, time::Instant};

pub const RESET: &str = "\x1b[0m";
pub const DIM: &str = "\x1b[90m";
pub const HEADER: &str = "\x1b[95m";
pub const MODULE: &str = "\x1b[96m";
pub const FUNCTION: &str = "\x1b[92m";
pub const EDGE: &str = "\x1b[93m";
pub const KIND: &str = "\x1b[95m";

pub fn graph_step(
    name: &str,
    started_at: Instant,
    command: String,
    context: &CliContext,
    output_args: &GraphOutputArgs,
    rendered: String,
    mut metadata: BTreeMap<String, Value>,
    details: Value,
) -> CliStep {
    let format = if output_args.dot { "dot" } else { "text" };
    metadata.insert(
        "outputFormat".to_string(),
        Value::String(format.to_string()),
    );

    let stdout = if let Some(output) = output_args.output.as_deref() {
        let output_path = resolve_output_path(&context.project_root, Some(output));
        if let Some(parent) = output_path.parent() {
            if let Err(error) = fs::create_dir_all(parent) {
                return CliStep::failed(
                    name,
                    started_at,
                    CliDiagnostic::error(
                        name,
                        format!(
                            "Could not create output directory {}: {error}",
                            parent.display()
                        ),
                    ),
                );
            }
        }

        if let Err(error) = fs::write(&output_path, rendered) {
            return CliStep::failed(
                name,
                started_at,
                CliDiagnostic::error(
                    name,
                    format!(
                        "Could not write graph output {}: {error}",
                        output_path.display()
                    ),
                ),
            );
        }

        metadata.insert(
            "outputPath".to_string(),
            Value::String(output_path.display().to_string()),
        );
        format!("Wrote {format} graph to {}", output_path.display())
    } else {
        rendered
    };

    CliStep {
        name: name.to_string(),
        status: CliStatus::Passed,
        duration_ms: elapsed_ms(started_at),
        exit_code: EXIT_SUCCESS,
        command: Some(command),
        diagnostics: Vec::new(),
        metadata,
        stdout,
        stderr: String::new(),
        details,
    }
}

pub fn requested_modules(modules: &[String]) -> Vec<&str> {
    modules
        .iter()
        .map(|module| module.trim())
        .filter(|module| !module.is_empty())
        .collect()
}
