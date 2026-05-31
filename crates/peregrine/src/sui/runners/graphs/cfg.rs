use super::{
    common::{DIM, EDGE, FUNCTION, HEADER, KIND, MODULE, RESET, graph_step},
    dot::{DotEdgeStyle, dot_edge_attrs, dot_id, dot_label},
    project::{module_matches, selected_source_package},
};
use crate::{
    output::{CliDiagnostic, CliDiagnosticSeverity, CliStatus, CliStep, elapsed_ms},
    sui::{args::CfgArgs, project::CliContext, runners::run_build},
};
use peregrine_bytecode::{
    MoveBytecodeControlFlowView, MoveBytecodeFunctionView, MoveBytecodeModuleView,
    load_package_bytecode,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, time::Instant};

struct CfgTarget<'a> {
    module: &'a MoveBytecodeModuleView,
    function: &'a MoveBytecodeFunctionView,
}

pub fn run_cfg(context: &CliContext, args: &CfgArgs) -> CliStep {
    let started_at = Instant::now();
    let package = match selected_source_package(context, "cfg") {
        Ok(package) => package,
        Err(error) => return CliStep::failed("cfg", started_at, error),
    };

    let build = run_build(context);

    if build.status != CliStatus::Passed {
        let details = json!({ "build": build.clone() });

        return CliStep {
            name: "cfg".to_string(),
            status: CliStatus::Failed,
            duration_ms: elapsed_ms(started_at),
            exit_code: build.exit_code,
            command: Some(display_command(args)),
            diagnostics: vec![CliDiagnostic {
                severity: CliDiagnosticSeverity::Error,
                source: "cfg".to_string(),
                code: Some("BuildFailed".to_string()),
                message: "Could not build the package before reading bytecode control flow."
                    .to_string(),
                file: None,
                span: None,
            }],
            metadata: BTreeMap::from([("buildExitCode".to_string(), json!(build.exit_code))]),
            stdout: build.stdout,
            stderr: build.stderr,
            details,
        };
    }

    let bytecode = match load_package_bytecode(&context.package_root, &package.name) {
        Ok(bytecode) => bytecode,
        Err(error) => {
            return CliStep::failed("cfg", started_at, CliDiagnostic::error("cfg", error));
        }
    };
    let targets = selected_cfg_targets(&bytecode.modules, args);

    if targets.is_empty() {
        return CliStep::failed(
            "cfg",
            started_at,
            CliDiagnostic::error(
                "cfg",
                "No bytecode functions matched the requested CFG target.",
            ),
        );
    }

    let rendered = if args.output.dot {
        render_cfg_dot(&targets)
    } else {
        render_cfg_text(&package.name, &targets)
    };
    let block_count = targets
        .iter()
        .map(|target| target.function.control_flow.blocks.len())
        .sum::<usize>();
    let edge_count = targets
        .iter()
        .map(|target| target.function.control_flow.edges.len())
        .sum::<usize>();

    graph_step(
        "cfg",
        started_at,
        display_command(args),
        context,
        &args.output,
        rendered,
        BTreeMap::from([
            ("package".to_string(), json!(package.name)),
            ("functionCount".to_string(), json!(targets.len())),
            ("blockCount".to_string(), json!(block_count)),
            ("edgeCount".to_string(), json!(edge_count)),
        ]),
        json!({
            "package": package.name,
            "targets": targets.iter().map(cfg_target_details).collect::<Vec<_>>(),
        }),
    )
}

fn selected_cfg_targets<'a>(
    modules: &'a [MoveBytecodeModuleView],
    args: &CfgArgs,
) -> Vec<CfgTarget<'a>> {
    let requested_module = args
        .module
        .as_deref()
        .map(str::trim)
        .filter(|module| !module.is_empty());
    let requested_function = args
        .function
        .as_deref()
        .map(str::trim)
        .filter(|function| !function.is_empty());
    let mut targets = Vec::new();

    for module in modules {
        if requested_module.is_some_and(|requested| {
            !module_matches(requested, Some(module.address.as_str()), &module.name)
        }) {
            continue;
        }

        for function in &module.functions {
            if requested_function.is_some_and(|requested| requested != function.name) {
                continue;
            }

            targets.push(CfgTarget { module, function });
        }
    }

    targets.sort_by(|left, right| {
        left.module
            .name
            .cmp(&right.module.name)
            .then_with(|| left.function.name.cmp(&right.function.name))
    });
    targets
}

fn render_cfg_text(package_name: &str, targets: &[CfgTarget<'_>]) -> String {
    let mut lines = vec![format!(
        "{HEADER}cfg{RESET} {package_name} {DIM}functions={}{RESET}",
        targets.len()
    )];

    for target in targets {
        let flow = &target.function.control_flow;
        lines.push(format!(
            "{DIM}|--{RESET} {MODULE}module{RESET} {}::{}",
            target.module.address, target.module.name
        ));
        lines.push(format!(
            "{DIM}|   |--{RESET} {FUNCTION}{}{RESET}{} {DIM}blocks={} edges={}{RESET}",
            target.function.name,
            function_type_suffix(target.function),
            flow.blocks.len(),
            flow.edges.len()
        ));

        for block in &flow.blocks {
            lines.push(format!(
                "{DIM}|   |   |--{RESET} {KIND}{}{RESET} {DIM}offsets {}..{}{RESET}",
                block.label, block.start_offset, block.end_offset
            ));

            for offset in &block.instruction_offsets {
                if let Some(instruction) = target
                    .function
                    .instructions
                    .iter()
                    .find(|instruction| instruction.offset == *offset)
                {
                    let detail = if instruction.detail.is_empty() {
                        instruction.opcode.clone()
                    } else {
                        format!("{} {}", instruction.opcode, instruction.detail)
                    };
                    lines.push(format!(
                        "{DIM}|   |   |   |--{RESET} {EDGE}{:04}{RESET} {}",
                        instruction.offset, detail
                    ));
                }
            }
        }

        if !flow.edges.is_empty() {
            lines.push(format!("{DIM}|   |   |--{RESET} {KIND}edges{RESET}"));
            for edge in &flow.edges {
                lines.push(format!(
                    "{DIM}|   |   |   |--{RESET} {} {EDGE}--{}-->{RESET} {} {DIM}@{} -> @{}{RESET}",
                    edge.source, edge.kind, edge.target, edge.source_offset, edge.target_offset
                ));
            }
        }
    }

    lines.join("\n")
}

fn render_cfg_dot(targets: &[CfgTarget<'_>]) -> String {
    let mut lines = vec![
        "digraph peregrine_cfg {".to_string(),
        "  graph [rankdir=TB, bgcolor=\"transparent\", compound=true];".to_string(),
        "  node [shape=box, style=\"rounded,filled\", fontname=\"Menlo\", fontsize=10, fillcolor=\"#1f2937\", fontcolor=\"#f8fafc\"];".to_string(),
        "  edge [fontname=\"Menlo\", fontsize=9, color=\"#38bdf8\", fontcolor=\"#bae6fd\"];".to_string(),
    ];

    for target in targets {
        let cluster_id = format!(
            "cluster_{}_{}",
            dot_safe_name(&target.module.name),
            dot_safe_name(&target.function.name)
        );
        lines.push(format!("  subgraph {cluster_id} {{"));
        lines.push(format!(
            "    label={};",
            dot_label(&format!("{}::{}", target.module.name, target.function.name))
        ));
        lines.push("    color=\"#475569\";".to_string());

        for block in &target.function.control_flow.blocks {
            let node_id = cfg_node_id(target.module, target.function, &block.id);
            lines.push(format!(
                "    {} [label={}];",
                dot_id(&node_id),
                dot_label(&cfg_block_label(
                    &target.function.control_flow,
                    target.function,
                    &block.id
                ))
            ));
        }

        for edge in &target.function.control_flow.edges {
            lines.push(format!(
                "    {} -> {} [{}];",
                dot_id(&cfg_node_id(target.module, target.function, &edge.source)),
                dot_id(&cfg_node_id(target.module, target.function, &edge.target)),
                dot_edge_attrs(&edge.kind, cfg_edge_style(&edge.kind))
            ));
        }

        lines.push("  }".to_string());
    }

    lines.push("}".to_string());
    lines.join("\n")
}

const TRUE_EDGE: DotEdgeStyle = DotEdgeStyle::new("#22c55e", "#bbf7d0", "solid", "1.8");
const FALSE_EDGE: DotEdgeStyle = DotEdgeStyle::new("#ef4444", "#fecaca", "solid", "1.8");
const BRANCH_EDGE: DotEdgeStyle = DotEdgeStyle::new("#8b5cf6", "#ddd6fe", "bold", "2.1");
const FALLTHROUGH_EDGE: DotEdgeStyle = DotEdgeStyle::new("#38bdf8", "#bae6fd", "solid", "1.5");
const VARIANT_EDGE: DotEdgeStyle = DotEdgeStyle::new("#f59e0b", "#fde68a", "dashed", "1.8");
const SUCCESSOR_EDGE: DotEdgeStyle = DotEdgeStyle::new("#94a3b8", "#cbd5e1", "dotted", "1.4");

fn cfg_edge_style(kind: &str) -> DotEdgeStyle {
    match kind {
        "true" => TRUE_EDGE,
        "false" => FALSE_EDGE,
        "branch" => BRANCH_EDGE,
        "fallthrough" => FALLTHROUGH_EDGE,
        "variant" => VARIANT_EDGE,
        "successor" => SUCCESSOR_EDGE,
        _ => SUCCESSOR_EDGE,
    }
}

fn cfg_block_label(
    flow: &MoveBytecodeControlFlowView,
    function: &MoveBytecodeFunctionView,
    block_id: &str,
) -> String {
    let Some(block) = flow.blocks.iter().find(|block| block.id == block_id) else {
        return block_id.to_string();
    };
    let instructions = block
        .instruction_offsets
        .iter()
        .filter_map(|offset| {
            function
                .instructions
                .iter()
                .find(|instruction| instruction.offset == *offset)
        })
        .map(|instruction| {
            if instruction.detail.is_empty() {
                format!("{:04}: {}", instruction.offset, instruction.opcode)
            } else {
                format!(
                    "{:04}: {} {}",
                    instruction.offset, instruction.opcode, instruction.detail
                )
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    if instructions.is_empty() {
        format!(
            "{}\noffsets {}..{}",
            block.label, block.start_offset, block.end_offset
        )
    } else {
        format!(
            "{}\noffsets {}..{}\n{}",
            block.label, block.start_offset, block.end_offset, instructions
        )
    }
}

fn function_type_suffix(function: &MoveBytecodeFunctionView) -> String {
    let params = function.parameters.join(", ");
    let returns = match function.returns.as_slice() {
        [] => "()".to_string(),
        [single] => single.clone(),
        many => format!("({})", many.join(", ")),
    };

    format!("({params}): {returns}")
}

fn cfg_target_details(target: &CfgTarget<'_>) -> Value {
    json!({
        "module": target.module.name,
        "address": target.module.address,
        "function": target.function.name,
        "blockCount": target.function.control_flow.blocks.len(),
        "edgeCount": target.function.control_flow.edges.len(),
        "controlFlow": target.function.control_flow,
    })
}

fn cfg_node_id(
    module: &MoveBytecodeModuleView,
    function: &MoveBytecodeFunctionView,
    block_id: &str,
) -> String {
    format!("{}::{}::{}", module.name, function.name, block_id)
}

fn dot_safe_name(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn display_command(args: &CfgArgs) -> String {
    let mut command = "peregrine cfg".to_string();

    if let Some(module) = &args.module {
        command.push_str(&format!(" --module {module}"));
    }
    if let Some(function) = &args.function {
        command.push_str(&format!(" --function {function}"));
    }
    if args.output.dot {
        command.push_str(" --dot");
    }
    if let Some(output) = &args.output.output {
        command.push_str(&format!(" --output {}", output.display()));
    }

    command
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_bytecode::{
        MoveBytecodeBasicBlockView, MoveBytecodeControlFlowEdgeView, MoveBytecodeInstructionView,
    };

    #[test]
    fn cfg_dot_renders_blocks_and_edges() {
        let module = module();
        let target = CfgTarget {
            module: &module,
            function: &module.functions[0],
        };
        let rendered = render_cfg_dot(&[target]);

        assert!(rendered.contains("digraph peregrine_cfg"));
        assert!(rendered.contains("entry"));
        assert!(rendered.contains("fallthrough"));
        assert!(rendered.contains("color=\"#38bdf8\""));
    }

    fn module() -> MoveBytecodeModuleView {
        MoveBytecodeModuleView {
            name: "m".to_string(),
            address: "pkg".to_string(),
            package_name: "pkg".to_string(),
            is_dependency: false,
            bytecode_path: "build/pkg/bytecode_modules/m.mv".to_string(),
            source_map_path: None,
            source_path: None,
            byte_size: 1,
            version: 6,
            function_count: 1,
            instruction_count: 2,
            struct_count: 0,
            constant_count: 0,
            import_count: 0,
            friend_count: 0,
            functions: vec![MoveBytecodeFunctionView {
                name: "entry".to_string(),
                visibility: "Public".to_string(),
                is_entry: true,
                parameters: vec!["u64".to_string()],
                returns: Vec::new(),
                type_parameter_count: 0,
                instruction_count: 2,
                local_count: 1,
                return_count: 1,
                acquires: Vec::new(),
                instructions: vec![
                    MoveBytecodeInstructionView {
                        offset: 0,
                        opcode: "LdU64".to_string(),
                        detail: "1".to_string(),
                        call: None,
                        source: None,
                    },
                    MoveBytecodeInstructionView {
                        offset: 1,
                        opcode: "Ret".to_string(),
                        detail: String::new(),
                        call: None,
                        source: None,
                    },
                ],
                control_flow: MoveBytecodeControlFlowView {
                    blocks: vec![
                        MoveBytecodeBasicBlockView {
                            id: "bb-0".to_string(),
                            label: "BB0 (entry)".to_string(),
                            start_offset: 0,
                            end_offset: 0,
                            instruction_offsets: vec![0],
                        },
                        MoveBytecodeBasicBlockView {
                            id: "bb-1".to_string(),
                            label: "BB1".to_string(),
                            start_offset: 1,
                            end_offset: 1,
                            instruction_offsets: vec![1],
                        },
                    ],
                    edges: vec![MoveBytecodeControlFlowEdgeView {
                        source: "bb-0".to_string(),
                        target: "bb-1".to_string(),
                        source_offset: 0,
                        target_offset: 1,
                        kind: "fallthrough".to_string(),
                    }],
                },
            }],
            imports: Vec::new(),
            disassembly: String::new(),
        }
    }
}
