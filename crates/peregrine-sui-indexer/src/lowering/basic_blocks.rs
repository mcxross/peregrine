use move_binary_format::file_format::{Bytecode, CodeOffset};

use crate::core::{BasicBlock, SourceSpan, logical_id};

pub fn lower_basic_blocks(
    package_id: &str,
    function_id: &str,
    code: &[Bytecode],
    jump_tables: &[move_binary_format::file_format::VariantJumpTable],
    source_span: SourceSpan,
) -> Vec<BasicBlock> {
    if code.is_empty() {
        return Vec::new();
    }
    let mut leaders = std::collections::BTreeMap::<CodeOffset, ()>::new();
    leaders.insert(0, ());
    for (offset, instruction) in code.iter().enumerate() {
        let offset = offset as CodeOffset;
        for target in instruction.offsets(jump_tables) {
            if (target as usize) < code.len() {
                leaders.insert(target, ());
            }
        }
        if instruction.is_branch() {
            let next = offset.saturating_add(1);
            if (next as usize) < code.len() {
                leaders.insert(next, ());
            }
        }
    }
    let starts = leaders.keys().copied().collect::<Vec<_>>();
    starts
        .iter()
        .enumerate()
        .map(|(index, start)| {
            let end = starts
                .get(index + 1)
                .copied()
                .map(|next| next.saturating_sub(1))
                .unwrap_or_else(|| code.len().saturating_sub(1) as CodeOffset);
            BasicBlock {
                id: logical_id("block", [function_id, &index.to_string()]),
                package_id: package_id.to_string(),
                function_id: function_id.to_string(),
                index_in_function: index,
                label: if index == 0 {
                    "BB0 (entry)".to_string()
                } else {
                    format!("BB{index}")
                },
                start_operation_index: Some(*start as usize),
                end_operation_index: Some(end as usize),
                source_span: source_span.clone(),
            }
        })
        .collect()
}
