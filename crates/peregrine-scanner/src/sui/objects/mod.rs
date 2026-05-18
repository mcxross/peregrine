use std::collections::{BTreeMap, BTreeSet, HashSet, VecDeque};
use std::path::PathBuf;

use peregrine_types::sui::move_model::{MoveModule, MoveStructSignature};
use serde::Serialize;

use crate::{
    core::{
        EvidenceSource, PackageScanner, ScanInput, ScannerConfidence, ScannerDiagnostic,
        ScannerOutput, SourceMode,
    },
    sui::facts::{load_bytecode_package_facts, BytecodeFunctionFact, BytecodePackageFacts},
};

pub const OBJECT_SCANNER_ID: &str = "sui.objects";
const CALL_GRAPH_DEPTH: usize = 5;
const STAGE_ORDER: &[ObjectLifecycleStageKind] = &[
    ObjectLifecycleStageKind::Created,
    ObjectLifecycleStageKind::Owned,
    ObjectLifecycleStageKind::Mutated,
    ObjectLifecycleStageKind::Transferred,
    ObjectLifecycleStageKind::Shared,
    ObjectLifecycleStageKind::Wrapped,
    ObjectLifecycleStageKind::Immutable,
    ObjectLifecycleStageKind::Party,
    ObjectLifecycleStageKind::Deleted,
];

pub struct ObjectScanner;

impl PackageScanner for ObjectScanner {
    fn id(&self) -> &'static str {
        OBJECT_SCANNER_ID
    }

    fn scan(&self, input: &ScanInput<'_>) -> ScannerOutput {
        ScannerOutput::Objects(scan_objects(input))
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectScanReport {
    pub capability_findings: Vec<ObjectCapabilityFinding>,
    pub ownership_findings: Vec<ObjectOwnershipModel>,
    pub lifecycle_maps: Vec<ObjectLifecycleModel>,
    pub shared_object_structs: Vec<String>,
    pub diagnostics: Vec<ScannerDiagnostic>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectCapabilityFinding {
    pub type_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub confidence: ScannerConfidence,
    pub evidence: Vec<ObjectEvidence>,
    pub protected_functions: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectOwnershipModel {
    pub type_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub ownership_kind: ObjectClassification,
    pub confidence: ScannerConfidence,
    pub evidence: Vec<ObjectEvidence>,
    pub related_functions: Vec<String>,
    pub wrapped_types: Vec<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectLifecycleModel {
    pub type_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub abilities: Vec<String>,
    pub is_capability_like: bool,
    pub stages: Vec<ObjectLifecycleStageModel>,
    pub touched_by: Vec<ObjectLifecycleFunctionRef>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectLifecycleStageModel {
    pub kind: ObjectLifecycleStageKind,
    pub functions: Vec<ObjectLifecycleFunctionRef>,
    pub evidence: Vec<ObjectEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectLifecycleFunctionRef {
    pub module_name: String,
    pub function_name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub visibility: String,
    pub is_entry: bool,
    pub is_transaction_callable: bool,
    pub direct: bool,
    pub call_path: Vec<String>,
    pub evidence: Vec<ObjectEvidence>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ObjectClassification {
    Capability,
    Shared,
    AddressOwned,
    Immutable,
    Wrapped,
    Party,
}

impl ObjectClassification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Capability => "capability",
            Self::Shared => "shared",
            Self::AddressOwned => "addressOwned",
            Self::Immutable => "immutable",
            Self::Wrapped => "wrapped",
            Self::Party => "party",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ObjectLifecycleStageKind {
    Created,
    Owned,
    Mutated,
    Transferred,
    Shared,
    Wrapped,
    Immutable,
    Party,
    Deleted,
}

impl ObjectLifecycleStageKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::Owned => "owned",
            Self::Mutated => "mutated",
            Self::Transferred => "transferred",
            Self::Shared => "shared",
            Self::Wrapped => "wrapped",
            Self::Immutable => "immutable",
            Self::Party => "party",
            Self::Deleted => "deleted",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectEvidence {
    pub source: EvidenceSource,
    pub confidence: ScannerConfidence,
    pub message: String,
}

impl ObjectEvidence {
    fn source(message: impl Into<String>) -> Self {
        Self {
            source: EvidenceSource::SourceFallback,
            confidence: ScannerConfidence::Medium,
            message: message.into(),
        }
    }

    fn bytecode(message: impl Into<String>) -> Self {
        Self {
            source: EvidenceSource::Bytecode,
            confidence: ScannerConfidence::High,
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

#[derive(Clone)]
struct ObjectCandidate {
    module_name: String,
    type_name: String,
    qualified_name: String,
    file_path: String,
    abilities: Vec<String>,
    fields: Vec<ObjectField>,
    source: EvidenceSource,
}

#[derive(Clone)]
struct ObjectField {
    name: String,
    type_name: String,
}

#[derive(Clone)]
struct FunctionLookup {
    module_name: String,
    function_name: String,
    qualified_name: String,
    file_path: String,
    visibility: String,
    is_entry: bool,
    is_transaction_callable: bool,
    signature: String,
    body: Option<String>,
    attributes: Vec<String>,
    source: EvidenceSource,
    bytecode: Option<BytecodeFunctionFact>,
}

#[derive(Clone)]
struct DirectEvent {
    object_key: String,
    stage: ObjectLifecycleStageKind,
    function_key: String,
    evidence: ObjectEvidence,
}

struct IndirectCaller {
    caller_key: String,
    call_path: Vec<String>,
}

pub fn scan_objects(input: &ScanInput<'_>) -> ObjectScanReport {
    let mut diagnostics = Vec::new();
    let bytecode_facts = if input.source_mode == SourceMode::SourceOnly {
        diagnostics.push(ScannerDiagnostic::info(
            OBJECT_SCANNER_ID,
            EvidenceSource::Bytecode,
            "bytecode provider skipped by source-only scan mode",
        ));
        BytecodePackageFacts::default()
    } else {
        let (facts, mut provider_diagnostics) =
            load_bytecode_package_facts(&bytecode_roots_from_input(input));
        diagnostics.append(&mut provider_diagnostics);
        facts
    };

    if input.package_model.modules.is_empty() {
        diagnostics.push(ScannerDiagnostic::warning(
            OBJECT_SCANNER_ID,
            EvidenceSource::Compiler,
            "source model contains no parsed modules",
        ));
    } else {
        diagnostics.push(ScannerDiagnostic::info(
            OBJECT_SCANNER_ID,
            EvidenceSource::Compiler,
            format!(
                "loaded {} parsed source module(s)",
                input.package_model.modules.len()
            ),
        ));
    }

    if bytecode_facts.is_empty() && input.source_mode == SourceMode::CompilerOnly {
        diagnostics.push(ScannerDiagnostic::warning(
            OBJECT_SCANNER_ID,
            EvidenceSource::Scanner,
            "compiler-only scan requested but no bytecode facts were available",
        ));
    }

    let candidates = object_candidates(&input.package_model.modules, &bytecode_facts);
    let functions = function_index(&input.package_model.modules, &bytecode_facts);
    let capabilities = capability_findings(&candidates, &functions);
    let capability_structs = capabilities
        .iter()
        .filter(|finding| finding.confidence != ScannerConfidence::Low)
        .map(|finding| finding.qualified_name.clone())
        .collect::<Vec<_>>();
    let ownership_findings =
        object_ownership_findings(&candidates, &functions, &capability_structs);
    let lifecycle_maps = object_lifecycle_maps(&candidates, &functions, &capability_structs);
    let shared_object_structs = shared_object_structs(&candidates, &functions, &capability_structs);

    ObjectScanReport {
        capability_findings: capabilities,
        ownership_findings,
        lifecycle_maps,
        shared_object_structs,
        diagnostics,
    }
}

fn bytecode_roots_from_input(input: &ScanInput<'_>) -> Vec<PathBuf> {
    let mut roots = Vec::new();

    if let Some(build_root) = input.build_root.clone() {
        roots.push(build_root);
    }

    if let Some(package_root) = input.package_root.clone() {
        roots.push(package_root);
    }

    roots
}

fn object_candidates(
    modules: &[MoveModule],
    bytecode_facts: &BytecodePackageFacts,
) -> Vec<ObjectCandidate> {
    let mut by_name = BTreeMap::<String, ObjectCandidate>::new();

    for module in modules.iter().filter(|module| !is_test_module(module)) {
        for move_struct in module.structs.iter().filter(|move_struct| {
            struct_has_ability(move_struct, "key") && !has_test_attribute(&move_struct.attributes)
        }) {
            let qualified_name = format!("{}::{}", module.name, move_struct.name);
            by_name.insert(
                qualified_name.clone(),
                ObjectCandidate {
                    module_name: module.name.clone(),
                    type_name: move_struct.name.clone(),
                    qualified_name,
                    file_path: module.file_path.clone(),
                    abilities: move_struct.abilities.clone(),
                    fields: move_struct
                        .fields
                        .iter()
                        .map(|field| ObjectField {
                            name: field.name.clone(),
                            type_name: field.type_name.clone(),
                        })
                        .collect(),
                    source: EvidenceSource::Compiler,
                },
            );
        }
    }

    for struct_fact in bytecode_facts
        .modules
        .iter()
        .flat_map(|module| module.structs.iter())
        .filter(|struct_fact| has_ability(&struct_fact.abilities, "key"))
    {
        let candidate = ObjectCandidate {
            module_name: struct_fact.module_name.clone(),
            type_name: struct_fact.type_name.clone(),
            qualified_name: struct_fact.qualified_name.clone(),
            file_path: struct_fact.full_name.clone(),
            abilities: struct_fact.abilities.clone(),
            fields: struct_fact
                .fields
                .iter()
                .map(|field| ObjectField {
                    name: field.name.clone(),
                    type_name: field.type_name.clone(),
                })
                .collect(),
            source: EvidenceSource::Bytecode,
        };

        by_name
            .entry(candidate.qualified_name.clone())
            .and_modify(|existing| {
                existing.abilities = candidate.abilities.clone();
                if existing.fields.is_empty() {
                    existing.fields = candidate.fields.clone();
                }
                existing.source = EvidenceSource::Bytecode;
            })
            .or_insert(candidate);
    }

    by_name.into_values().collect()
}

fn function_index(
    modules: &[MoveModule],
    bytecode_facts: &BytecodePackageFacts,
) -> BTreeMap<String, FunctionLookup> {
    let mut functions = BTreeMap::new();

    for module in modules.iter().filter(|module| !is_test_module(module)) {
        for function in module
            .functions
            .iter()
            .filter(|function| !has_test_attribute(&function.attributes))
        {
            let qualified_name = format!("{}::{}", module.name, function.name);
            functions.insert(
                qualified_name.clone(),
                FunctionLookup {
                    module_name: module.name.clone(),
                    function_name: function.name.clone(),
                    qualified_name,
                    file_path: module.file_path.clone(),
                    visibility: function.visibility.clone(),
                    is_entry: function.is_entry,
                    is_transaction_callable: function.is_transaction_callable,
                    signature: function.signature.clone(),
                    body: function.body.clone(),
                    attributes: function.attributes.clone(),
                    source: EvidenceSource::Compiler,
                    bytecode: None,
                },
            );
        }
    }

    for function in bytecode_facts
        .modules
        .iter()
        .flat_map(|module| module.functions.iter())
    {
        functions
            .entry(function.qualified_name.clone())
            .and_modify(|existing| {
                existing.visibility = function.visibility.clone();
                existing.is_entry = function.is_entry;
                existing.is_transaction_callable = function.is_transaction_callable;
                existing.source = EvidenceSource::Bytecode;
                existing.bytecode = Some(function.clone());
            })
            .or_insert_with(|| FunctionLookup {
                module_name: function.module_name.clone(),
                function_name: function.function_name.clone(),
                qualified_name: function.qualified_name.clone(),
                file_path: function.full_name.clone(),
                visibility: function.visibility.clone(),
                is_entry: function.is_entry,
                is_transaction_callable: function.is_transaction_callable,
                signature: bytecode_signature(function),
                body: None,
                attributes: Vec::new(),
                source: EvidenceSource::Bytecode,
                bytecode: Some(function.clone()),
            });
    }

    functions
}

fn bytecode_signature(function: &BytecodeFunctionFact) -> String {
    let parameters = function
        .parameter_types
        .iter()
        .enumerate()
        .map(|(index, type_name)| format!("arg{index}: {type_name}"))
        .collect::<Vec<_>>()
        .join(", ");
    let returns = match function.return_types.as_slice() {
        [] => String::new(),
        [single] => format!(": {single}"),
        many => format!(": ({})", many.join(", ")),
    };

    format!("{}({parameters}){returns}", function.function_name)
}

fn capability_findings(
    candidates: &[ObjectCandidate],
    functions: &BTreeMap<String, FunctionLookup>,
) -> Vec<ObjectCapabilityFinding> {
    let mut findings = Vec::new();

    for object in candidates {
        let mut score = 0;
        let mut evidence = Vec::new();
        let mut protected_functions = Vec::new();
        let mut used_in_transaction_callable = false;
        let mut guards_privileged_function = false;
        let mut created_and_transferred_signal = false;
        let has_capability_name = capability_like_name(&object.type_name);

        if has_ability(&object.abilities, "key") {
            score += 2;
            evidence.push(evidence_for_source(object.source, "struct has key ability"));
        }

        if has_capability_name {
            score += 3;
            evidence.push(evidence_for_source(
                object.source,
                "type name follows capability/authority naming pattern",
            ));
        }

        for function in functions.values() {
            let parameter_uses_type =
                function_parameters_contain_type(&function.signature, &object.type_name)
                    || function_parameters_contain_type(
                        &function.signature,
                        &object.qualified_name,
                    )
                    || function
                        .bytecode
                        .as_ref()
                        .is_some_and(|bytecode| bytecode_function_touches_type(bytecode, object));

            if parameter_uses_type && function.is_transaction_callable {
                used_in_transaction_callable = true;
                evidence.push(evidence_for_function(
                    function,
                    format!(
                        "used as a parameter in transaction-callable function {}",
                        function.qualified_name
                    ),
                ));
            }

            if parameter_uses_type && privileged_function(function) {
                guards_privileged_function = true;
                evidence.push(evidence_for_function(
                    function,
                    format!(
                        "guards privileged-looking function {}",
                        function.qualified_name
                    ),
                ));
                protected_functions.push(function.qualified_name.clone());
            }

            if created_and_transferred(function, object) {
                created_and_transferred_signal = true;
                evidence.push(evidence_for_function(
                    function,
                    format!("created and transferred in {}", function.qualified_name),
                ));
            }
        }

        if used_in_transaction_callable {
            score += 2;
        }
        if guards_privileged_function {
            score += 2;
        }
        if created_and_transferred_signal {
            score += 2;
        }

        sort_evidence(&mut evidence);
        protected_functions.sort();
        protected_functions.dedup();

        let confidence = if has_capability_name {
            capability_confidence(score)
        } else {
            ScannerConfidence::Low
        };

        if confidence == ScannerConfidence::Low && evidence.is_empty() {
            continue;
        }

        findings.push(ObjectCapabilityFinding {
            type_name: object.type_name.clone(),
            module_name: object.module_name.clone(),
            qualified_name: object.qualified_name.clone(),
            confidence,
            evidence,
            protected_functions,
        });
    }

    findings.sort_by(|left, right| {
        right
            .confidence
            .rank()
            .cmp(&left.confidence.rank())
            .then_with(|| left.qualified_name.cmp(&right.qualified_name))
    });
    findings
}

fn capability_confidence(score: i32) -> ScannerConfidence {
    if score >= 7 {
        ScannerConfidence::High
    } else if score >= 4 {
        ScannerConfidence::Medium
    } else {
        ScannerConfidence::Low
    }
}

fn object_ownership_findings(
    candidates: &[ObjectCandidate],
    functions: &BTreeMap<String, FunctionLookup>,
    capability_structs: &[String],
) -> Vec<ObjectOwnershipModel> {
    let capability_set = capability_structs.iter().cloned().collect::<BTreeSet<_>>();
    let mut findings = Vec::new();

    for object in candidates {
        if capability_set.contains(&object.qualified_name) {
            continue;
        }

        for kind in [
            ObjectClassification::Shared,
            ObjectClassification::AddressOwned,
            ObjectClassification::Immutable,
            ObjectClassification::Party,
        ] {
            let (evidence, related_functions) = ownership_evidence(functions, object, kind);
            if evidence.is_empty() {
                continue;
            }

            findings.push(ObjectOwnershipModel {
                type_name: object.type_name.clone(),
                module_name: object.module_name.clone(),
                qualified_name: object.qualified_name.clone(),
                ownership_kind: kind,
                confidence: if related_functions.is_empty() {
                    ScannerConfidence::Medium
                } else {
                    ScannerConfidence::High
                },
                evidence,
                related_functions,
                wrapped_types: Vec::new(),
            });
        }
    }

    findings.extend(wrapped_object_findings(candidates, &capability_set));
    findings.sort_by(|left, right| {
        left.ownership_kind
            .cmp(&right.ownership_kind)
            .then_with(|| left.qualified_name.cmp(&right.qualified_name))
    });
    findings
}

fn ownership_evidence(
    functions: &BTreeMap<String, FunctionLookup>,
    object: &ObjectCandidate,
    kind: ObjectClassification,
) -> (Vec<ObjectEvidence>, Vec<String>) {
    let mut evidence = Vec::new();
    let mut related_functions = Vec::new();

    for function in functions.values() {
        if has_test_attribute(&function.attributes) {
            continue;
        }

        let returns_type = function_returns_type(&function.signature, &object.type_name)
            || function_returns_type(&function.signature, &object.qualified_name)
            || function
                .bytecode
                .as_ref()
                .is_some_and(|bytecode| bytecode_returns_type(bytecode, object));
        let matched = match kind {
            ObjectClassification::Shared => function_operation_touches_type(
                function,
                object,
                &[
                    "transfer::share_object",
                    "transfer::public_share_object",
                    "share_object",
                ],
            ),
            ObjectClassification::AddressOwned => {
                function_operation_touches_type(
                    function,
                    object,
                    &[
                        "transfer::transfer",
                        "transfer::public_transfer",
                        "public_transfer",
                    ],
                ) || (function.is_transaction_callable && returns_type)
            }
            ObjectClassification::Immutable => function_operation_touches_type(
                function,
                object,
                &[
                    "transfer::freeze_object",
                    "transfer::public_freeze_object",
                    "freeze_object",
                ],
            ),
            ObjectClassification::Party => function_operation_touches_type(
                function,
                object,
                &[
                    "transfer::party_transfer",
                    "transfer::public_party_transfer",
                    "party_transfer",
                    "party::",
                ],
            ),
            ObjectClassification::Capability | ObjectClassification::Wrapped => false,
        };

        if matched {
            evidence.push(ownership_evidence_label(
                kind,
                function,
                object,
                returns_type,
            ));
            related_functions.push(function.qualified_name.clone());
        }
    }

    sort_evidence(&mut evidence);
    related_functions.sort();
    related_functions.dedup();
    (evidence, related_functions)
}

fn ownership_evidence_label(
    kind: ObjectClassification,
    function: &FunctionLookup,
    object: &ObjectCandidate,
    returns_type: bool,
) -> ObjectEvidence {
    let message = match kind {
        ObjectClassification::AddressOwned if function.is_transaction_callable && returns_type => {
            format!(
                "owned object returned from transaction-callable {}",
                function.qualified_name
            )
        }
        ObjectClassification::Shared => {
            format!(
                "object shared via transfer::share_object in {}",
                function.qualified_name
            )
        }
        ObjectClassification::Immutable => {
            format!(
                "object frozen via transfer::freeze_object in {}",
                function.qualified_name
            )
        }
        ObjectClassification::Party => {
            format!(
                "object moved through party transfer API in {}",
                function.qualified_name
            )
        }
        _ => format!(
            "{} ownership evidence for {} in {}",
            kind.as_str(),
            object.type_name,
            function.qualified_name
        ),
    };

    evidence_for_function(function, message)
}

fn wrapped_object_findings(
    candidates: &[ObjectCandidate],
    capability_structs: &BTreeSet<String>,
) -> Vec<ObjectOwnershipModel> {
    let key_names = candidates
        .iter()
        .map(|object| object.type_name.clone())
        .collect::<HashSet<_>>();
    let mut findings = Vec::new();

    for wrapper in candidates {
        if capability_structs.contains(&wrapper.qualified_name) {
            continue;
        }

        let mut wrapped_types = wrapper
            .fields
            .iter()
            .filter_map(|field| {
                key_names
                    .iter()
                    .find(|key_name| type_reference_matches(&field.type_name, key_name))
                    .cloned()
            })
            .collect::<Vec<_>>();
        wrapped_types.sort();
        wrapped_types.dedup();

        if wrapped_types.is_empty() {
            continue;
        }

        findings.push(ObjectOwnershipModel {
            type_name: wrapper.type_name.clone(),
            module_name: wrapper.module_name.clone(),
            qualified_name: wrapper.qualified_name.clone(),
            ownership_kind: ObjectClassification::Wrapped,
            confidence: ScannerConfidence::High,
            evidence: vec![evidence_for_source(
                wrapper.source,
                "struct stores another key object type as a field",
            )],
            related_functions: Vec::new(),
            wrapped_types,
        });
    }

    findings
}

fn object_lifecycle_maps(
    candidates: &[ObjectCandidate],
    functions: &BTreeMap<String, FunctionLookup>,
    capability_structs: &[String],
) -> Vec<ObjectLifecycleModel> {
    let reverse_call_graph = reverse_call_graph(functions);
    let direct_events = direct_lifecycle_events(candidates, functions);
    let wrapper_evidence = wrapper_evidence(candidates);
    let capability_set = capability_structs.iter().cloned().collect::<BTreeSet<_>>();
    let mut maps = Vec::new();

    for object in candidates {
        let mut stage_functions =
            BTreeMap::<ObjectLifecycleStageKind, Vec<ObjectLifecycleFunctionRef>>::new();
        let mut stage_evidence =
            BTreeMap::<ObjectLifecycleStageKind, BTreeSet<ObjectEvidence>>::new();

        for (stage, evidence) in wrapper_evidence
            .get(&object.qualified_name)
            .into_iter()
            .flat_map(|evidence| evidence.iter())
        {
            stage_evidence
                .entry(*stage)
                .or_default()
                .insert(evidence.clone());
        }

        for event in direct_events
            .iter()
            .filter(|event| event.object_key == object.qualified_name)
        {
            let Some(lookup) = functions.get(&event.function_key) else {
                continue;
            };

            push_stage_function(
                &mut stage_functions,
                event.stage,
                lifecycle_function_ref(lookup, true, Vec::new(), vec![event.evidence.clone()]),
            );
            stage_evidence
                .entry(event.stage)
                .or_default()
                .insert(event.evidence.clone());

            for indirect in indirect_callers(&event.function_key, &reverse_call_graph) {
                let Some(lookup) = functions.get(&indirect.caller_key) else {
                    continue;
                };
                let evidence =
                    evidence_for_function(lookup, format!("calls {}", event.function_key));

                push_stage_function(
                    &mut stage_functions,
                    event.stage,
                    lifecycle_function_ref(
                        lookup,
                        false,
                        indirect.call_path,
                        vec![evidence.clone()],
                    ),
                );
                stage_evidence
                    .entry(event.stage)
                    .or_default()
                    .insert(evidence);
            }
        }

        let mut stages = stage_evidence
            .into_iter()
            .map(|(kind, evidence)| {
                let mut functions = stage_functions.remove(&kind).unwrap_or_default();
                sort_function_refs(&mut functions);

                ObjectLifecycleStageModel {
                    kind,
                    functions,
                    evidence: evidence.into_iter().collect(),
                }
            })
            .collect::<Vec<_>>();
        stages.sort_by_key(|stage| stage_rank(stage.kind));

        let mut touched_by = stages
            .iter()
            .flat_map(|stage| stage.functions.iter().cloned())
            .collect::<Vec<_>>();
        sort_function_refs(&mut touched_by);
        touched_by.dedup();

        maps.push(ObjectLifecycleModel {
            type_name: object.type_name.clone(),
            module_name: object.module_name.clone(),
            qualified_name: object.qualified_name.clone(),
            file_path: object.file_path.clone(),
            abilities: object.abilities.clone(),
            is_capability_like: capability_set.contains(&object.qualified_name)
                || capability_like_name(&object.type_name),
            stages,
            touched_by,
        });
    }

    maps.sort_by(|left, right| {
        left.file_path
            .cmp(&right.file_path)
            .then_with(|| left.qualified_name.cmp(&right.qualified_name))
    });
    maps
}

fn reverse_call_graph(
    functions: &BTreeMap<String, FunctionLookup>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut reverse = BTreeMap::<String, BTreeSet<String>>::new();

    for (caller_key, caller) in functions {
        for (callee_key, callee) in functions {
            if caller_key == callee_key {
                continue;
            }

            if function_calls_function(caller, callee) {
                reverse
                    .entry(callee_key.clone())
                    .or_default()
                    .insert(caller_key.clone());
            }
        }
    }

    reverse
}

fn function_calls_function(caller: &FunctionLookup, callee: &FunctionLookup) -> bool {
    if caller.body.as_deref().is_some_and(|body| {
        body_calls_function(
            body,
            &caller.module_name,
            &callee.module_name,
            &callee.function_name,
        )
    }) {
        return true;
    }

    caller.bytecode.as_ref().is_some_and(|bytecode| {
        bytecode.calls.iter().any(|target| {
            target.ends_with(&format!(
                "::{}::{}",
                callee.module_name, callee.function_name
            ))
        })
    })
}

fn direct_lifecycle_events(
    candidates: &[ObjectCandidate],
    functions: &BTreeMap<String, FunctionLookup>,
) -> Vec<DirectEvent> {
    let mut events = Vec::new();
    let mut seen = BTreeSet::new();

    for object in candidates {
        for (function_key, lookup) in functions {
            for (stage, evidence) in function_lifecycle_events(object, lookup) {
                let key = format!(
                    "{}::{}::{}::{}",
                    object.qualified_name,
                    stage.as_str(),
                    function_key,
                    evidence.message()
                );

                if !seen.insert(key) {
                    continue;
                }

                events.push(DirectEvent {
                    object_key: object.qualified_name.clone(),
                    stage,
                    function_key: function_key.clone(),
                    evidence,
                });
            }
        }
    }

    events
}

fn function_lifecycle_events(
    object: &ObjectCandidate,
    function: &FunctionLookup,
) -> Vec<(ObjectLifecycleStageKind, ObjectEvidence)> {
    let mut events = Vec::new();
    let returns_type = function_returns_type(&function.signature, &object.type_name)
        || function_returns_type(&function.signature, &object.qualified_name)
        || function
            .bytecode
            .as_ref()
            .is_some_and(|bytecode| bytecode_returns_type(bytecode, object));
    let constructs_type = function.body.as_deref().is_some_and(|body| {
        body_constructs_type(body, &object.type_name)
            || body_constructs_type(body, &object.qualified_name)
    }) || function
        .bytecode
        .as_ref()
        .is_some_and(|bytecode| bytecode_packs_type(bytecode, object));
    let has_object_new = function
        .body
        .as_deref()
        .is_some_and(|body| body.to_ascii_lowercase().contains("object::new"))
        || function.bytecode.as_ref().is_some_and(|bytecode| {
            bytecode
                .calls
                .iter()
                .any(|target| target.to_ascii_lowercase().contains("::object::new"))
        });

    if constructs_type && (has_object_new || returns_type) {
        events.push((
            ObjectLifecycleStageKind::Created,
            evidence_for_function(
                function,
                format!(
                    "{} constructed in {}",
                    object.type_name, function.qualified_name
                ),
            ),
        ));
    }

    if function_mutably_touches_type(&function.signature, &object.type_name)
        || function_mutably_touches_type(&function.signature, &object.qualified_name)
        || function.bytecode.as_ref().is_some_and(|bytecode| {
            bytecode.parameter_types.iter().any(|parameter| {
                parameter.starts_with("&mut") && bytecode_type_matches(parameter, object)
            })
        })
    {
        events.push((
            ObjectLifecycleStageKind::Mutated,
            evidence_for_function(
                function,
                format!(
                    "{} takes &mut {}",
                    function.qualified_name, object.type_name
                ),
            ),
        ));
    }

    if let Some(body) = function.body.as_deref() {
        if borrowed_identity_mutates_related_state(
            body,
            &function.signature,
            &object.type_name,
            &object.qualified_name,
        ) {
            events.push((
                ObjectLifecycleStageKind::Mutated,
                evidence_for_function(
                    function,
                    format!(
                        "{} mutates state keyed by {} identity",
                        function.qualified_name, object.type_name
                    ),
                ),
            ));
        }
    }

    if function_operation_touches_type(
        function,
        object,
        &[
            "transfer::transfer",
            "transfer::public_transfer",
            "public_transfer",
        ],
    ) {
        events.push((
            ObjectLifecycleStageKind::Transferred,
            evidence_for_function(
                function,
                format!("ownership transferred in {}", function.qualified_name),
            ),
        ));
        events.push((
            ObjectLifecycleStageKind::Owned,
            evidence_for_function(
                function,
                format!("address-owned object path in {}", function.qualified_name),
            ),
        ));
    }

    if function.is_transaction_callable && returns_type {
        events.push((
            ObjectLifecycleStageKind::Transferred,
            evidence_for_function(
                function,
                format!(
                    "returned to transaction caller from {}",
                    function.qualified_name
                ),
            ),
        ));
        events.push((
            ObjectLifecycleStageKind::Owned,
            evidence_for_function(
                function,
                format!(
                    "returned from transaction-callable {}",
                    function.qualified_name
                ),
            ),
        ));
    }

    if function_operation_touches_type(
        function,
        object,
        &[
            "transfer::share_object",
            "transfer::public_share_object",
            "share_object",
        ],
    ) {
        events.push((
            ObjectLifecycleStageKind::Shared,
            evidence_for_function(
                function,
                format!(
                    "shared via transfer::share_object in {}",
                    function.qualified_name
                ),
            ),
        ));
    }

    if function_operation_touches_type(
        function,
        object,
        &[
            "transfer::freeze_object",
            "transfer::public_freeze_object",
            "freeze_object",
        ],
    ) {
        events.push((
            ObjectLifecycleStageKind::Immutable,
            evidence_for_function(
                function,
                format!(
                    "frozen via transfer::freeze_object in {}",
                    function.qualified_name
                ),
            ),
        ));
    }

    if function_operation_touches_type(
        function,
        object,
        &[
            "transfer::party_transfer",
            "transfer::public_party_transfer",
            "party_transfer",
            "party::",
        ],
    ) {
        events.push((
            ObjectLifecycleStageKind::Party,
            evidence_for_function(
                function,
                format!(
                    "moved through party ownership API in {}",
                    function.qualified_name
                ),
            ),
        ));
    }

    if function.body.as_deref().is_some_and(|body| {
        delete_touches_type(
            body,
            &function.signature,
            &object.type_name,
            &object.qualified_name,
        )
    }) || function.bytecode.as_ref().is_some_and(|bytecode| {
        bytecode
            .unpacks
            .iter()
            .any(|target| bytecode_type_matches(target, object))
    }) {
        events.push((
            ObjectLifecycleStageKind::Deleted,
            evidence_for_function(
                function,
                format!("delete signal observed in {}", function.qualified_name),
            ),
        ));
    }

    if function_operation_touches_type(
        function,
        object,
        &[
            "dynamic_field::add",
            "dynamic_object_field::add",
            "table::add",
            "bag::add",
        ],
    ) {
        events.push((
            ObjectLifecycleStageKind::Wrapped,
            evidence_for_function(
                function,
                format!(
                    "stored through dynamic object storage in {}",
                    function.qualified_name
                ),
            ),
        ));
    }

    events
}

fn function_operation_touches_type(
    function: &FunctionLookup,
    object: &ObjectCandidate,
    operations: &[&str],
) -> bool {
    if function.body.as_deref().is_some_and(|body| {
        operation_touches_type(
            body,
            &function.signature,
            &object.type_name,
            &object.qualified_name,
            operations,
        )
    }) {
        return true;
    }

    function.bytecode.as_ref().is_some_and(|bytecode| {
        bytecode_has_operation(bytecode, operations)
            && bytecode_function_touches_type(bytecode, object)
    })
}

fn bytecode_has_operation(function: &BytecodeFunctionFact, operations: &[&str]) -> bool {
    function.calls.iter().any(|target| {
        let target = target.to_ascii_lowercase();
        operations
            .iter()
            .any(|operation| target.contains(&operation.to_ascii_lowercase()))
    })
}

fn bytecode_function_touches_type(
    function: &BytecodeFunctionFact,
    object: &ObjectCandidate,
) -> bool {
    function
        .parameter_types
        .iter()
        .chain(function.return_types.iter())
        .chain(function.packs.iter())
        .chain(function.unpacks.iter())
        .any(|type_name| bytecode_type_matches(type_name, object))
}

fn bytecode_returns_type(function: &BytecodeFunctionFact, object: &ObjectCandidate) -> bool {
    function
        .return_types
        .iter()
        .any(|type_name| bytecode_type_matches(type_name, object))
}

fn bytecode_packs_type(function: &BytecodeFunctionFact, object: &ObjectCandidate) -> bool {
    function
        .packs
        .iter()
        .any(|type_name| bytecode_type_matches(type_name, object))
}

fn bytecode_type_matches(type_name: &str, object: &ObjectCandidate) -> bool {
    type_reference_matches(type_name, &object.type_name)
        || type_reference_matches(type_name, &object.qualified_name)
        || type_name.ends_with(&format!("::{}::{}", object.module_name, object.type_name))
}

fn wrapper_evidence(
    objects: &[ObjectCandidate],
) -> BTreeMap<String, Vec<(ObjectLifecycleStageKind, ObjectEvidence)>> {
    let mut evidence = BTreeMap::<String, Vec<(ObjectLifecycleStageKind, ObjectEvidence)>>::new();

    for object in objects {
        for wrapper in objects {
            if object.qualified_name == wrapper.qualified_name {
                continue;
            }

            for field in &wrapper.fields {
                if type_reference_matches(&field.type_name, &object.type_name)
                    || type_reference_matches(&field.type_name, &object.qualified_name)
                {
                    evidence
                        .entry(object.qualified_name.clone())
                        .or_default()
                        .push((
                            ObjectLifecycleStageKind::Wrapped,
                            evidence_for_source(
                                wrapper.source,
                                format!(
                                    "stored in {}::{}.{}",
                                    wrapper.module_name, wrapper.type_name, field.name
                                ),
                            ),
                        ));
                }
            }
        }
    }

    evidence
}

fn indirect_callers(
    direct_key: &str,
    reverse_call_graph: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<IndirectCaller> {
    let mut callers = Vec::new();
    let mut queue = VecDeque::from([(
        direct_key.to_string(),
        vec![direct_key.to_string()],
        0_usize,
    )]);
    let mut visited = BTreeSet::new();

    while let Some((target, path_to_direct, depth)) = queue.pop_front() {
        if depth >= CALL_GRAPH_DEPTH {
            continue;
        }

        let Some(next_callers) = reverse_call_graph.get(&target) else {
            continue;
        };

        for caller_key in next_callers {
            if caller_key == direct_key || !visited.insert(caller_key.clone()) {
                continue;
            }

            let mut call_path = vec![caller_key.clone()];
            call_path.extend(path_to_direct.clone());
            callers.push(IndirectCaller {
                caller_key: caller_key.clone(),
                call_path: call_path.clone(),
            });
            queue.push_back((caller_key.clone(), call_path, depth + 1));
        }
    }

    callers
}

fn lifecycle_function_ref(
    function: &FunctionLookup,
    direct: bool,
    call_path: Vec<String>,
    evidence: Vec<ObjectEvidence>,
) -> ObjectLifecycleFunctionRef {
    ObjectLifecycleFunctionRef {
        module_name: function.module_name.clone(),
        function_name: function.function_name.clone(),
        qualified_name: function.qualified_name.clone(),
        file_path: function.file_path.clone(),
        visibility: function.visibility.clone(),
        is_entry: function.is_entry,
        is_transaction_callable: function.is_transaction_callable,
        direct,
        call_path,
        evidence,
    }
}

fn push_stage_function(
    stage_functions: &mut BTreeMap<ObjectLifecycleStageKind, Vec<ObjectLifecycleFunctionRef>>,
    stage: ObjectLifecycleStageKind,
    function_ref: ObjectLifecycleFunctionRef,
) {
    let functions = stage_functions.entry(stage).or_default();

    if let Some(existing) = functions.iter_mut().find(|existing| {
        existing.qualified_name == function_ref.qualified_name
            && existing.direct == function_ref.direct
            && existing.call_path == function_ref.call_path
    }) {
        for evidence in function_ref.evidence {
            if !existing.evidence.contains(&evidence) {
                existing.evidence.push(evidence);
            }
        }
        return;
    }

    functions.push(function_ref);
}

fn shared_object_structs(
    candidates: &[ObjectCandidate],
    functions: &BTreeMap<String, FunctionLookup>,
    capability_structs: &[String],
) -> Vec<String> {
    let capability_set = capability_structs.iter().cloned().collect::<BTreeSet<_>>();
    let shared_mentions = shared_object_mentions(functions, candidates);
    let mut shared = candidates
        .iter()
        .filter(|object| {
            !capability_set.contains(&object.qualified_name)
                && has_ability(&object.abilities, "key")
                && !capability_like_name(&object.type_name)
                && (shared_mentions.is_empty()
                    || shared_mentions.contains(&object.type_name)
                    || shared_mentions.contains(&object.qualified_name))
        })
        .map(|object| object.qualified_name.clone())
        .collect::<Vec<_>>();
    shared.sort();
    shared.dedup();
    shared
}

fn shared_object_mentions(
    functions: &BTreeMap<String, FunctionLookup>,
    candidates: &[ObjectCandidate],
) -> HashSet<String> {
    let mut mentions = HashSet::new();

    for function in functions.values() {
        let Some(body) = function.body.as_deref() else {
            continue;
        };

        if !body.contains("share_object") {
            continue;
        }

        for object in candidates {
            if body.contains(&object.type_name) {
                mentions.insert(object.type_name.clone());
                mentions.insert(object.qualified_name.clone());
            }
        }
    }

    mentions
}

fn evidence_for_source(source: EvidenceSource, message: impl Into<String>) -> ObjectEvidence {
    match source {
        EvidenceSource::Bytecode => ObjectEvidence::bytecode(message),
        EvidenceSource::Compiler => ObjectEvidence {
            source: EvidenceSource::Compiler,
            confidence: ScannerConfidence::High,
            message: message.into(),
        },
        _ => ObjectEvidence::source(message),
    }
}

fn evidence_for_function(function: &FunctionLookup, message: impl Into<String>) -> ObjectEvidence {
    evidence_for_source(function.source, message)
}

fn sort_evidence(evidence: &mut Vec<ObjectEvidence>) {
    evidence.sort();
    evidence.dedup();
}

fn sort_function_refs(functions: &mut Vec<ObjectLifecycleFunctionRef>) {
    functions.sort_by(|left, right| {
        right
            .direct
            .cmp(&left.direct)
            .then_with(|| left.file_path.cmp(&right.file_path))
            .then_with(|| left.qualified_name.cmp(&right.qualified_name))
            .then_with(|| left.call_path.cmp(&right.call_path))
    });
}

fn stage_rank(kind: ObjectLifecycleStageKind) -> usize {
    STAGE_ORDER
        .iter()
        .position(|candidate| *candidate == kind)
        .unwrap_or(STAGE_ORDER.len())
}

fn privileged_function(function: &FunctionLookup) -> bool {
    let name = function.function_name.to_ascii_lowercase();
    let body = function.body.as_deref().unwrap_or("").to_ascii_lowercase();
    const PRIVILEGED_TERMS: &[&str] = &[
        "admin", "burn", "claim", "config", "create", "destroy", "fee", "mint", "owner", "pause",
        "set", "transfer", "treasury", "unpause", "update", "upgrade", "withdraw",
    ];
    const PRIVILEGED_BODY_TERMS: &[&str] = &[
        "balance::",
        "coin::",
        "dynamic_field",
        "event::emit",
        "object::new",
        "share_object",
        "transfer::",
        "tx_context::sender",
    ];

    PRIVILEGED_TERMS.iter().any(|term| name.contains(term))
        || PRIVILEGED_BODY_TERMS.iter().any(|term| body.contains(term))
        || function.bytecode.as_ref().is_some_and(|bytecode| {
            bytecode.calls.iter().any(|target| {
                let target = target.to_ascii_lowercase();
                PRIVILEGED_BODY_TERMS
                    .iter()
                    .any(|term| target.contains(term))
            })
        })
}

fn created_and_transferred(function: &FunctionLookup, object: &ObjectCandidate) -> bool {
    if function.body.as_deref().is_some_and(|body| {
        let lower_body = body.to_ascii_lowercase();

        body.contains(&object.type_name)
            && lower_body.contains("object::new")
            && (lower_body.contains("transfer::transfer")
                || lower_body.contains("transfer::public_transfer")
                || lower_body.contains("share_object"))
    }) {
        return true;
    }

    function.bytecode.as_ref().is_some_and(|bytecode| {
        bytecode_packs_type(bytecode, object)
            && bytecode
                .calls
                .iter()
                .any(|target| target.contains("::transfer::") || target.contains("share_object"))
    })
}

fn function_parameters_contain_type(signature: &str, type_name: &str) -> bool {
    let Some(parameters) = function_parameters(signature) else {
        return false;
    };

    type_reference_matches(parameters, type_name)
}

fn function_mutably_touches_type(signature: &str, type_name: &str) -> bool {
    let Some(parameters) = function_parameters(signature) else {
        return false;
    };

    split_top_level(parameters, ',')
        .into_iter()
        .any(|parameter| {
            let Some((_, parameter_type)) = parameter.split_once(':') else {
                return false;
            };
            let parameter_type = parameter_type.trim();

            parameter_type.starts_with("&mut") && type_reference_matches(parameter_type, type_name)
        })
}

fn borrowed_identity_mutates_related_state(
    body: &str,
    signature: &str,
    type_name: &str,
    qualified_name: &str,
) -> bool {
    let body_block = function_body_block(body);
    let borrowed_names = borrowed_parameter_names(signature, type_name)
        .into_iter()
        .chain(borrowed_parameter_names(signature, qualified_name))
        .collect::<BTreeSet<_>>();

    if borrowed_names.is_empty() {
        return false;
    }

    let identity_names = object_identity_names(body_block, &borrowed_names);

    if identity_names.is_empty() {
        return false;
    }

    body_block.split(';').any(|statement| {
        identity_names
            .iter()
            .any(|identity| source_contains_identifier(statement, identity))
            && statement_has_mutation_signal(statement)
    })
}

fn function_body_block(function_source: &str) -> &str {
    let Some(start) = function_source.find('{') else {
        return function_source;
    };
    let Some(end) = function_source.rfind('}') else {
        return &function_source[start + 1..];
    };

    if start < end {
        &function_source[start + 1..end]
    } else {
        function_source
    }
}

fn borrowed_parameter_names(signature: &str, type_name: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(parameters) = function_parameters(signature) else {
        return names;
    };

    for parameter in split_top_level(parameters, ',') {
        let Some((name, parameter_type)) = parameter.split_once(':') else {
            continue;
        };
        let parameter_type = parameter_type.trim();

        if !parameter_type.starts_with('&')
            || parameter_type.starts_with("&mut")
            || !type_reference_matches(parameter_type, type_name)
        {
            continue;
        }

        let name = name
            .split_whitespace()
            .last()
            .unwrap_or("")
            .trim_matches(|character: char| !is_identifier_character(character));

        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }

    names
}

fn object_identity_names(body: &str, object_names: &BTreeSet<String>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for statement in body.split(';') {
        let Some((left, right)) = statement.split_once('=') else {
            continue;
        };

        if !left.contains("let") {
            continue;
        }

        let derives_identity = object_names.iter().any(|object_name| {
            right.contains(&format!("object::id({object_name})"))
                || right.contains(&format!("object::id(&{object_name})"))
                || right.contains(&format!("{object_name}.id"))
        });

        if !derives_identity {
            continue;
        }

        let Some(raw_name) = left
            .split("let")
            .last()
            .and_then(|binding| binding.split(':').next())
        else {
            continue;
        };

        let name = raw_name
            .split_whitespace()
            .filter(|part| *part != "mut")
            .next_back()
            .unwrap_or("")
            .trim_matches(|character: char| !is_identifier_character(character));

        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }

    names
}

fn statement_has_mutation_signal(statement: &str) -> bool {
    let statement = statement.to_ascii_lowercase();
    const MUTATION_SIGNALS: &[&str] = &[
        "&mut",
        "_mut",
        "borrow_mut",
        "set_",
        "add_",
        "remove",
        "delete",
        "destroy",
        "insert",
        "push_back",
        ".add(",
        "::add(",
        ".remove(",
        "::remove(",
        "deposit",
        "withdraw",
        "supply",
        "split(",
        "decrease",
        "increase",
        "increment",
        "decrement",
        "latch",
        "refresh",
    ];

    MUTATION_SIGNALS
        .iter()
        .any(|signal| statement.contains(signal))
}

fn function_parameters(signature: &str) -> Option<&str> {
    let start = signature.find('(')?;
    let mut depth = 0_i32;

    for (offset, character) in signature[start..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;

                if depth == 0 {
                    return Some(&signature[start + 1..start + offset]);
                }
            }
            _ => {}
        }
    }

    None
}

fn function_returns_type(signature: &str, type_name: &str) -> bool {
    if type_name.is_empty() {
        return false;
    }

    let Some(close_parameters) = signature.rfind(')') else {
        return false;
    };

    let after_parameters = signature[close_parameters + 1..].trim_start();
    let Some(return_type) = after_parameters.strip_prefix(':') else {
        return false;
    };

    type_reference_matches(return_type, type_name)
}

fn type_reference_matches(source: &str, type_name: &str) -> bool {
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);

    source
        .split(|character: char| {
            character.is_whitespace()
                || matches!(
                    character,
                    '&' | ',' | ':' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ';' | '='
                )
        })
        .any(|token| token == short_name || token == type_name)
}

fn body_constructs_type(body: &str, type_name: &str) -> bool {
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);

    body.contains(&format!("{short_name} {{"))
        || body.contains(&format!("{short_name}<"))
        || body.contains(&format!("{type_name} {{"))
        || body.contains(&format!("{type_name}<"))
}

fn operation_touches_type(
    body: &str,
    signature: &str,
    type_name: &str,
    qualified_name: &str,
    operations: &[&str],
) -> bool {
    let value_names = owned_or_constructed_value_names(body, signature, type_name, qualified_name);

    operation_call_snippets(body, operations)
        .iter()
        .any(|snippet| {
            body_constructs_type(snippet, type_name)
                || body_constructs_type(snippet, qualified_name)
                || value_names
                    .iter()
                    .any(|value_name| source_contains_identifier(snippet, value_name))
        })
}

fn delete_touches_type(body: &str, signature: &str, type_name: &str, qualified_name: &str) -> bool {
    let lower_body = body.to_ascii_lowercase();

    if !is_delete_signal(&lower_body) {
        return false;
    }

    let value_names = owned_or_constructed_value_names(body, signature, type_name, qualified_name);
    let destructures_type =
        body_destructures_type(body, type_name) || body_destructures_type(body, qualified_name);

    destructures_type
        || value_names
            .iter()
            .any(|value_name| source_contains_identifier(body, value_name))
}

fn owned_or_constructed_value_names(
    body: &str,
    signature: &str,
    type_name: &str,
    qualified_name: &str,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    names.extend(owned_parameter_names(signature, type_name));
    names.extend(owned_parameter_names(signature, qualified_name));
    names.extend(constructed_value_names(body, type_name));
    names.extend(constructed_value_names(body, qualified_name));
    names
}

fn owned_parameter_names(signature: &str, type_name: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(parameters) = function_parameters(signature) else {
        return names;
    };

    for parameter in split_top_level(parameters, ',') {
        let Some((name, parameter_type)) = parameter.split_once(':') else {
            continue;
        };
        let parameter_type = parameter_type.trim();

        if parameter_type.starts_with('&') || !type_reference_matches(parameter_type, type_name) {
            continue;
        }

        let name = name
            .split_whitespace()
            .last()
            .unwrap_or("")
            .trim_matches(|character: char| !is_identifier_character(character));

        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }

    names
}

fn constructed_value_names(body: &str, type_name: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for statement in body.split(';') {
        let Some((left, right)) = statement.split_once('=') else {
            continue;
        };

        if !left.contains("let") || !body_constructs_type(right, type_name) {
            continue;
        }

        let Some(raw_name) = left
            .split("let")
            .last()
            .and_then(|binding| binding.split(':').next())
        else {
            continue;
        };

        let name = raw_name
            .split_whitespace()
            .filter(|part| *part != "mut")
            .next_back()
            .unwrap_or("")
            .trim_matches(|character: char| !is_identifier_character(character));

        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }

    names
}

fn body_destructures_type(body: &str, type_name: &str) -> bool {
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);

    body.contains(&format!("let {short_name} {{"))
        || body.contains(&format!("let {short_name}<"))
        || body.contains(&format!("let {type_name} {{"))
        || body.contains(&format!("let {type_name}<"))
}

fn operation_call_snippets(body: &str, operations: &[&str]) -> Vec<String> {
    let lower_body = body.to_ascii_lowercase();
    let mut snippets = Vec::new();

    for operation in operations {
        let operation = operation.to_ascii_lowercase();
        let mut search_start = 0;

        while let Some(relative_start) = lower_body[search_start..].find(&operation) {
            let start = search_start + relative_start;
            let end = lower_body[start..]
                .find(';')
                .map(|offset| start + offset)
                .unwrap_or(body.len());

            snippets.push(body[start..end].to_string());
            search_start = end.saturating_add(1);
        }
    }

    snippets
}

fn split_top_level(source: &str, delimiter: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut angle_depth = 0_i32;
    let mut paren_depth = 0_i32;

    for (index, character) in source.char_indices() {
        match character {
            '<' => angle_depth += 1,
            '>' => angle_depth -= 1,
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            _ if character == delimiter && angle_depth == 0 && paren_depth == 0 => {
                parts.push(source[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }

    parts.push(source[start..].trim());
    parts
}

fn source_contains_identifier(source: &str, identifier: &str) -> bool {
    source
        .split(|character: char| !is_identifier_character(character))
        .any(|token| token == identifier)
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn body_calls_function(
    body: &str,
    caller_module: &str,
    callee_module: &str,
    callee_name: &str,
) -> bool {
    let body = function_body_block(body);
    let qualified = format!("{callee_module}::{callee_name}");
    let qualified_call =
        body.contains(&format!("{qualified}(")) || body.contains(&format!("{qualified}<"));
    let same_module_call = caller_module == callee_module
        && (body.contains(&format!("{callee_name}(")) || body.contains(&format!("{callee_name}<")));

    qualified_call || same_module_call
}

fn is_delete_signal(lower_body: &str) -> bool {
    lower_body.contains(".delete(")
        || lower_body.contains("object::delete")
        || lower_body.contains("id.delete")
        || lower_body.contains("uid.delete")
}

fn has_ability(abilities: &[String], ability: &str) -> bool {
    abilities.iter().any(|candidate| candidate == ability)
}

fn struct_has_ability(move_struct: &MoveStructSignature, ability: &str) -> bool {
    move_struct
        .abilities
        .iter()
        .any(|candidate| candidate == ability)
}

fn is_test_module(module: &MoveModule) -> bool {
    module
        .file_path
        .split('/')
        .any(|segment| segment == "tests")
        || has_test_attribute(&module.attributes)
}

fn has_test_attribute(attributes: &[String]) -> bool {
    attributes.iter().any(|attribute| {
        let attribute = attribute.to_ascii_lowercase();
        attribute.contains("test")
            || attribute.contains("test_only")
            || attribute.contains("random_test")
            || attribute.contains("expected_failure")
    })
}

fn capability_like_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();

    name.ends_with("cap")
        || name.ends_with("capability")
        || name.contains("_cap")
        || name.contains("admin")
        || name.contains("authority")
        || name.contains("owner")
        || name.contains("publisher")
        || name.contains("operator")
        || name.contains("guardian")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use peregrine_types::sui::move_model::{parse_module_declarations, MovePackageModel};

    use super::*;
    use crate::core::{ScanInput, SourceMode};

    fn package_from_source(source: &str) -> MovePackageModel {
        let root = Path::new("/workspace");
        let modules =
            parse_module_declarations(source, root, Path::new("/workspace/sources/main.move"));
        MovePackageModel {
            name: "test_package".to_string(),
            path: String::new(),
            manifest_path: "Move.toml".to_string(),
            modules,
        }
    }

    fn scan_source(source: &str) -> ObjectScanReport {
        let package = package_from_source(source);
        let input = ScanInput {
            package_model: &package,
            package_root: None,
            build_root: None,
            source_mode: SourceMode::SourceOnly,
        };
        scan_objects(&input)
    }

    #[test]
    fn detects_objects_from_bytecode_when_source_is_unavailable() {
        let package = MovePackageModel {
            name: "struct_abilities".to_string(),
            path: String::new(),
            manifest_path: "Move.toml".to_string(),
            modules: Vec::new(),
        };
        let build_root = Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../peregrine-indexer/tests/fixtures/sui/struct_abilities/build/struct_abilities",
        );
        let input = ScanInput {
            package_model: &package,
            package_root: None,
            build_root: Some(build_root),
            source_mode: SourceMode::BestAvailable,
        };
        let report = scan_objects(&input);

        let lifecycle = report
            .lifecycle_maps
            .iter()
            .find(|map| map.qualified_name == "main::KeyObject")
            .expect("bytecode-backed key object");
        assert!(lifecycle
            .stages
            .iter()
            .any(|stage| stage.kind == ObjectLifecycleStageKind::Created));
        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("compiled bytecode")));
    }

    #[test]
    fn classifies_capability_objects() {
        let report = scan_source(
            r#"
module demo::admin {
    public struct AdminCap has key, store { id: UID }

    public entry fun update(cap: &AdminCap) {
        assert!(true, 0);
    }
}
"#,
        );

        let cap = report
            .capability_findings
            .iter()
            .find(|finding| finding.qualified_name == "admin::AdminCap")
            .expect("capability finding");
        assert_eq!(cap.confidence, ScannerConfidence::High);
    }

    #[test]
    fn extracts_source_lifecycle_stages() {
        let report = scan_source(
            r#"
module demo::vault {
    public struct Vault has key, store { id: UID }

    public entry fun create(ctx: &mut TxContext) {
        let vault = Vault { id: object::new(ctx) };
        transfer::share_object(vault);
    }
}
"#,
        );

        let lifecycle = report
            .lifecycle_maps
            .iter()
            .find(|map| map.qualified_name == "vault::Vault")
            .expect("vault lifecycle");
        let stages = lifecycle
            .stages
            .iter()
            .map(|stage| stage.kind)
            .collect::<Vec<_>>();

        assert!(stages.contains(&ObjectLifecycleStageKind::Created));
        assert!(stages.contains(&ObjectLifecycleStageKind::Shared));
    }

    #[test]
    fn falls_back_when_bytecode_is_missing() {
        let package = package_from_source(
            r#"
module demo::objects {
    public struct OwnedObject has key, store { id: UID }
}
"#,
        );
        let input = ScanInput {
            package_model: &package,
            package_root: None,
            build_root: None,
            source_mode: SourceMode::BestAvailable,
        };
        let report = scan_objects(&input);

        assert!(report
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("no compiled bytecode")));
        assert!(report
            .lifecycle_maps
            .iter()
            .any(|map| map.qualified_name == "objects::OwnedObject"));
    }

    #[test]
    fn detects_wrapped_objects() {
        let report = scan_source(
            r#"
module demo::wrap {
    public struct Inner has key, store { id: UID }
    public struct Wrapper has key, store { id: UID, inner: Inner }
}
"#,
        );

        assert!(report.ownership_findings.iter().any(|finding| {
            finding.qualified_name == "wrap::Wrapper"
                && finding.ownership_kind == ObjectClassification::Wrapped
        }));
        assert!(report
            .lifecycle_maps
            .iter()
            .find(|map| map.qualified_name == "wrap::Inner")
            .expect("inner lifecycle")
            .stages
            .iter()
            .any(|stage| stage.kind == ObjectLifecycleStageKind::Wrapped));
    }
}
