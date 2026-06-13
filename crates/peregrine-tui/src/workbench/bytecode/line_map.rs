use peregrine_sui_mcp_protocol::{MoveBytecodeModuleView, MoveBytecodeSourceSpan};
use regex::Regex;
use std::collections::HashMap;

pub(crate) fn build_bytecode_line_map(
    module: &MoveBytecodeModuleView,
    lines: &[String],
) -> Result<HashMap<usize, MoveBytecodeSourceSpan>, String> {
    let offset_regex =
        Regex::new(r"^(\d+):.*").map_err(|error| format!("Invalid offset regex: {error}"))?;
    let function_regex =
        Regex::new(r"^(?:public(?:\(\w+\))?|native|entry)?\s*(\w+)\s*(?:<.*>)?\s*\(.*\).*\{")
            .map_err(|error| format!("Invalid function regex: {error}"))?;
    let functions = module
        .functions
        .iter()
        .map(|function| (function.name.as_str(), function))
        .collect::<HashMap<_, _>>();
    let mut current_function = None;
    let mut line_map = HashMap::new();

    for (line_index, line) in lines.iter().map(|line| line.trim()).enumerate() {
        if let Some(capture) = function_regex.captures(line) {
            current_function = capture
                .get(1)
                .and_then(|name| functions.get(name.as_str()).copied());
        }

        let Some(function) = current_function else {
            continue;
        };
        let Some(offset) = offset_regex
            .captures(line)
            .and_then(|capture| capture.get(1))
            .and_then(|offset| offset.as_str().parse::<u16>().ok())
        else {
            continue;
        };
        if let Some(span) = function
            .instructions
            .iter()
            .find(|instruction| instruction.offset == offset)
            .and_then(|instruction| instruction.source)
        {
            line_map.insert(line_index, span);
        }
    }

    Ok(line_map)
}
