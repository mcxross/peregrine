use peregrine_analysis::{GraphEdge, GraphNode, SourceSpan};
use peregrine_sui_bytecode::{
    MoveBytecodeFunctionView, MoveBytecodeInstructionView, MoveBytecodeModuleView,
};
use serde_json::json;
use std::collections::BTreeMap;

pub(crate) struct DataFlowGraphParts {
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

pub(crate) fn build_data_flow(
    package_name: &str,
    module: &MoveBytecodeModuleView,
    function: &MoveBytecodeFunctionView,
) -> DataFlowGraphParts {
    let function_id = format!(
        "{package_name}::{}::{}::{}",
        module.address, module.name, function.name
    );
    let mut builder = DataFlowBuilder::new(function_id, module, function);
    for instruction in &function.instructions {
        builder.visit(instruction);
    }
    builder.finish()
}

struct DataFlowBuilder<'a> {
    function_id: String,
    module: &'a MoveBytecodeModuleView,
    function: &'a MoveBytecodeFunctionView,
    nodes: BTreeMap<String, GraphNode>,
    edges: Vec<GraphEdge>,
    stack: Vec<String>,
    edge_index: usize,
}

impl<'a> DataFlowBuilder<'a> {
    fn new(
        function_id: String,
        module: &'a MoveBytecodeModuleView,
        function: &'a MoveBytecodeFunctionView,
    ) -> Self {
        let mut builder = Self {
            function_id,
            module,
            function,
            nodes: BTreeMap::new(),
            edges: Vec::new(),
            stack: Vec::new(),
            edge_index: 0,
        };
        for (index, type_name) in function.parameters.iter().enumerate() {
            builder.add_node(
                builder.local_id(index),
                "parameter",
                format!("parameter {index}"),
                json!({"index": index, "type": type_name}),
                None,
            );
        }
        for index in function.parameters.len()..function.local_count {
            builder.add_node(
                builder.local_id(index),
                "local",
                format!("local {index}"),
                json!({"index": index}),
                None,
            );
        }
        for (index, type_name) in function.returns.iter().enumerate() {
            builder.add_node(
                builder.return_id(index),
                "return",
                format!("return {index}"),
                json!({"index": index, "type": type_name}),
                None,
            );
        }
        builder
    }

    fn visit(&mut self, instruction: &MoveBytecodeInstructionView) {
        let instruction_id = self.instruction_id(instruction.offset);
        let span = source_span(self.module, instruction);
        self.add_node(
            instruction_id.clone(),
            &operation_kind(&instruction.opcode),
            format!("{} {}", instruction.offset, instruction.opcode),
            json!({
                "offset": instruction.offset,
                "opcode": instruction.opcode,
                "detail": instruction.detail,
                "call": instruction.call,
            }),
            span.clone(),
        );

        match instruction.opcode.as_str() {
            "CopyLoc" | "MoveLoc" | "BorrowLoc" => {
                if let Some(index) = first_index(&instruction.detail) {
                    self.connect(
                        self.local_id(index),
                        instruction_id.clone(),
                        local_read_kind(&instruction.opcode),
                        span,
                    );
                }
                self.stack.push(instruction_id);
            }
            "StLoc" => {
                if let Some(index) = first_index(&instruction.detail)
                    && let Some(value) = self.stack.pop()
                {
                    self.connect(value, self.local_id(index), "write", span);
                }
            }
            "Call" | "CallGeneric" => {
                if let Some(value) = self.stack.pop() {
                    self.connect(value, instruction_id.clone(), "argument", span.clone());
                }
                self.stack.push(instruction_id);
            }
            "Ret" => {
                for index in (0..self.function.return_count).rev() {
                    if let Some(value) = self.stack.pop() {
                        self.connect(value, self.return_id(index), "return", span.clone());
                    }
                }
            }
            opcode if is_field_borrow(opcode) => {
                if let Some(owner) = self.stack.pop() {
                    let field_id = self.field_id(&instruction.detail);
                    self.add_node(
                        field_id.clone(),
                        "field",
                        instruction.detail.clone(),
                        json!({"detail": instruction.detail}),
                        span.clone(),
                    );
                    self.connect(owner, field_id.clone(), "owner", span.clone());
                    self.connect(field_id, instruction_id.clone(), "borrow", span.clone());
                }
                self.stack.push(instruction_id);
            }
            "ReadRef" => {
                if let Some(reference) = self.stack.pop() {
                    self.connect(reference, instruction_id.clone(), "read", span.clone());
                }
                self.stack.push(instruction_id);
            }
            "WriteRef" => {
                if let Some(value) = self.stack.pop() {
                    self.connect(value, instruction_id.clone(), "writeValue", span.clone());
                }
                if let Some(reference) = self.stack.pop() {
                    self.connect(reference, instruction_id, "writeTarget", span);
                }
            }
            opcode if is_pack(opcode) => {
                if let Some(value) = self.stack.pop() {
                    self.connect(value, instruction_id.clone(), "packField", span.clone());
                }
                self.stack.push(instruction_id);
            }
            opcode if is_unpack(opcode) => {
                if let Some(value) = self.stack.pop() {
                    self.connect(value, instruction_id.clone(), "unpack", span.clone());
                }
                self.stack.push(instruction_id);
            }
            opcode if is_global_read(opcode) => {
                let resource_id = self.resource_id(&instruction.detail);
                self.add_node(
                    resource_id.clone(),
                    "resource",
                    instruction.detail.clone(),
                    json!({"detail": instruction.detail}),
                    span.clone(),
                );
                self.connect(
                    resource_id,
                    instruction_id.clone(),
                    "globalRead",
                    span.clone(),
                );
                self.stack.push(instruction_id);
            }
            opcode if is_global_write(opcode) => {
                let resource_id = self.resource_id(&instruction.detail);
                self.add_node(
                    resource_id.clone(),
                    "resource",
                    instruction.detail.clone(),
                    json!({"detail": instruction.detail}),
                    span.clone(),
                );
                if let Some(value) = self.stack.pop() {
                    self.connect(value, resource_id, "globalWrite", span);
                }
            }
            opcode if produces_value(opcode) => {
                for input in self.pop_inputs(opcode) {
                    self.connect(input, instruction_id.clone(), "operand", span.clone());
                }
                self.stack.push(instruction_id);
            }
            _ => {}
        }
    }

    fn pop_inputs(&mut self, opcode: &str) -> Vec<String> {
        let count = if is_binary(opcode) { 2 } else { 1 };
        (0..count).filter_map(|_| self.stack.pop()).collect()
    }

    fn connect(&mut self, from: String, to: String, kind: &str, span: Option<SourceSpan>) {
        self.edge_index += 1;
        self.edges.push(GraphEdge {
            id: format!("{}:edge:{}", self.function_id, self.edge_index),
            from,
            to,
            kind: kind.to_string(),
            spans: span.into_iter().collect(),
            evidence: Vec::new(),
            metadata: json!({}),
        });
    }

    fn add_node(
        &mut self,
        id: String,
        kind: &str,
        label: String,
        metadata: serde_json::Value,
        span: Option<SourceSpan>,
    ) {
        self.nodes.entry(id.clone()).or_insert(GraphNode {
            id,
            kind: kind.to_string(),
            label,
            span,
            metadata,
        });
    }

    fn local_id(&self, index: usize) -> String {
        format!("{}:local:{index}", self.function_id)
    }

    fn return_id(&self, index: usize) -> String {
        format!("{}:return:{index}", self.function_id)
    }

    fn instruction_id(&self, offset: u16) -> String {
        format!("{}:instruction:{offset}", self.function_id)
    }

    fn field_id(&self, detail: &str) -> String {
        format!("{}:field:{}", self.function_id, stable_label(detail))
    }

    fn resource_id(&self, detail: &str) -> String {
        format!("{}:resource:{}", self.function_id, stable_label(detail))
    }

    fn finish(self) -> DataFlowGraphParts {
        DataFlowGraphParts {
            nodes: self.nodes.into_values().collect(),
            edges: self.edges,
        }
    }
}

fn source_span(
    module: &MoveBytecodeModuleView,
    instruction: &MoveBytecodeInstructionView,
) -> Option<SourceSpan> {
    let source = instruction.source.as_ref()?;
    Some(SourceSpan {
        file_path: module
            .source_path
            .clone()
            .unwrap_or_else(|| module.bytecode_path.clone()),
        start_line: 0,
        end_line: 0,
        start_byte: source.start_byte as usize,
        end_byte: source.end_byte as usize,
    })
}

fn first_index(detail: &str) -> Option<usize> {
    detail
        .split(|character: char| !character.is_ascii_digit())
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse().ok())
}

fn stable_label(detail: &str) -> String {
    detail
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

fn operation_kind(opcode: &str) -> String {
    match opcode {
        "Call" | "CallGeneric" => "call",
        "Ret" => "return",
        opcode if is_pack(opcode) => "pack",
        opcode if is_unpack(opcode) => "unpack",
        opcode if is_field_borrow(opcode) => "fieldBorrow",
        opcode if is_global_read(opcode) => "globalRead",
        opcode if is_global_write(opcode) => "globalWrite",
        _ => "operation",
    }
    .to_string()
}

fn local_read_kind(opcode: &str) -> &'static str {
    match opcode {
        "MoveLoc" => "move",
        "BorrowLoc" => "borrow",
        _ => "copy",
    }
}

fn is_field_borrow(opcode: &str) -> bool {
    matches!(
        opcode,
        "MutBorrowField"
            | "ImmBorrowField"
            | "MutBorrowFieldGeneric"
            | "ImmBorrowFieldGeneric"
            | "BorrowField"
            | "BorrowFieldMut"
    )
}

fn is_pack(opcode: &str) -> bool {
    matches!(
        opcode,
        "Pack" | "PackGeneric" | "PackVariant" | "PackVariantGeneric"
    )
}

fn is_unpack(opcode: &str) -> bool {
    matches!(
        opcode,
        "Unpack" | "UnpackGeneric" | "UnpackVariant" | "UnpackVariantGeneric"
    )
}

fn is_global_read(opcode: &str) -> bool {
    matches!(
        opcode,
        "MoveFrom"
            | "MoveFromGeneric"
            | "ImmBorrowGlobal"
            | "ImmBorrowGlobalGeneric"
            | "MutBorrowGlobal"
            | "MutBorrowGlobalGeneric"
            | "Exists"
            | "ExistsGeneric"
    )
}

fn is_global_write(opcode: &str) -> bool {
    matches!(opcode, "MoveTo" | "MoveToGeneric")
}

fn produces_value(opcode: &str) -> bool {
    opcode.starts_with("Ld")
        || is_binary(opcode)
        || matches!(
            opcode,
            "ReadRef"
                | "FreezeRef"
                | "CastU8"
                | "CastU16"
                | "CastU32"
                | "CastU64"
                | "CastU128"
                | "CastU256"
                | "VecLen"
                | "VecPack"
                | "VecUnpack"
        )
}

fn is_binary(opcode: &str) -> bool {
    matches!(
        opcode,
        "Add"
            | "Sub"
            | "Mul"
            | "Mod"
            | "Div"
            | "BitOr"
            | "BitAnd"
            | "Xor"
            | "Shl"
            | "Shr"
            | "Or"
            | "And"
            | "Eq"
            | "Neq"
            | "Lt"
            | "Gt"
            | "Le"
            | "Ge"
    )
}
