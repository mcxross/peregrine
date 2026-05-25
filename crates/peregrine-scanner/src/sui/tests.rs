use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use peregrine_move_model::{parse_module_declarations, MoveFunctionSignature, MoveModule};
use serde::Serialize;
use walkdir::{DirEntry, WalkDir};

use crate::core::{
    EvidenceSource, PackageScanner, ScanInput, ScannerConfidence, ScannerDiagnostic, ScannerOutput,
};

pub const TESTS_SCANNER_ID: &str = "sui.tests";

pub struct TestsScanner;

impl PackageScanner for TestsScanner {
    fn id(&self) -> &'static str {
        TESTS_SCANNER_ID
    }

    fn scan(&self, input: &ScanInput<'_>) -> ScannerOutput {
        ScannerOutput::Tests(scan_tests(input))
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestsScanReport {
    pub has_unit_tests: bool,
    pub has_movy_invariant_tests: bool,
    pub has_formal_prover_specs: bool,
    pub unit_test_count: usize,
    pub movy_invariant_test_count: usize,
    pub formal_prover_spec_count: usize,
    pub unit_tests: Vec<UnitTestFinding>,
    pub movy_invariant_tests: Vec<MovyInvariantFinding>,
    pub formal_prover_specs: Vec<FormalProverSpecFinding>,
    pub diagnostics: Vec<ScannerDiagnostic>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitTestFinding {
    pub module_name: String,
    pub function_name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub source_folder: String,
    pub is_random_test: bool,
    pub expected_failure: bool,
    pub confidence: ScannerConfidence,
    pub evidence: Vec<TestsEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovyInvariantFinding {
    pub module_name: String,
    pub function_name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub hook_kind: MovyHookKind,
    pub target_function: Option<String>,
    pub confidence: ScannerConfidence,
    pub evidence: Vec<TestsEvidence>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FormalProverSpecFinding {
    pub spec_kind: FormalSpecKind,
    pub module_name: String,
    pub function_name: Option<String>,
    pub qualified_name: String,
    pub file_path: String,
    pub attributes: Vec<String>,
    pub confidence: ScannerConfidence,
    pub evidence: Vec<TestsEvidence>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MovyHookKind {
    Init,
    SequencePre,
    SequencePost,
    FunctionPre,
    FunctionPost,
    Oracle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FormalSpecKind {
    Module,
    Function,
    SourceFile,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TestsEvidence {
    pub source: EvidenceSource,
    pub confidence: ScannerConfidence,
    pub message: String,
}

impl TestsEvidence {
    fn compiler(confidence: ScannerConfidence, message: impl Into<String>) -> Self {
        Self {
            source: EvidenceSource::Compiler,
            confidence,
            message: message.into(),
        }
    }

    fn source(confidence: ScannerConfidence, message: impl Into<String>) -> Self {
        Self {
            source: EvidenceSource::SourceFallback,
            confidence,
            message: message.into(),
        }
    }

    fn scanner(confidence: ScannerConfidence, message: impl Into<String>) -> Self {
        Self {
            source: EvidenceSource::Scanner,
            confidence,
            message: message.into(),
        }
    }
}

#[derive(Clone)]
struct ActiveMoveFile {
    package_root: PathBuf,
    path: PathBuf,
    relative_path: String,
    source: String,
    stripped_source: String,
}

#[derive(Clone)]
struct CompilerModule {
    module: MoveModule,
    file_source: Option<String>,
}

#[derive(Clone, Copy)]
struct MovyPackageEvidence {
    manifest_declares_movy: bool,
}

pub fn scan_tests(input: &ScanInput<'_>) -> TestsScanReport {
    let mut diagnostics = Vec::new();
    let active_files = active_move_files(input.package_root.as_deref(), &mut diagnostics);
    let movy_package_evidence = MovyPackageEvidence {
        manifest_declares_movy: input
            .package_root
            .as_deref()
            .is_some_and(manifest_declares_movy),
    };
    let compiler_modules = compiler_modules(input, &active_files, &mut diagnostics);

    if compiler_modules.is_empty() {
        diagnostics.push(ScannerDiagnostic::warning(
            TESTS_SCANNER_ID,
            EvidenceSource::Compiler,
            "compiler parser did not provide any modules for the tests scanner",
        ));
    } else {
        diagnostics.push(ScannerDiagnostic::info(
            TESTS_SCANNER_ID,
            EvidenceSource::Compiler,
            format!(
                "loaded {} compiler-parsed module(s) for test discovery",
                compiler_modules.len()
            ),
        ));
    }

    let mut unit_tests = compiler_unit_tests(&compiler_modules);
    let mut movy_invariant_tests =
        compiler_movy_invariant_tests(&compiler_modules, movy_package_evidence);
    let mut formal_prover_specs = compiler_formal_specs(&compiler_modules);

    let fallback_unit_tests = heuristic_unit_tests(&active_files);
    let added_unit_tests = append_missing_unit_tests(&mut unit_tests, fallback_unit_tests);
    if added_unit_tests > 0 {
        diagnostics.push(ScannerDiagnostic::info(
            TESTS_SCANNER_ID,
            EvidenceSource::SourceFallback,
            format!(
                "unit-test detection used source fallback and added {} candidate(s)",
                added_unit_tests
            ),
        ));
    }

    let fallback_movy_invariant_tests =
        heuristic_movy_invariant_tests(&active_files, movy_package_evidence);
    let added_movy_invariant_tests = append_missing_movy_invariant_tests(
        &mut movy_invariant_tests,
        fallback_movy_invariant_tests,
    );
    if added_movy_invariant_tests > 0 {
        diagnostics.push(ScannerDiagnostic::info(
            TESTS_SCANNER_ID,
            EvidenceSource::SourceFallback,
            format!(
                "Movy invariant detection used source fallback and added {} candidate(s)",
                added_movy_invariant_tests
            ),
        ));
    }

    let fallback_formal_prover_specs = heuristic_formal_specs(&active_files);
    let added_formal_prover_specs =
        append_missing_formal_specs(&mut formal_prover_specs, fallback_formal_prover_specs);
    if added_formal_prover_specs > 0 {
        diagnostics.push(ScannerDiagnostic::info(
            TESTS_SCANNER_ID,
            EvidenceSource::SourceFallback,
            format!(
                "formal spec detection used source fallback and added {} candidate(s)",
                added_formal_prover_specs
            ),
        ));
    }

    sort_and_dedup(&mut unit_tests);
    sort_and_dedup(&mut movy_invariant_tests);
    sort_and_dedup(&mut formal_prover_specs);

    TestsScanReport {
        has_unit_tests: !unit_tests.is_empty(),
        has_movy_invariant_tests: !movy_invariant_tests.is_empty(),
        has_formal_prover_specs: !formal_prover_specs.is_empty(),
        unit_test_count: unit_tests.len(),
        movy_invariant_test_count: movy_invariant_tests.len(),
        formal_prover_spec_count: formal_prover_specs.len(),
        unit_tests,
        movy_invariant_tests,
        formal_prover_specs,
        diagnostics,
    }
}

fn compiler_modules(
    input: &ScanInput<'_>,
    active_files: &[ActiveMoveFile],
    diagnostics: &mut Vec<ScannerDiagnostic>,
) -> Vec<CompilerModule> {
    let mut modules = Vec::new();
    let mut seen = BTreeSet::new();

    for module in &input.package_model.modules {
        let key = module_key(module);
        if seen.insert(key) {
            modules.push(CompilerModule {
                module: module.clone(),
                file_source: source_for_module(module, active_files),
            });
        }
    }

    let parse_sources = input.package_model.modules.is_empty();
    for file in active_files.iter().filter(|file| {
        is_tests_path(&file.relative_path)
            || (parse_sources && is_sources_path(&file.relative_path))
    }) {
        let parsed = parse_module_declarations(&file.source, &file.package_root, &file.path);
        if parsed.is_empty() {
            diagnostics.push(ScannerDiagnostic::info(
                TESTS_SCANNER_ID,
                EvidenceSource::Compiler,
                format!(
                    "compiler parser found no module declarations in {}",
                    file.relative_path
                ),
            ));
        }

        for module in parsed {
            let key = module_key(&module);
            if seen.insert(key) {
                modules.push(CompilerModule {
                    module,
                    file_source: Some(file.source.clone()),
                });
            }
        }
    }

    modules
}

fn compiler_unit_tests(modules: &[CompilerModule]) -> Vec<UnitTestFinding> {
    let mut findings = Vec::new();

    for compiler_module in modules {
        let module = &compiler_module.module;
        for function in module
            .functions
            .iter()
            .filter(|function| is_unit_test(function))
        {
            let mut evidence = vec![TestsEvidence::compiler(
                ScannerConfidence::High,
                format!(
                    "{} is annotated with {}",
                    qualified_function_name(module, function),
                    test_attribute_label(function)
                ),
            )];
            if is_tests_path(&module.file_path) {
                evidence.push(TestsEvidence::compiler(
                    ScannerConfidence::High,
                    "test function is in the dedicated tests folder",
                ));
            } else if is_sources_path(&module.file_path) {
                evidence.push(TestsEvidence::compiler(
                    ScannerConfidence::High,
                    "test function is in a package source file",
                ));
            }

            findings.push(UnitTestFinding {
                module_name: module.name.clone(),
                function_name: function.name.clone(),
                qualified_name: qualified_function_name(module, function),
                file_path: module.file_path.clone(),
                source_folder: source_folder(&module.file_path).to_string(),
                is_random_test: is_random_test(function),
                expected_failure: has_attribute(&function.attributes, "expected_failure"),
                confidence: ScannerConfidence::High,
                evidence,
            });
        }
    }

    findings
}

fn compiler_movy_invariant_tests(
    modules: &[CompilerModule],
    package_evidence: MovyPackageEvidence,
) -> Vec<MovyInvariantFinding> {
    let mut findings = Vec::new();

    for compiler_module in modules {
        let module = &compiler_module.module;
        let file_mentions_movy = compiler_module
            .file_source
            .as_deref()
            .is_some_and(source_mentions_movy)
            || module_mentions_movy(module);

        for function in module
            .functions
            .iter()
            .filter(|function| is_unit_test(function))
        {
            let Some((hook_kind, target_function)) = movy_hook_kind(&function.name) else {
                continue;
            };

            if !package_evidence.manifest_declares_movy && !file_mentions_movy {
                continue;
            }

            let mut evidence = vec![
                TestsEvidence::compiler(
                    ScannerConfidence::High,
                    format!(
                        "{} matches Movy invariant hook naming",
                        qualified_function_name(module, function)
                    ),
                ),
                TestsEvidence::compiler(
                    ScannerConfidence::High,
                    format!(
                        "{} is annotated with {}",
                        qualified_function_name(module, function),
                        test_attribute_label(function)
                    ),
                ),
            ];
            add_movy_reference_evidence(&mut evidence, package_evidence, file_mentions_movy);

            findings.push(MovyInvariantFinding {
                module_name: module.name.clone(),
                function_name: function.name.clone(),
                qualified_name: qualified_function_name(module, function),
                file_path: module.file_path.clone(),
                hook_kind,
                target_function,
                confidence: ScannerConfidence::High,
                evidence,
            });
        }
    }

    findings
}

fn compiler_formal_specs(modules: &[CompilerModule]) -> Vec<FormalProverSpecFinding> {
    let mut findings = Vec::new();

    for compiler_module in modules {
        let module = &compiler_module.module;
        let source = compiler_module.file_source.as_deref().unwrap_or_default();
        let source_evidence = formal_source_evidence(source, ScannerConfidence::High);

        if has_formal_attribute(&module.attributes) {
            let mut evidence = vec![TestsEvidence::compiler(
                ScannerConfidence::High,
                format!("module {} has a formal prover attribute", module.name),
            )];
            evidence.extend(source_evidence.clone());
            findings.push(FormalProverSpecFinding {
                spec_kind: FormalSpecKind::Module,
                module_name: module.name.clone(),
                function_name: None,
                qualified_name: module.name.clone(),
                file_path: module.file_path.clone(),
                attributes: formal_attributes(&module.attributes),
                confidence: ScannerConfidence::High,
                evidence,
            });
        }

        for function in module
            .functions
            .iter()
            .filter(|function| has_formal_attribute(&function.attributes))
        {
            let mut evidence = vec![TestsEvidence::compiler(
                ScannerConfidence::High,
                format!(
                    "{} has a formal prover attribute",
                    qualified_function_name(module, function)
                ),
            )];
            evidence.extend(source_evidence.clone());
            findings.push(FormalProverSpecFinding {
                spec_kind: FormalSpecKind::Function,
                module_name: module.name.clone(),
                function_name: Some(function.name.clone()),
                qualified_name: qualified_function_name(module, function),
                file_path: module.file_path.clone(),
                attributes: formal_attributes(&function.attributes),
                confidence: ScannerConfidence::High,
                evidence,
            });
        }
    }

    findings
}

fn heuristic_unit_tests(files: &[ActiveMoveFile]) -> Vec<UnitTestFinding> {
    let mut findings = Vec::new();

    for file in files {
        for candidate in heuristic_test_functions(file) {
            findings.push(UnitTestFinding {
                module_name: candidate.module_name,
                function_name: candidate.function_name.clone(),
                qualified_name: format!(
                    "{}::{}",
                    candidate.qualified_module, candidate.function_name
                ),
                file_path: file.relative_path.clone(),
                source_folder: source_folder(&file.relative_path).to_string(),
                is_random_test: candidate.is_random_test,
                expected_failure: candidate.expected_failure,
                confidence: ScannerConfidence::Medium,
                evidence: vec![TestsEvidence::source(
                    ScannerConfidence::Medium,
                    "comment-stripped source contains a Move test attribute",
                )],
            });
        }
    }

    findings
}

fn heuristic_movy_invariant_tests(
    files: &[ActiveMoveFile],
    package_evidence: MovyPackageEvidence,
) -> Vec<MovyInvariantFinding> {
    let mut findings = Vec::new();

    for file in files {
        let file_mentions_movy = source_mentions_movy(&file.stripped_source);
        if !package_evidence.manifest_declares_movy && !file_mentions_movy {
            continue;
        }

        for candidate in heuristic_test_functions(file) {
            let Some((hook_kind, target_function)) = movy_hook_kind(&candidate.function_name)
            else {
                continue;
            };

            let mut evidence = vec![TestsEvidence::source(
                ScannerConfidence::Medium,
                "comment-stripped source contains a Movy hook test function",
            )];
            add_movy_reference_evidence(&mut evidence, package_evidence, file_mentions_movy);

            findings.push(MovyInvariantFinding {
                module_name: candidate.module_name,
                function_name: candidate.function_name.clone(),
                qualified_name: format!(
                    "{}::{}",
                    candidate.qualified_module, candidate.function_name
                ),
                file_path: file.relative_path.clone(),
                hook_kind,
                target_function,
                confidence: ScannerConfidence::Medium,
                evidence,
            });
        }
    }

    findings
}

fn heuristic_formal_specs(files: &[ActiveMoveFile]) -> Vec<FormalProverSpecFinding> {
    let mut findings = Vec::new();

    for file in files {
        let evidence = formal_source_evidence(&file.stripped_source, ScannerConfidence::Medium);
        if evidence.is_empty() {
            continue;
        }

        let module_name = module_name_from_source(&file.stripped_source).unwrap_or_default();
        let qualified_module = if module_name.is_empty() {
            file.relative_path.clone()
        } else {
            module_name.clone()
        };
        let mut file_findings = Vec::new();

        for group in attribute_groups(&file.stripped_source) {
            if attribute_group_has_name(&group.content, "spec") {
                let function_name = function_name_after(&file.stripped_source, group.end);
                let qualified_name = function_name
                    .as_ref()
                    .map(|name| format!("{qualified_module}::{name}"))
                    .unwrap_or_else(|| format!("{}#spec@{}", file.relative_path, group.end));
                file_findings.push(FormalProverSpecFinding {
                    spec_kind: if function_name.is_some() {
                        FormalSpecKind::Function
                    } else {
                        FormalSpecKind::SourceFile
                    },
                    module_name: module_name.clone(),
                    function_name,
                    qualified_name,
                    file_path: file.relative_path.clone(),
                    attributes: vec!["spec".to_string()],
                    confidence: ScannerConfidence::Medium,
                    evidence: evidence.clone(),
                });
            } else if attribute_group_has_name(&group.content, "spec_only") {
                file_findings.push(FormalProverSpecFinding {
                    spec_kind: FormalSpecKind::SourceFile,
                    module_name: module_name.clone(),
                    function_name: None,
                    qualified_name: format!("{}#spec_only@{}", file.relative_path, group.end),
                    file_path: file.relative_path.clone(),
                    attributes: vec!["spec_only".to_string()],
                    confidence: ScannerConfidence::Medium,
                    evidence: evidence.clone(),
                });
            }
        }

        if file_findings.is_empty() && contains_old_style_spec(&file.stripped_source) {
            file_findings.push(FormalProverSpecFinding {
                spec_kind: FormalSpecKind::SourceFile,
                module_name: module_name.clone(),
                function_name: None,
                qualified_name: qualified_module,
                file_path: file.relative_path.clone(),
                attributes: heuristic_formal_attributes(&file.stripped_source),
                confidence: ScannerConfidence::Medium,
                evidence,
            });
        }

        findings.extend(file_findings);
    }

    findings
}

fn append_missing_unit_tests(
    findings: &mut Vec<UnitTestFinding>,
    fallback: Vec<UnitTestFinding>,
) -> usize {
    let mut seen = findings.iter().map(unit_test_key).collect::<BTreeSet<_>>();
    let before = findings.len();

    for finding in fallback {
        if seen.insert(unit_test_key(&finding)) {
            findings.push(finding);
        }
    }

    findings.len() - before
}

fn unit_test_key(finding: &UnitTestFinding) -> (String, String) {
    (
        scan_file_key(&finding.file_path),
        finding.function_name.clone(),
    )
}

fn append_missing_movy_invariant_tests(
    findings: &mut Vec<MovyInvariantFinding>,
    fallback: Vec<MovyInvariantFinding>,
) -> usize {
    let mut seen = findings
        .iter()
        .map(movy_invariant_key)
        .collect::<BTreeSet<_>>();
    let before = findings.len();

    for finding in fallback {
        if seen.insert(movy_invariant_key(&finding)) {
            findings.push(finding);
        }
    }

    findings.len() - before
}

fn movy_invariant_key(finding: &MovyInvariantFinding) -> (String, String, MovyHookKind) {
    (
        scan_file_key(&finding.file_path),
        finding.function_name.clone(),
        finding.hook_kind,
    )
}

fn append_missing_formal_specs(
    findings: &mut Vec<FormalProverSpecFinding>,
    fallback: Vec<FormalProverSpecFinding>,
) -> usize {
    let mut seen = findings
        .iter()
        .map(formal_spec_key)
        .collect::<BTreeSet<_>>();
    let before = findings.len();

    for finding in fallback {
        if seen.insert(formal_spec_key(&finding)) {
            findings.push(finding);
        }
    }

    findings.len() - before
}

fn formal_spec_key(
    finding: &FormalProverSpecFinding,
) -> (String, FormalSpecKind, Option<String>, String, Vec<String>) {
    (
        scan_file_key(&finding.file_path),
        finding.spec_kind,
        finding.function_name.clone(),
        finding.qualified_name.clone(),
        finding.attributes.clone(),
    )
}

fn scan_file_key(path: &str) -> String {
    ["sources/", "tests/"]
        .iter()
        .find_map(|marker| path.find(marker).map(|index| path[index..].to_string()))
        .unwrap_or_else(|| path.to_string())
}

#[derive(Clone)]
struct HeuristicTestFunction {
    module_name: String,
    qualified_module: String,
    function_name: String,
    is_random_test: bool,
    expected_failure: bool,
}

fn heuristic_test_functions(file: &ActiveMoveFile) -> Vec<HeuristicTestFunction> {
    let module_name = module_name_from_source(&file.stripped_source).unwrap_or_default();
    let qualified_module = if module_name.is_empty() {
        file.relative_path.clone()
    } else {
        module_name.clone()
    };
    let mut results = Vec::new();

    for group in attribute_groups(&file.stripped_source) {
        if !attribute_group_has_unit_test(&group.content) {
            continue;
        }

        let Some(function_name) = function_name_after(&file.stripped_source, group.end) else {
            continue;
        };

        results.push(HeuristicTestFunction {
            module_name: module_name.clone(),
            qualified_module: qualified_module.clone(),
            function_name,
            is_random_test: attribute_group_has_name(&group.content, "random_test")
                || attribute_group_has_name(&group.content, "rand_test"),
            expected_failure: attribute_group_has_name(&group.content, "expected_failure"),
        });
    }

    results
}

#[derive(Clone)]
struct AttributeGroup {
    content: String,
    end: usize,
}

fn attribute_groups(source: &str) -> Vec<AttributeGroup> {
    let bytes = source.as_bytes();
    let mut groups = Vec::new();
    let mut index = 0;

    while index + 1 < bytes.len() {
        if bytes[index] != b'#' || bytes[index + 1] != b'[' {
            index += 1;
            continue;
        }

        let start = index + 2;
        let mut end = start;
        while end < bytes.len() && bytes[end] != b']' {
            end += 1;
        }

        if end < bytes.len() {
            groups.push(AttributeGroup {
                content: source[start..end].to_ascii_lowercase(),
                end: end + 1,
            });
            index = end + 1;
        } else {
            break;
        }
    }

    groups
}

fn attribute_group_has_unit_test(group: &str) -> bool {
    attribute_group_has_name(group, "test")
        || attribute_group_has_name(group, "random_test")
        || attribute_group_has_name(group, "rand_test")
}

fn attribute_group_has_name(group: &str, expected: &str) -> bool {
    group
        .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .any(|token| token == expected)
}

fn function_name_after(source: &str, offset: usize) -> Option<String> {
    let tail = source.get(offset..)?;
    let fun_index = find_keyword(tail, "fun")?;
    let mut chars = tail.get(fun_index + "fun".len()..)?.chars().peekable();

    while chars
        .peek()
        .is_some_and(|character| character.is_whitespace())
    {
        chars.next();
    }

    let mut name = String::new();
    while chars
        .peek()
        .is_some_and(|character| character.is_ascii_alphanumeric() || *character == '_')
    {
        name.push(chars.next().unwrap());
    }

    (!name.is_empty()).then_some(name)
}

fn find_keyword(source: &str, keyword: &str) -> Option<usize> {
    let bytes = source.as_bytes();
    let keyword_bytes = keyword.as_bytes();
    if keyword_bytes.is_empty() || keyword_bytes.len() > bytes.len() {
        return None;
    }

    for index in 0..=(bytes.len() - keyword_bytes.len()) {
        if &bytes[index..index + keyword_bytes.len()] != keyword_bytes {
            continue;
        }
        let before = index
            .checked_sub(1)
            .and_then(|previous| bytes.get(previous))
            .copied();
        let after = bytes.get(index + keyword_bytes.len()).copied();
        if !is_identifier_byte(before) && !is_identifier_byte(after) {
            return Some(index);
        }
    }

    None
}

fn is_identifier_byte(byte: Option<u8>) -> bool {
    byte.is_some_and(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn module_name_from_source(source: &str) -> Option<String> {
    let module_index = find_keyword(source, "module")?;
    let mut chars = source
        .get(module_index + "module".len()..)?
        .chars()
        .peekable();

    while chars
        .peek()
        .is_some_and(|character| character.is_whitespace())
    {
        chars.next();
    }

    let mut name = String::new();
    while chars.peek().is_some_and(|character| {
        character.is_ascii_alphanumeric() || *character == '_' || *character == ':'
    }) {
        name.push(chars.next().unwrap());
    }

    if name.is_empty() {
        return None;
    }

    Some(
        name.split("::")
            .last()
            .filter(|module| !module.is_empty())
            .unwrap_or(name.as_str())
            .to_string(),
    )
}

fn active_move_files(
    package_root: Option<&Path>,
    diagnostics: &mut Vec<ScannerDiagnostic>,
) -> Vec<ActiveMoveFile> {
    let Some(package_root) = package_root else {
        diagnostics.push(ScannerDiagnostic::info(
            TESTS_SCANNER_ID,
            EvidenceSource::SourceFallback,
            "source fallback skipped because package root was not provided",
        ));
        return Vec::new();
    };

    let mut files = Vec::new();
    for directory in [package_root.join("sources"), package_root.join("tests")] {
        if !directory.is_dir() {
            continue;
        }

        let walker = WalkDir::new(&directory)
            .into_iter()
            .filter_entry(|entry| should_descend(package_root, entry));

        for entry in walker
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
        {
            let path = entry.into_path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("move") {
                continue;
            }
            let Ok(source) = fs::read_to_string(&path) else {
                diagnostics.push(ScannerDiagnostic::warning(
                    TESTS_SCANNER_ID,
                    EvidenceSource::SourceFallback,
                    format!("could not read Move source file {}", path.display()),
                ));
                continue;
            };
            let Some(relative_path) = relative_path(package_root, &path) else {
                continue;
            };
            let stripped_source = strip_move_comments(&source);
            files.push(ActiveMoveFile {
                package_root: package_root.to_path_buf(),
                path,
                relative_path,
                source,
                stripped_source,
            });
        }
    }

    files.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    diagnostics.push(ScannerDiagnostic::info(
        TESTS_SCANNER_ID,
        EvidenceSource::SourceFallback,
        format!(
            "loaded {} active Move source/test file(s) for fallback scanning",
            files.len()
        ),
    ));
    files
}

fn should_descend(package_root: &Path, entry: &DirEntry) -> bool {
    if !entry.file_type().is_dir() {
        return true;
    }

    let path = entry.path();
    if path == package_root {
        return true;
    }

    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };

    if name.starts_with('.') {
        return false;
    }

    if matches!(
        name,
        "build"
            | "dependencies"
            | "node_modules"
            | "package_summaries"
            | "target"
            | "coverage"
            | "dist"
    ) {
        return false;
    }

    if path != package_root && path.join("Move.toml").is_file() {
        return false;
    }

    true
}

fn manifest_declares_movy(package_root: &Path) -> bool {
    let Ok(manifest) = fs::read_to_string(package_root.join("Move.toml")) else {
        return false;
    };

    let Ok(value) = manifest.parse::<toml::Value>() else {
        return manifest.to_ascii_lowercase().contains("movy");
    };

    ["dependencies", "dev-dependencies"]
        .iter()
        .any(|section| toml_table_contains_movy(value.get(section)))
}

fn toml_table_contains_movy(value: Option<&toml::Value>) -> bool {
    let Some(toml::Value::Table(table)) = value else {
        return false;
    };

    table.iter().any(|(key, value)| {
        key.eq_ignore_ascii_case("movy") || value.to_string().to_ascii_lowercase().contains("movy")
    })
}

fn source_for_module(module: &MoveModule, active_files: &[ActiveMoveFile]) -> Option<String> {
    active_files
        .iter()
        .find(|file| file.relative_path == module.file_path)
        .or_else(|| {
            active_files.iter().find(|file| {
                module.file_path.ends_with(&file.relative_path)
                    || file.relative_path.ends_with(&module.file_path)
            })
        })
        .map(|file| file.source.clone())
}

fn strip_move_comments(source: &str) -> String {
    let mut output = String::with_capacity(source.len());
    let mut chars = source.chars().peekable();
    let mut block_depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    while let Some(character) = chars.next() {
        if block_depth > 0 {
            if character == '/' && chars.peek() == Some(&'*') {
                output.push(' ');
                output.push(' ');
                chars.next();
                block_depth += 1;
                continue;
            }
            if character == '*' && chars.peek() == Some(&'/') {
                output.push(' ');
                output.push(' ');
                chars.next();
                block_depth = block_depth.saturating_sub(1);
                continue;
            }
            if character == '\n' {
                output.push('\n');
            } else {
                output.push(' ');
            }
            continue;
        }

        if in_string {
            output.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }

        if character == '"' {
            in_string = true;
            output.push(character);
            continue;
        }

        if character == '/' && chars.peek() == Some(&'/') {
            output.push(' ');
            output.push(' ');
            chars.next();
            for comment_character in chars.by_ref() {
                if comment_character == '\n' {
                    output.push('\n');
                    break;
                }
                output.push(' ');
            }
            continue;
        }

        if character == '/' && chars.peek() == Some(&'*') {
            output.push(' ');
            output.push(' ');
            chars.next();
            block_depth = 1;
            continue;
        }

        output.push(character);
    }

    output
}

fn module_key(module: &MoveModule) -> String {
    format!(
        "{}::{}::{}",
        module.file_path,
        module.address.as_deref().unwrap_or_default(),
        module.name
    )
}

fn qualified_function_name(module: &MoveModule, function: &MoveFunctionSignature) -> String {
    format!("{}::{}", module.name, function.name)
}

fn is_unit_test(function: &MoveFunctionSignature) -> bool {
    has_attribute(&function.attributes, "test")
        || has_attribute(&function.attributes, "random_test")
        || has_attribute(&function.attributes, "rand_test")
}

fn is_random_test(function: &MoveFunctionSignature) -> bool {
    has_attribute(&function.attributes, "random_test")
        || has_attribute(&function.attributes, "rand_test")
}

fn test_attribute_label(function: &MoveFunctionSignature) -> &'static str {
    if is_random_test(function) {
        "#[random_test]"
    } else {
        "#[test]"
    }
}

fn has_formal_attribute(attributes: &[String]) -> bool {
    has_attribute(attributes, "spec") || has_attribute(attributes, "spec_only")
}

fn formal_attributes(attributes: &[String]) -> Vec<String> {
    attributes
        .iter()
        .filter(|attribute| attribute.as_str() == "spec" || attribute.as_str() == "spec_only")
        .cloned()
        .collect()
}

fn heuristic_formal_attributes(source: &str) -> Vec<String> {
    let mut attributes = Vec::new();
    if source.contains("#[spec") {
        attributes.push("spec".to_string());
    }
    if source.contains("#[spec_only") {
        attributes.push("spec_only".to_string());
    }
    if contains_old_style_spec(source) {
        attributes.push("legacy_spec_block".to_string());
    }
    attributes.sort();
    attributes.dedup();
    attributes
}

fn has_attribute(attributes: &[String], expected: &str) -> bool {
    attributes.iter().any(|attribute| attribute == expected)
}

fn movy_hook_kind(function_name: &str) -> Option<(MovyHookKind, Option<String>)> {
    if function_name == "movy_init" {
        return Some((MovyHookKind::Init, None));
    }
    if function_name == "movy_pre_ptb" {
        return Some((MovyHookKind::SequencePre, Some("ptb".to_string())));
    }
    if function_name == "movy_post_ptb" {
        return Some((MovyHookKind::SequencePost, Some("ptb".to_string())));
    }
    if let Some(target) = function_name.strip_prefix("movy_pre_") {
        if !target.is_empty() {
            return Some((MovyHookKind::FunctionPre, Some(target.to_string())));
        }
    }
    if let Some(target) = function_name.strip_prefix("movy_post_") {
        if !target.is_empty() {
            return Some((MovyHookKind::FunctionPost, Some(target.to_string())));
        }
    }
    if function_name.starts_with("movy_oracle") {
        return Some((MovyHookKind::Oracle, None));
    }
    None
}

fn add_movy_reference_evidence(
    evidence: &mut Vec<TestsEvidence>,
    package_evidence: MovyPackageEvidence,
    file_mentions_movy: bool,
) {
    if package_evidence.manifest_declares_movy {
        evidence.push(TestsEvidence::scanner(
            ScannerConfidence::High,
            "Move.toml declares a movy dependency",
        ));
    }
    if file_mentions_movy {
        evidence.push(TestsEvidence::source(
            ScannerConfidence::High,
            "source references movy context/oracle APIs",
        ));
    }
}

fn module_mentions_movy(module: &MoveModule) -> bool {
    module.functions.iter().any(|function| {
        function.signature.contains("MovyContext")
            || function.body.as_deref().is_some_and(source_mentions_movy)
    })
}

fn source_mentions_movy(source: &str) -> bool {
    let lower = source.to_ascii_lowercase();
    lower.contains("movy::")
        || lower.contains("movycontext")
        || lower.contains("crash_because")
        || lower.contains("borrow_mut_state")
        || lower.contains("borrow_state")
}

fn formal_source_evidence(source: &str, confidence: ScannerConfidence) -> Vec<TestsEvidence> {
    if source.is_empty() {
        return Vec::new();
    }

    let mut evidence = Vec::new();
    let lower = source.to_ascii_lowercase();
    let markers = [
        ("#[spec", "source contains Sui Prover #[spec] attributes"),
        (
            "#[spec_only",
            "source contains Sui Prover #[spec_only] attributes",
        ),
        ("prover::prover", "source imports prover::prover contracts"),
    ];

    for (marker, message) in markers {
        if lower.contains(marker) {
            evidence.push(TestsEvidence::source(confidence, message));
        }
    }

    let contract_markers = [
        ("requires", "source contains a requires contract"),
        ("ensures", "source contains an ensures contract"),
        ("asserts", "source contains an asserts contract"),
    ];
    for (marker, message) in contract_markers {
        if find_keyword(&lower, marker).is_some() {
            evidence.push(TestsEvidence::source(confidence, message));
        }
    }

    let macro_markers = [
        ("clone!", "source contains clone! spec helper"),
        ("invariant!", "source contains invariant! spec helper"),
    ];
    for (marker, message) in macro_markers {
        if lower.contains(marker) {
            evidence.push(TestsEvidence::source(confidence, message));
        }
    }

    if lower.contains("#[spec") && lower.contains("prove") {
        evidence.push(TestsEvidence::source(
            confidence,
            "source contains a prove spec attribute",
        ));
    }
    if lower.contains("#[spec") && lower.contains("target") {
        evidence.push(TestsEvidence::source(
            confidence,
            "source contains a target spec attribute",
        ));
    }
    if contains_old_style_spec(&lower) {
        evidence.push(TestsEvidence::source(
            confidence,
            "source contains old-style Move spec block syntax",
        ));
    }

    sort_and_dedup(&mut evidence);
    evidence
}

fn contains_old_style_spec(source: &str) -> bool {
    ["spec fun", "spec module", "spec struct"]
        .iter()
        .any(|marker| source.contains(marker))
}

fn is_tests_path(path: &str) -> bool {
    path == "tests"
        || path.starts_with("tests/")
        || path.contains("/tests/")
        || path.ends_with("/tests")
}

fn is_sources_path(path: &str) -> bool {
    path == "sources"
        || path.starts_with("sources/")
        || path.contains("/sources/")
        || path.ends_with("/sources")
}

fn source_folder(path: &str) -> &'static str {
    if is_tests_path(path) {
        "tests"
    } else if is_sources_path(path) {
        "sources"
    } else {
        "unknown"
    }
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    Some(
        path.strip_prefix(root)
            .ok()?
            .components()
            .map(|component| component.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()?
            .join("/"),
    )
}

fn sort_and_dedup<T>(items: &mut Vec<T>)
where
    T: Ord,
{
    items.sort();
    items.dedup();
}

#[cfg(test)]
mod unit_tests {
    use std::{fs, path::Path};

    use peregrine_move_model::{
        parse_module_declarations, MoveFunctionSignature, MoveModule, MovePackageModel,
    };
    use tempfile::tempdir;

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
            has_source_files: true,
            has_source_modules: !modules.is_empty(),
            source_file_count: 1,
            modules,
        }
    }

    fn empty_package() -> MovePackageModel {
        MovePackageModel {
            name: "empty".to_string(),
            path: String::new(),
            manifest_path: "Move.toml".to_string(),
            has_source_files: false,
            has_source_modules: false,
            source_file_count: 0,
            modules: Vec::new(),
        }
    }

    fn package_with_compiler_formal_spec() -> MovePackageModel {
        MovePackageModel {
            name: "demo".to_string(),
            path: String::new(),
            manifest_path: "Move.toml".to_string(),
            has_source_files: true,
            has_source_modules: true,
            source_file_count: 1,
            modules: vec![MoveModule {
                name: "main".to_string(),
                address: Some("demo".to_string()),
                file_path: "sources/main.move".to_string(),
                attributes: Vec::new(),
                structs: Vec::new(),
                functions: vec![MoveFunctionSignature {
                    name: "ok_spec".to_string(),
                    visibility: "private".to_string(),
                    is_entry: false,
                    is_transaction_callable: false,
                    signature: "fun ok_spec(): u64".to_string(),
                    body: None,
                    attributes: vec!["spec".to_string()],
                }],
            }],
        }
    }

    fn scan_model(package: &MovePackageModel, package_root: Option<PathBuf>) -> TestsScanReport {
        let input = ScanInput {
            package_model: package,
            package_root,
            build_root: None,
            source_mode: SourceMode::SourceOnly,
        };
        scan_tests(&input)
    }

    fn write_package_file(root: &Path, relative: &str, contents: &str) {
        let path = root.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        fs::write(path, contents).expect("write source");
    }

    #[test]
    fn detects_unit_tests_in_sources() {
        let report = scan_model(
            &package_from_source(
                r#"
module demo::main;

#[test]
fun test_create() {}
"#,
            ),
            None,
        );

        assert!(report.has_unit_tests);
        assert_eq!(report.unit_test_count, 1);
        assert_eq!(report.unit_tests[0].source_folder, "sources");
        assert_eq!(report.unit_tests[0].confidence, ScannerConfidence::High);
    }

    #[test]
    fn detects_unit_tests_in_dedicated_tests_folder() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"test_package\"\n",
        )
        .expect("manifest");
        write_package_file(
            temp.path(),
            "tests/main_tests.move",
            r#"
#[test_only]
module test_package::main_tests;

#[test]
fun test_flow() {}
"#,
        );
        let package = empty_package();
        let report = scan_model(&package, Some(temp.path().to_path_buf()));

        assert!(report.has_unit_tests);
        assert_eq!(report.unit_test_count, 1);
        assert_eq!(report.unit_tests[0].source_folder, "tests");
        assert_eq!(report.unit_tests[0].function_name, "test_flow");
    }

    #[test]
    fn detects_random_test_and_expected_failure_metadata() {
        let report = scan_model(
            &package_from_source(
                r#"
module demo::main;

#[random_test]
fun fuzz_value(x: u64) {}

#[test, expected_failure]
fun aborts() { abort 0 }

#[expected_failure]
fun metadata_only() { abort 1 }
"#,
            ),
            None,
        );

        assert!(report.has_unit_tests);
        assert_eq!(report.unit_test_count, 2);
        assert!(report
            .unit_tests
            .iter()
            .any(|finding| finding.function_name == "fuzz_value" && finding.is_random_test));
        assert!(report
            .unit_tests
            .iter()
            .any(|finding| finding.function_name == "aborts" && finding.expected_failure));
    }

    #[test]
    fn ignores_commented_out_tests_in_fallback() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"commented\"\n",
        )
        .expect("manifest");
        write_package_file(
            temp.path(),
            "sources/commented.move",
            r#"
/*
module commented::commented;

#[test]
fun test_commented() {}
*/
"#,
        );
        let report = scan_model(&empty_package(), Some(temp.path().to_path_buf()));

        assert!(!report.has_unit_tests);
        assert_eq!(report.unit_test_count, 0);
    }

    #[test]
    fn detects_movy_counter_style_hooks() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("Move.toml"),
            r#"
[package]
name = "counter"

[dev-dependencies]
movy = { git = "https://github.com/BitsLabSec/movy", subdir = "move/movy", rev = "master" }
"#,
        )
        .expect("manifest");
        write_package_file(
            temp.path(),
            "tests/movy.move",
            r#"
#[test_only]
module counter::counter_tests;

use movy::context::Self;
use movy::oracle::crash_because;

#[test]
public fun movy_init(deployer: address, attacker: address) {}

#[test]
public fun movy_pre_ptb(movy: &mut context::MovyContext) {}

#[test]
public fun movy_post_increment(movy: &mut context::MovyContext, n: u64) {
    crash_because(b"bad".to_string());
}
"#,
        );
        let report = scan_model(&empty_package(), Some(temp.path().to_path_buf()));

        assert!(report.has_movy_invariant_tests);
        assert_eq!(report.movy_invariant_test_count, 3);
        assert!(report
            .movy_invariant_tests
            .iter()
            .any(|finding| finding.hook_kind == MovyHookKind::FunctionPost
                && finding.target_function.as_deref() == Some("increment")));
    }

    #[test]
    fn detects_formal_prover_specs() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("manifest");
        write_package_file(
            temp.path(),
            "sources/vault.move",
            r#"
module demo::vault;

#[spec_only]
use prover::prover::{requires, ensures};

public fun withdraw(x: u64): u64 { x }

#[spec(prove)]
fun withdraw_spec(x: u64): u64 {
    requires(x > 0);
    let result = withdraw(x);
    ensures(result == x);
    result
}

#[spec(target = vault::withdraw)]
fun withdraw_summary(x: u64): u64 {
    withdraw(x)
}
"#,
        );
        let package = empty_package();
        let report = scan_model(&package, Some(temp.path().to_path_buf()));

        assert!(report.has_formal_prover_specs);
        assert!(report.formal_prover_spec_count >= 3);
        assert!(report
            .formal_prover_specs
            .iter()
            .any(|finding| finding.function_name.as_deref() == Some("withdraw_spec")));
    }

    #[test]
    fn supplements_compiler_specs_with_malformed_source_fallback() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"demo\"\n",
        )
        .expect("manifest");
        write_package_file(
            temp.path(),
            "sources/broken.move",
            r#"
module demo::broken {
    #[spec(target = broken::withdraw)]
    fun broken_spec( {
        ensures (true);
    }
}
"#,
        );
        let package = package_with_compiler_formal_spec();
        let report = scan_model(&package, Some(temp.path().to_path_buf()));

        assert!(report.has_formal_prover_specs);
        assert!(report.formal_prover_spec_count >= 2);
        assert!(report
            .formal_prover_specs
            .iter()
            .any(|finding| finding.confidence == ScannerConfidence::Medium));
    }

    #[test]
    fn falls_back_for_malformed_formal_specs() {
        let temp = tempdir().expect("tempdir");
        fs::write(
            temp.path().join("Move.toml"),
            "[package]\nname = \"broken\"\n",
        )
        .expect("manifest");
        write_package_file(
            temp.path(),
            "sources/specs.move",
            r#"
module broken::specs {
    #[spec(prove)]
    fun broken_spec( {
        ensures(true);
    }
}
"#,
        );
        let report = scan_model(&empty_package(), Some(temp.path().to_path_buf()));

        assert!(report.has_formal_prover_specs);
        assert_eq!(report.formal_prover_spec_count, 1);
        assert_eq!(
            report.formal_prover_specs[0].confidence,
            ScannerConfidence::Medium
        );
    }

    #[test]
    fn empty_package_reports_no_tests_or_specs() {
        let report = scan_model(&empty_package(), None);

        assert!(!report.has_unit_tests);
        assert!(!report.has_movy_invariant_tests);
        assert!(!report.has_formal_prover_specs);
        assert_eq!(report.unit_test_count, 0);
        assert_eq!(report.movy_invariant_test_count, 0);
        assert_eq!(report.formal_prover_spec_count, 0);
    }
}
