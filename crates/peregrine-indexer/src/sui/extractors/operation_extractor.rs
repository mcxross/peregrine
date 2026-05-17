use crate::core::{Operation, OperationKind};

pub fn operation_histogram<'a>(
    operations: impl IntoIterator<Item = &'a Operation>,
) -> std::collections::BTreeMap<String, usize> {
    let mut histogram = std::collections::BTreeMap::new();
    for operation in operations {
        *histogram
            .entry(format!("{:?}", operation.kind))
            .or_default() += 1;
    }
    histogram
}

pub fn is_high_signal_operation(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::Call
            | OperationKind::Abort
            | OperationKind::Assert
            | OperationKind::ReadField
            | OperationKind::WriteField
            | OperationKind::BorrowField
            | OperationKind::BorrowFieldMut
            | OperationKind::MoveFrom
            | OperationKind::MoveTo
            | OperationKind::Pack
            | OperationKind::Unpack
            | OperationKind::Branch
            | OperationKind::BranchIf
    )
}

pub fn compact_operation_line(operation: &Operation) -> String {
    format!(
        "{}: {}",
        operation.index_in_function,
        if operation.display.is_empty() {
            format!("{:?}", operation.kind)
        } else {
            operation.display.clone()
        }
    )
}
