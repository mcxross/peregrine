#[derive(Clone)]
struct NamePart {
    name: String,
    is_macro: bool,
}

enum ResolvedCall {
    Local(String),
    External(MemberRef),
    Unresolved(&'static str),
}

#[derive(Default)]
struct CallEdgeInput {
    source: String,
    target: String,
    call_kind: String,
    confidence: String,
    raw_target: String,
    type_arguments: Vec<String>,
    span: MoveSourceSpan,
    is_external: bool,
    is_resolved: bool,
}

#[derive(Clone)]
struct TypeUsageInput {
    relationship: String,
    field_name: Option<String>,
    variant_name: Option<String>,
    function_name: Option<String>,
    parameter_name: Option<String>,
    type_argument_index: Option<usize>,
    is_mutable: bool,
    is_reference: bool,
    declaring_type_id: Option<String>,
    declaring_field_name: Option<String>,
}

impl Default for TypeUsageInput {
    fn default() -> Self {
        Self {
            relationship: "usage".to_string(),
            field_name: None,
            variant_name: None,
            function_name: None,
            parameter_name: None,
            type_argument_index: None,
            is_mutable: false,
            is_reference: false,
            declaring_type_id: None,
            declaring_field_name: None,
        }
    }
}

struct TypeEdgeInput {
    source: String,
    target: String,
    relationship: String,
    field_name: Option<String>,
    variant_name: Option<String>,
    function_name: Option<String>,
    parameter_name: Option<String>,
    type_argument_index: Option<usize>,
    is_mutable: bool,
    is_reference: bool,
    type_expression: Option<String>,
    declaring_type_id: Option<String>,
    declaring_field_name: Option<String>,
    type_argument_name: Option<String>,
    span: MoveSourceSpan,
    confidence: String,
    evidence: Vec<String>,
}

#[derive(Default)]
struct StateAccessEdgeInput {
    source: String,
    target: String,
    access_kind: String,
    field_name: Option<String>,
    via_function: Option<String>,
    span: MoveSourceSpan,
    confidence: String,
    evidence: Vec<String>,
}

impl Default for TypeEdgeInput {
    fn default() -> Self {
        Self {
            source: String::new(),
            target: String::new(),
            relationship: String::new(),
            field_name: None,
            variant_name: None,
            function_name: None,
            parameter_name: None,
            type_argument_index: None,
            is_mutable: false,
            is_reference: false,
            type_expression: None,
            declaring_type_id: None,
            declaring_field_name: None,
            type_argument_name: None,
            span: MoveSourceSpan::default(),
            confidence: "syntactic".to_string(),
            evidence: Vec::new(),
        }
    }
}

#[derive(Clone)]
struct SummaryTypeUsageInput {
    relationship: String,
    field_name: Option<String>,
    variant_name: Option<String>,
    function_name: Option<String>,
    parameter_name: Option<String>,
    type_argument_index: Option<usize>,
    is_mutable: bool,
    is_reference: bool,
    type_expression: Option<String>,
    declaring_type_id: Option<String>,
    declaring_field_name: Option<String>,
    type_argument_name: Option<String>,
    span: MoveSourceSpan,
}

impl Default for SummaryTypeUsageInput {
    fn default() -> Self {
        Self {
            relationship: "summaryUsage".to_string(),
            field_name: None,
            variant_name: None,
            function_name: None,
            parameter_name: None,
            type_argument_index: None,
            is_mutable: false,
            is_reference: false,
            type_expression: None,
            declaring_type_id: None,
            declaring_field_name: None,
            type_argument_name: None,
            span: summary_span(),
        }
    }
}

