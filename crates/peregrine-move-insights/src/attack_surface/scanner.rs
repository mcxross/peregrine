fn scanner_object_report(
    model: &MovePackageModel,
    package_root: Option<PathBuf>,
    build_root: Option<PathBuf>,
) -> ObjectScanReport {
    let report = scan_package(ScanInput {
        package_model: model,
        package_root,
        build_root,
        source_mode: SourceMode::BestAvailable,
    });

    report
        .scanners
        .into_iter()
        .find_map(|output| match output {
            ScannerOutput::Objects(objects) => Some(objects),
        })
        .unwrap_or_else(|| ObjectScanReport {
            capability_findings: Vec::new(),
            ownership_findings: Vec::new(),
            lifecycle_maps: Vec::new(),
            shared_object_structs: Vec::new(),
            diagnostics: report.diagnostics,
        })
}

fn scanner_capability_findings(report: &ObjectScanReport) -> Vec<CapabilityFinding> {
    report
        .capability_findings
        .iter()
        .map(|finding| CapabilityFinding {
            type_name: finding.type_name.clone(),
            module_name: finding.module_name.clone(),
            qualified_name: finding.qualified_name.clone(),
            confidence: finding.confidence.as_str().to_string(),
            evidence: finding
                .evidence
                .iter()
                .map(|evidence| evidence.message.clone())
                .collect(),
            protected_functions: finding.protected_functions.clone(),
        })
        .collect()
}

fn scanner_object_ownership_findings(report: &ObjectScanReport) -> Vec<ObjectOwnershipFinding> {
    report
        .ownership_findings
        .iter()
        .map(|finding| ObjectOwnershipFinding {
            type_name: finding.type_name.clone(),
            module_name: finding.module_name.clone(),
            qualified_name: finding.qualified_name.clone(),
            ownership_kind: finding.ownership_kind.as_str().to_string(),
            confidence: finding.confidence.as_str().to_string(),
            evidence: finding
                .evidence
                .iter()
                .map(|evidence| evidence.message.clone())
                .collect(),
            related_functions: finding.related_functions.clone(),
            wrapped_types: finding.wrapped_types.clone(),
        })
        .collect()
}

fn scanner_object_lifecycle_maps(
    report: &ObjectScanReport,
    modules: &[MoveModule],
) -> Vec<ObjectLifecycleMap> {
    report
        .lifecycle_maps
        .iter()
        .map(|lifecycle| {
            let mut map = scanner_lifecycle_map(lifecycle);
            map.risks = object_lifecycle_risks(modules, &map);
            map
        })
        .collect()
}

fn scanner_lifecycle_map(lifecycle: &ScannerLifecycleModel) -> ObjectLifecycleMap {
    ObjectLifecycleMap {
        type_name: lifecycle.type_name.clone(),
        module_name: lifecycle.module_name.clone(),
        qualified_name: lifecycle.qualified_name.clone(),
        file_path: lifecycle.file_path.clone(),
        abilities: lifecycle.abilities.clone(),
        is_capability_like: lifecycle.is_capability_like,
        stages: lifecycle
            .stages
            .iter()
            .map(scanner_lifecycle_stage)
            .collect(),
        touched_by: lifecycle
            .touched_by
            .iter()
            .map(scanner_lifecycle_function_ref)
            .collect(),
        risks: Vec::new(),
    }
}

fn scanner_lifecycle_stage(stage: &ScannerLifecycleStageModel) -> ObjectLifecycleStage {
    ObjectLifecycleStage {
        kind: stage.kind.as_str().to_string(),
        functions: stage
            .functions
            .iter()
            .map(scanner_lifecycle_function_ref)
            .collect(),
        evidence: stage
            .evidence
            .iter()
            .map(|evidence| evidence.message.clone())
            .collect(),
    }
}

fn scanner_lifecycle_function_ref(
    function: &ScannerLifecycleFunctionRef,
) -> ObjectLifecycleFunctionRef {
    ObjectLifecycleFunctionRef {
        module_name: function.module_name.clone(),
        function_name: function.function_name.clone(),
        qualified_name: function.qualified_name.clone(),
        file_path: function.file_path.clone(),
        visibility: function.visibility.clone(),
        is_entry: function.is_entry,
        is_transaction_callable: function.is_transaction_callable,
        direct: function.direct,
        call_path: function.call_path.clone(),
        evidence: function
            .evidence
            .iter()
            .map(|evidence| evidence.message.clone())
            .collect(),
    }
}

fn ownership_count(findings: &[ObjectOwnershipFinding], kind: &str) -> usize {
    findings
        .iter()
        .filter(|finding| finding.ownership_kind == kind && finding.confidence != "low")
        .map(|finding| finding.qualified_name.as_str())
        .collect::<HashSet<_>>()
        .len()
}
