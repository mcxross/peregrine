use crate::core::OperationKind;

pub fn bytecode_opcode_to_operation_kind(opcode: &str) -> OperationKind {
    match opcode {
        "Call" | "CallGeneric" => OperationKind::Call,
        "Ret" => OperationKind::Return,
        "Abort" => OperationKind::Abort,
        "Branch" => OperationKind::Branch,
        "BrTrue" | "BrFalse" | "VariantSwitch" => OperationKind::BranchIf,
        "ReadRef" => OperationKind::ReadField,
        "WriteRef" => OperationKind::WriteField,
        "Pack" | "PackGeneric" | "PackVariant" | "PackVariantGeneric" => OperationKind::Pack,
        "Unpack" | "UnpackGeneric" | "UnpackVariant" | "UnpackVariantGeneric" => {
            OperationKind::Unpack
        }
        "Add" => OperationKind::Add,
        "Sub" => OperationKind::Sub,
        "Mul" => OperationKind::Mul,
        "Div" => OperationKind::Div,
        "Mod" => OperationKind::Mod,
        _ => OperationKind::Unknown,
    }
}
