use super::{AuditStore, AuditStoreError, AuditStoreEvent};
use peregrine_types::{
    AuditAgentConclusion, AuditAgentConclusionStatus, AuditAgentRole, AuditEvidence,
    AuditEvidenceAttestation, AuditReport, AuditRun, AuditRunStatus, AuditStageId,
    AuditStageStatus, AuditWorkItemStatus, EvidenceConfidence, FindingCandidate,
    FindingCandidateStatus, Metadata, VerificationMethod,
};
use serde_json::json;
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub struct FinalizedAuditReport {
    pub run: AuditRun,
    pub report_ref: String,
    pub markdown_ref: String,
}

impl AuditStore {
    pub fn finalize_report(
        &self,
        run_id: &str,
        findings: Vec<FindingCandidate>,
        metadata: Metadata,
        now: i64,
    ) -> Result<FinalizedAuditReport, AuditStoreError> {
        let report_ref = "reports/report.json".to_string();
        let markdown_ref = "reports/report.md".to_string();
        let finding_events = findings.clone();
        let (run, _) = self.mutate_run(run_id, |run| {
            ensure_report_ready(run)?;
            let evidence = self.read_evidence_index(run_id, run)?;
            let judge_conclusions = self.read_judge_conclusions(run_id, run)?;
            validate_findings(&findings, &evidence, &judge_conclusions)?;

            let status = terminal_status(run);
            let report = AuditReport {
                schema_version: 1,
                audit_run_id: run.id.clone(),
                status: status.clone(),
                findings,
                coverage_gaps: run.coverage_gaps.clone(),
                evidence_refs: run.evidence_refs.clone(),
                generated_at: now,
                metadata,
            };
            self.write_json(&self.audits_root.join(run_id).join(&report_ref), &report)?;
            self.write_text(
                &self.audits_root.join(run_id).join(&markdown_ref),
                &render_report_markdown(&report),
            )?;

            complete_terminal_work(run, now);
            run.status = status;
            run.updated_at = now;
            push_ref_once(&mut run.artifact_refs, &report_ref);
            push_ref_once(&mut run.artifact_refs, &markdown_ref);
            run.metadata
                .insert("terminalReportRef".to_string(), json!(report_ref.clone()));
            run.metadata.insert(
                "terminalReportMarkdownRef".to_string(),
                json!(markdown_ref.clone()),
            );
            run.metadata
                .insert("terminalReportGeneratedAt".to_string(), json!(now));
            Ok(())
        })?;
        self.publish_event(AuditStoreEvent::StageUpdated {
            audit_id: run.id.clone(),
            stage: run.current_stage.clone(),
            status: terminal_stage_status(&run.status),
            run: run.clone(),
        });
        self.publish_event(AuditStoreEvent::Activity {
            audit_id: run.id.clone(),
            category: "report".to_string(),
            message: format!(
                "terminal report written with status {:?}: {} findings, {} coverage gaps",
                run.status,
                finding_events.len(),
                run.coverage_gaps.len()
            ),
            stage: Some(format!("{:?}", run.current_stage)),
            work_item_id: None,
            artifact_ref: Some(report_ref.clone()),
            agent_role: Some("Judge".to_string()),
            tool_name: Some("audit_finalize_report".to_string()),
        });
        for finding in finding_events {
            self.publish_event(AuditStoreEvent::FindingUpdated {
                audit_id: run.id.clone(),
                finding,
                report_ref: report_ref.clone(),
            });
        }
        Ok(FinalizedAuditReport {
            run,
            report_ref,
            markdown_ref,
        })
    }

    fn read_evidence_index(
        &self,
        run_id: &str,
        run: &AuditRun,
    ) -> Result<BTreeMap<String, AuditEvidence>, AuditStoreError> {
        run.evidence_refs
            .iter()
            .map(|evidence_ref| {
                let path = self.audits_root.join(run_id).join(evidence_ref);
                let body =
                    std::fs::read_to_string(&path).map_err(|source| AuditStoreError::Io {
                        action: "read audit evidence",
                        source,
                    })?;
                let evidence = serde_json::from_str(&body)?;
                Ok((evidence_ref.clone(), evidence))
            })
            .collect()
    }

    fn read_judge_conclusions(
        &self,
        run_id: &str,
        run: &AuditRun,
    ) -> Result<Vec<AuditAgentConclusion>, AuditStoreError> {
        run.artifact_refs
            .iter()
            .filter(|artifact_ref| artifact_ref.starts_with("artifacts/agent-conclusions/"))
            .map(|artifact_ref| {
                let path = self.audits_root.join(run_id).join(artifact_ref);
                let body =
                    std::fs::read_to_string(&path).map_err(|source| AuditStoreError::Io {
                        action: "read audit agent conclusion",
                        source,
                    })?;
                let conclusion: AuditAgentConclusion = serde_json::from_str(&body)?;
                Ok(conclusion)
            })
            .filter(|result| {
                result
                    .as_ref()
                    .map(|conclusion| conclusion.role == AuditAgentRole::Judge)
                    .unwrap_or(true)
            })
            .collect()
    }
}

fn ensure_report_ready(run: &AuditRun) -> Result<(), AuditStoreError> {
    if run.status != AuditRunStatus::Running {
        return Err(AuditStoreError::InvalidRunState {
            run_id: run.id.clone(),
            status: format!("{:?}", run.status),
        });
    }
    if let Some(work_item) = run
        .work_items
        .iter()
        .find(|item| item.status == AuditWorkItemStatus::Pending)
    {
        return Err(AuditStoreError::InvalidReport(format!(
            "work item `{}` is still pending",
            work_item.id
        )));
    }
    if let Some(work_item) = run.work_items.iter().find(|item| {
        item.status == AuditWorkItemStatus::Claimed
            && !matches!(
                item.stage,
                AuditStageId::AuditReport | AuditStageId::AuditTrace
            )
    }) {
        return Err(AuditStoreError::InvalidReport(format!(
            "non-terminal work item `{}` is still claimed",
            work_item.id
        )));
    }
    Ok(())
}

fn validate_findings(
    findings: &[FindingCandidate],
    evidence: &BTreeMap<String, AuditEvidence>,
    judge_conclusions: &[AuditAgentConclusion],
) -> Result<(), AuditStoreError> {
    for finding in findings {
        if finding.evidence_refs.is_empty() {
            return Err(AuditStoreError::InvalidReport(format!(
                "finding `{}` has no evidence references",
                finding.id
            )));
        }
        for evidence_ref in &finding.evidence_refs {
            if !evidence.contains_key(evidence_ref) {
                return Err(AuditStoreError::EvidenceNotFound(evidence_ref.clone()));
            }
        }
        if requires_confirmation(finding) {
            validate_confirmed_finding(finding, evidence, judge_conclusions)?;
        }
    }
    Ok(())
}

fn requires_confirmation(finding: &FindingCandidate) -> bool {
    finding.status == FindingCandidateStatus::Confirmed
        || finding.confidence == EvidenceConfidence::Confirmed
}

fn validate_confirmed_finding(
    finding: &FindingCandidate,
    evidence: &BTreeMap<String, AuditEvidence>,
    judge_conclusions: &[AuditAgentConclusion],
) -> Result<(), AuditStoreError> {
    let mut verification_methods = Vec::new();
    let mut has_adapter_replay = false;

    for evidence_ref in &finding.evidence_refs {
        let item = evidence
            .get(evidence_ref)
            .ok_or_else(|| AuditStoreError::EvidenceNotFound(evidence_ref.clone()))?;
        if counts_as_independent_confirmation(item) {
            push_verification_method_once(&mut verification_methods, &item.verification_method);
        }
        if item.verification_method == VerificationMethod::ExploitReplay
            && item.attestation == AuditEvidenceAttestation::AdapterReplay
            && item.execution_trace_ref.is_some()
        {
            has_adapter_replay = true;
        }
    }

    if verification_methods.len() < 2 {
        return Err(AuditStoreError::InvalidReport(format!(
            "confirmed finding `{}` needs at least two independent verification classes",
            finding.id
        )));
    }
    if !has_adapter_replay {
        return Err(AuditStoreError::InvalidReport(format!(
            "confirmed finding `{}` needs an adapter replay evidence item with an execution trace",
            finding.id
        )));
    }
    if !has_positive_judge_conclusion(finding, judge_conclusions) {
        return Err(AuditStoreError::InvalidReport(format!(
            "confirmed finding `{}` needs a public positive Judge conclusion",
            finding.id
        )));
    }
    Ok(())
}

fn has_positive_judge_conclusion(
    finding: &FindingCandidate,
    judge_conclusions: &[AuditAgentConclusion],
) -> bool {
    judge_conclusions.iter().any(|conclusion| {
        matches!(
            conclusion.status,
            AuditAgentConclusionStatus::Accepted | AuditAgentConclusionStatus::Supported
        ) && (conclusion
            .candidate_ids
            .iter()
            .any(|candidate_id| candidate_id == &finding.id)
            || finding
                .evidence_refs
                .iter()
                .all(|evidence_ref| conclusion.evidence_refs.contains(evidence_ref)))
    })
}

fn push_verification_method_once(
    verification_methods: &mut Vec<VerificationMethod>,
    method: &VerificationMethod,
) {
    if !verification_methods.contains(method) {
        verification_methods.push(method.clone());
    }
}

fn counts_as_independent_confirmation(evidence: &AuditEvidence) -> bool {
    if evidence.attestation == AuditEvidenceAttestation::ModelSubmitted {
        return false;
    }
    if evidence.verification_method == VerificationMethod::HumanReview {
        return false;
    }
    evidence.verification_method != VerificationMethod::GeneratedTest
        || evidence.execution_trace_ref.is_some()
}

fn terminal_status(run: &AuditRun) -> AuditRunStatus {
    if run
        .coverage_gaps
        .iter()
        .any(|gap| gap.affects_terminal_status)
        || run.work_items.iter().any(|item| {
            matches!(
                item.status,
                AuditWorkItemStatus::Failed
                    | AuditWorkItemStatus::Blocked
                    | AuditWorkItemStatus::Cancelled
            )
        })
    {
        AuditRunStatus::CompletedWithGaps
    } else {
        AuditRunStatus::Completed
    }
}

fn complete_terminal_work(run: &mut AuditRun, now: i64) {
    for work_item in &mut run.work_items {
        if work_item.status == AuditWorkItemStatus::Claimed
            && matches!(
                work_item.stage,
                AuditStageId::AuditReport | AuditStageId::AuditTrace
            )
        {
            work_item.status = AuditWorkItemStatus::Completed;
            work_item.updated_at = now;
        }
    }
}

fn terminal_stage_status(status: &AuditRunStatus) -> AuditStageStatus {
    match status {
        AuditRunStatus::Completed | AuditRunStatus::CompletedWithGaps => {
            AuditStageStatus::Succeeded
        }
        AuditRunStatus::Failed => AuditStageStatus::Failed,
        AuditRunStatus::Cancelled => AuditStageStatus::Cancelled,
        AuditRunStatus::Pending => AuditStageStatus::Pending,
        AuditRunStatus::Running => AuditStageStatus::Running,
        AuditRunStatus::Paused => AuditStageStatus::Blocked,
    }
}

fn push_ref_once(refs: &mut Vec<String>, value: &str) {
    if !refs.iter().any(|existing| existing == value) {
        refs.push(value.to_string());
    }
}

fn render_report_markdown(report: &AuditReport) -> String {
    let mut output = String::new();
    output.push_str("# Audit Report\n\n");
    output.push_str(&format!("Audit run: `{}`\n\n", report.audit_run_id));
    output.push_str(&format!("Status: `{:?}`\n\n", report.status));
    output.push_str(&format!("Generated at: `{}`\n\n", report.generated_at));
    output.push_str(&format!("Findings: `{}`\n\n", report.findings.len()));

    for finding in &report.findings {
        output.push_str(&format!("## {}\n\n", finding.title));
        output.push_str(&format!("- ID: `{}`\n", finding.id));
        output.push_str(&format!("- Category: `{}`\n", finding.category));
        output.push_str(&format!("- Severity: `{:?}`\n", finding.severity));
        output.push_str(&format!("- Status: `{:?}`\n", finding.status));
        output.push_str(&format!("- Confidence: `{:?}`\n", finding.confidence));
        output.push_str("- Evidence:\n");
        for evidence_ref in &finding.evidence_refs {
            output.push_str(&format!("  - `{evidence_ref}`\n"));
        }
        output.push('\n');
    }

    if !report.coverage_gaps.is_empty() {
        output.push_str("## Coverage Gaps\n\n");
        for gap in &report.coverage_gaps {
            output.push_str(&format!(
                "- `{:?}` `{}`: {}{}\n",
                gap.stage,
                gap.capability,
                gap.reason,
                if gap.affects_terminal_status {
                    " (affects terminal status)"
                } else {
                    ""
                }
            ));
        }
        output.push('\n');
    }

    output
}
