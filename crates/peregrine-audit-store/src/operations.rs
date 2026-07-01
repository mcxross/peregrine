use super::{AuditStore, AuditStoreError, AuditStoreEvent, status_name};
use peregrine_security_tools::{AuditStageScheduleAction, schedule_metadata};
use peregrine_types::{
    AuditAgentAssignment, AuditAgentAssignmentStatus, AuditAgentConclusion, AuditCoverageGap,
    AuditEvidence, AuditEvidenceAttestation, AuditRun, AuditRunStatus, AuditStageId,
    AuditStageStatus, AuditWorkItem, AuditWorkItemStatus, Metadata, VerificationMethod,
};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path};
use uuid::Uuid;

const MAX_ARTIFACT_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct WorkUpdate {
    pub run: AuditRun,
    pub work_item: AuditWorkItem,
    pub stage_changed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentAssignmentUpdate {
    pub run: AuditRun,
    pub assignment: AuditAgentAssignment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScheduledWorkBlock {
    pub run: AuditRun,
    pub work_item: AuditWorkItem,
    pub artifact_ref: String,
    pub diagnostics: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CapabilityGapUpdate {
    pub run: AuditRun,
    pub work_item: AuditWorkItem,
    pub artifact_ref: String,
}

impl AuditStore {
    pub fn claim_work(
        &self,
        run_id: &str,
        worker_id: &str,
        stage: Option<&AuditStageId>,
        now: i64,
    ) -> Result<Option<(AuditRun, AuditWorkItem)>, AuditStoreError> {
        let claimed = self.mutate_run(run_id, |run| {
            ensure_running(run)?;
            let Some(work_item) = run.work_items.iter_mut().find(|item| {
                item.status == AuditWorkItemStatus::Pending
                    && stage.is_none_or(|stage| &item.stage == stage)
            }) else {
                return Ok(None);
            };
            work_item.status = AuditWorkItemStatus::Claimed;
            work_item.claimed_by = Some(worker_id.to_string());
            work_item.attempts += 1;
            work_item.updated_at = now;
            run.current_stage = work_item.stage.clone();
            run.updated_at = now;
            Ok(Some(work_item.clone()))
        })?;
        if let (run, Some(work_item)) = &claimed {
            self.publish_event(AuditStoreEvent::StageUpdated {
                audit_id: run.id.clone(),
                stage: work_item.stage.clone(),
                status: AuditStageStatus::Running,
                run: run.clone(),
            });
            self.publish_event(AuditStoreEvent::Activity {
                audit_id: run.id.clone(),
                category: "orchestrator".to_string(),
                message: format!("claimed stage {:?}: {}", work_item.stage, work_item.title),
                stage: Some(format!("{:?}", work_item.stage)),
                work_item_id: Some(work_item.id.clone()),
                artifact_ref: None,
                agent_role: None,
                tool_name: None,
            });
        }
        Ok(claimed.1.map(|work_item| (claimed.0, work_item)))
    }

    pub fn record_packet(
        &self,
        run_id: &str,
        work_item_id: &str,
        packet_kind: &str,
        summary: &str,
        packet: Value,
        now: i64,
    ) -> Result<(AuditRun, String), AuditStoreError> {
        let artifact_id = Uuid::new_v4().to_string();
        let artifact_ref = format!("artifacts/{artifact_id}.json");
        let packet_for_metadata = packet.clone();
        let activity_category = packet_activity_category(packet_kind).to_string();
        let activity_message = packet_activity_message(packet_kind, summary, &packet_for_metadata);
        let envelope = json!({
            "schemaVersion": 1,
            "id": artifact_id,
            "auditRunId": run_id,
            "workItemId": work_item_id,
            "kind": packet_kind,
            "summary": summary,
            "createdAt": now,
            "packet": packet,
        });
        let result = self.mutate_run(run_id, |run| {
            let work_item = ensure_claimed_work_item(run, work_item_id)?;
            if packet_kind == "capabilityDispatch" {
                work_item.metadata.insert(
                    "activeCapabilityDispatch".to_string(),
                    packet_for_metadata.clone(),
                );
            }
            self.write_json(
                &self.audits_root.join(run_id).join(&artifact_ref),
                &envelope,
            )?;
            run.artifact_refs.push(artifact_ref.clone());
            run.updated_at = now;
            Ok(artifact_ref.clone())
        });
        if let Ok((run, artifact_ref)) = &result {
            let stage = run
                .work_items
                .iter()
                .find(|item| item.id == work_item_id)
                .map(|item| format!("{:?}", item.stage));
            self.publish_event(AuditStoreEvent::Activity {
                audit_id: run.id.clone(),
                category: activity_category,
                message: activity_message,
                stage,
                work_item_id: Some(work_item_id.to_string()),
                artifact_ref: Some(artifact_ref.clone()),
                agent_role: None,
                tool_name: packet_for_metadata
                    .get("toolName")
                    .and_then(Value::as_str)
                    .map(str::to_string),
            });
        }
        result
    }

    pub fn record_capability_gap(
        &self,
        run_id: &str,
        work_item_id: &str,
        capability: &str,
        reason: &str,
        provider_id: Option<&str>,
        tool_name: Option<&str>,
        now: i64,
    ) -> Result<CapabilityGapUpdate, AuditStoreError> {
        let artifact_id = Uuid::new_v4().to_string();
        let artifact_ref = format!("artifacts/{artifact_id}.json");
        let (run, work_item) = self.mutate_run(run_id, |run| {
            ensure_running(run)?;
            let work_item_index = run
                .work_items
                .iter()
                .position(|item| item.id == work_item_id)
                .ok_or_else(|| AuditStoreError::WorkItemNotFound(work_item_id.to_string()))?;
            if run.work_items[work_item_index].status != AuditWorkItemStatus::Claimed {
                return Err(AuditStoreError::WorkItemNotClaimed(work_item_id.to_string()));
            }
            let stage = run.work_items[work_item_index].stage.clone();
            let gap = AuditCoverageGap {
                capability: capability.to_string(),
                stage: stage.clone(),
                reason: reason.to_string(),
                affects_terminal_status: true,
            };
            upsert_coverage_gap(&mut run.coverage_gaps, gap);
            let packet = json!({
                "schemaVersion": 1,
                "stage": stage,
                "workItemId": work_item_id,
                "action": "recordCapabilityGap",
                "capability": capability,
                "reason": reason,
                "providerId": provider_id,
                "toolName": tool_name,
                "affectsTerminalStatus": true,
            });
            let envelope = json!({
                "schemaVersion": 1,
                "id": artifact_id,
                "auditRunId": run_id,
                "workItemId": work_item_id,
                "kind": "capabilityUnavailable",
                "summary": "Desired audit capability is unavailable; recorded a visible coverage gap.",
                "createdAt": now,
                "packet": packet,
            });
            self.write_json(
                &self.audits_root.join(run_id).join(&artifact_ref),
                &envelope,
            )?;
            let work_item = &mut run.work_items[work_item_index];
            append_runtime_capability_gap(
                &mut work_item.metadata,
                capability,
                reason,
                provider_id,
                tool_name,
                now,
            );
            work_item.updated_at = now;
            let work_item = work_item.clone();
            run.artifact_refs.push(artifact_ref.clone());
            run.updated_at = now;
            Ok(work_item)
        })?;
        self.publish_event(AuditStoreEvent::Activity {
            audit_id: run.id.clone(),
            category: "coverage".to_string(),
            message: format!("capability gap recorded for {capability}: {reason}"),
            stage: Some(format!("{:?}", work_item.stage)),
            work_item_id: Some(work_item.id.clone()),
            artifact_ref: Some(artifact_ref.clone()),
            agent_role: None,
            tool_name: tool_name.map(str::to_string),
        });
        Ok(CapabilityGapUpdate {
            run,
            work_item,
            artifact_ref,
        })
    }

    pub fn record_evidence(
        &self,
        run_id: &str,
        mut evidence: AuditEvidence,
    ) -> Result<(AuditRun, String), AuditStoreError> {
        evidence.id = Uuid::new_v4().to_string();
        evidence.audit_run_id = run_id.to_string();
        let evidence_ref = format!("evidence/{}.json", evidence.id);
        let activity = evidence_activity(&evidence);
        let work_item_id = evidence.work_item_id.clone();
        let result = self.mutate_run(run_id, |run| {
            if let Some(work_item_id) = evidence.work_item_id.as_deref() {
                ensure_claimed_work_item(run, work_item_id)?
                    .evidence_refs
                    .push(evidence_ref.clone());
            }
            self.write_json(
                &self.audits_root.join(run_id).join(&evidence_ref),
                &evidence,
            )?;
            run.evidence_refs.push(evidence_ref.clone());
            run.updated_at = evidence.created_at;
            Ok(evidence_ref.clone())
        });
        if let Ok((run, evidence_ref)) = &result {
            self.publish_event(AuditStoreEvent::Activity {
                audit_id: run.id.clone(),
                category: activity.category,
                message: activity.message,
                stage: stage_for_work_item(run, work_item_id.as_deref()),
                work_item_id: work_item_id,
                artifact_ref: Some(evidence_ref.clone()),
                agent_role: None,
                tool_name: activity.tool_name,
            });
        }
        result
    }

    pub fn record_agent_conclusion(
        &self,
        run_id: &str,
        work_item_id: &str,
        mut conclusion: AuditAgentConclusion,
    ) -> Result<(AuditRun, String), AuditStoreError> {
        conclusion.id = Uuid::new_v4().to_string();
        conclusion.audit_run_id = run_id.to_string();
        conclusion.work_item_id = work_item_id.to_string();
        let artifact_ref = format!("artifacts/agent-conclusions/{}.json", conclusion.id);
        let artifact_dir = self
            .audits_root
            .join(run_id)
            .join("artifacts/agent-conclusions");
        fs::create_dir_all(&artifact_dir).map_err(|source| AuditStoreError::Io {
            action: "create agent conclusion artifact directory",
            source,
        })?;
        let result = self.mutate_run(run_id, |run| {
            ensure_claimed_work_item(run, work_item_id)?;
            for evidence_ref in &conclusion.evidence_refs {
                if !run.evidence_refs.contains(evidence_ref) {
                    return Err(AuditStoreError::EvidenceNotFound(evidence_ref.clone()));
                }
            }
            for artifact_ref in &conclusion.artifact_refs {
                if !run.artifact_refs.contains(artifact_ref) {
                    return Err(AuditStoreError::InvalidArtifactPath);
                }
            }
            self.write_json(
                &self.audits_root.join(run_id).join(&artifact_ref),
                &conclusion,
            )?;
            run.artifact_refs.push(artifact_ref.clone());
            if let Some(assignment) = run.agent_assignments.iter_mut().find(|assignment| {
                assignment.work_item_id == work_item_id
                    && assignment.role == conclusion.role
                    && (assignment.agent_thread_id.is_none()
                        || conclusion.agent_thread_id.is_none()
                        || assignment.agent_thread_id == conclusion.agent_thread_id)
            }) {
                if conclusion.agent_thread_id.is_some() {
                    assignment.agent_thread_id = conclusion.agent_thread_id.clone();
                }
                assignment.status = AuditAgentAssignmentStatus::Completed;
                assignment.conclusion_refs.push(artifact_ref.clone());
                assignment.updated_at = conclusion.created_at;
            }
            run.updated_at = conclusion.created_at;
            Ok(artifact_ref.clone())
        });
        if let Ok((run, artifact_ref)) = &result {
            self.publish_event(AuditStoreEvent::Activity {
                audit_id: run.id.clone(),
                category: "agent".to_string(),
                message: format!(
                    "{:?} recorded {:?} conclusion: {}",
                    conclusion.role, conclusion.status, conclusion.summary
                ),
                stage: stage_for_work_item(run, Some(work_item_id)),
                work_item_id: Some(work_item_id.to_string()),
                artifact_ref: Some(artifact_ref.clone()),
                agent_role: Some(format!("{:?}", conclusion.role)),
                tool_name: None,
            });
        }
        result
    }

    pub fn claim_agent_assignment(
        &self,
        run_id: &str,
        work_item_id: &str,
        assignment_id: &str,
        worker_id: &str,
        now: i64,
    ) -> Result<AgentAssignmentUpdate, AuditStoreError> {
        let (run, assignment) = self.mutate_run(run_id, |run| {
            ensure_running(run)?;
            let work_item = ensure_claimed_work_item(run, work_item_id)?;
            if work_item.claimed_by.as_deref() != Some(worker_id) {
                return Err(AuditStoreError::WorkItemClaimedByOther {
                    work_item_id: work_item_id.to_string(),
                    claimed_by: work_item
                        .claimed_by
                        .clone()
                        .unwrap_or_else(|| "unclaimed".to_string()),
                });
            }
            ensure_agent_assignment_claim_order(run, work_item_id, assignment_id)?;
            let assignment = ensure_agent_assignment(run, work_item_id, assignment_id)?;
            if assignment.status != AuditAgentAssignmentStatus::Pending {
                return Err(AuditStoreError::InvalidAgentAssignmentStatus {
                    assignment_id: assignment_id.to_string(),
                    status: format!("{:?}", assignment.status),
                });
            }
            assignment.status = AuditAgentAssignmentStatus::Spawned;
            assignment.updated_at = now;
            let assignment = assignment.clone();
            run.updated_at = now;
            Ok(assignment)
        })?;
        self.publish_event(AuditStoreEvent::Activity {
            audit_id: run.id.clone(),
            category: "agent".to_string(),
            message: format!(
                "claimed {:?} agent assignment `{}`",
                assignment.role, assignment.role_name
            ),
            stage: stage_for_work_item(&run, Some(work_item_id)),
            work_item_id: Some(work_item_id.to_string()),
            artifact_ref: None,
            agent_role: Some(format!("{:?}", assignment.role)),
            tool_name: Some("spawn_agent".to_string()),
        });
        Ok(AgentAssignmentUpdate { run, assignment })
    }

    pub fn update_agent_assignment_thread(
        &self,
        run_id: &str,
        work_item_id: &str,
        assignment_id: &str,
        agent_thread_id: &str,
        now: i64,
    ) -> Result<AgentAssignmentUpdate, AuditStoreError> {
        let (run, assignment) = self.mutate_run(run_id, |run| {
            ensure_running(run)?;
            ensure_claimed_work_item(run, work_item_id)?;
            let assignment = ensure_agent_assignment(run, work_item_id, assignment_id)?;
            if assignment.status != AuditAgentAssignmentStatus::Spawned {
                return Err(AuditStoreError::InvalidAgentAssignmentStatus {
                    assignment_id: assignment_id.to_string(),
                    status: format!("{:?}", assignment.status),
                });
            }
            assignment.agent_thread_id = Some(agent_thread_id.to_string());
            assignment.updated_at = now;
            let assignment = assignment.clone();
            run.updated_at = now;
            Ok(assignment)
        })?;
        self.publish_event(AuditStoreEvent::Activity {
            audit_id: run.id.clone(),
            category: "agent".to_string(),
            message: format!("{:?} agent spawned as {}", assignment.role, agent_thread_id),
            stage: stage_for_work_item(&run, Some(work_item_id)),
            work_item_id: Some(work_item_id.to_string()),
            artifact_ref: None,
            agent_role: Some(format!("{:?}", assignment.role)),
            tool_name: Some("spawn_agent".to_string()),
        });
        Ok(AgentAssignmentUpdate { run, assignment })
    }

    pub fn finish_agent_assignment(
        &self,
        run_id: &str,
        work_item_id: &str,
        assignment_id: &str,
        status: AuditAgentAssignmentStatus,
        reason: &str,
        now: i64,
    ) -> Result<AgentAssignmentUpdate, AuditStoreError> {
        if !matches!(
            status,
            AuditAgentAssignmentStatus::Failed | AuditAgentAssignmentStatus::Cancelled
        ) {
            return Err(AuditStoreError::InvalidAgentAssignmentStatus {
                assignment_id: assignment_id.to_string(),
                status: format!("{status:?}"),
            });
        }
        let activity_status = status.clone();
        let (run, assignment) = self.mutate_run(run_id, |run| {
            ensure_running(run)?;
            ensure_claimed_work_item(run, work_item_id)?;
            let assignment = ensure_agent_assignment(run, work_item_id, assignment_id)?;
            if matches!(assignment.status, AuditAgentAssignmentStatus::Completed) {
                return Err(AuditStoreError::InvalidAgentAssignmentStatus {
                    assignment_id: assignment_id.to_string(),
                    status: format!("{:?}", assignment.status),
                });
            }
            assignment.status = status;
            assignment
                .metadata
                .insert("finishReason".to_string(), json!(reason));
            assignment
                .metadata
                .insert("finishedAt".to_string(), json!(now));
            assignment.updated_at = now;
            let assignment = assignment.clone();
            run.updated_at = now;
            Ok(assignment)
        })?;
        self.publish_event(AuditStoreEvent::Activity {
            audit_id: run.id.clone(),
            category: "agent".to_string(),
            message: format!(
                "{:?} agent marked {:?}: {reason}",
                assignment.role, activity_status
            ),
            stage: stage_for_work_item(&run, Some(work_item_id)),
            work_item_id: Some(work_item_id.to_string()),
            artifact_ref: None,
            agent_role: Some(format!("{:?}", assignment.role)),
            tool_name: Some("wait_agent".to_string()),
        });
        Ok(AgentAssignmentUpdate { run, assignment })
    }

    pub fn record_router_evidence_for_current_work(
        &self,
        run_id: &str,
        mut evidence: AuditEvidence,
    ) -> Result<Option<(AuditRun, String)>, AuditStoreError> {
        evidence.id = Uuid::new_v4().to_string();
        let evidence_ref = format!("evidence/{}.json", evidence.id);
        let result = self
            .mutate_run(run_id, |run| {
                ensure_running(run)?;
                let Some(work_item_index) = current_claimed_work_item_index(run) else {
                    return Ok(None);
                };
                evidence.audit_run_id = run.id.clone();
                if evidence.adapter_id.is_none() {
                    evidence.adapter_id = run.adapter_id.clone();
                }
                apply_active_capability_dispatch(&mut evidence, &run.work_items[work_item_index]);
                evidence.work_item_id = Some(run.work_items[work_item_index].id.clone());
                self.write_json(
                    &self.audits_root.join(run_id).join(&evidence_ref),
                    &evidence,
                )?;
                run.work_items[work_item_index]
                    .evidence_refs
                    .push(evidence_ref.clone());
                run.evidence_refs.push(evidence_ref.clone());
                run.updated_at = evidence.created_at;
                Ok(Some(evidence_ref.clone()))
            })
            .map(|(run, evidence_ref)| evidence_ref.map(|evidence_ref| (run, evidence_ref)));
        if let Ok(Some((run, evidence_ref))) = &result
            && let Ok(evidence) = self.read_work_evidence(run_id, evidence_ref)
        {
            let activity = evidence_activity(&evidence);
            let work_item_id = evidence.work_item_id;
            self.publish_event(AuditStoreEvent::Activity {
                audit_id: run.id.clone(),
                category: activity.category,
                message: activity.message,
                stage: stage_for_work_item(run, work_item_id.as_deref()),
                work_item_id,
                artifact_ref: Some(evidence_ref.clone()),
                agent_role: None,
                tool_name: activity.tool_name,
            });
        }
        result
    }

    pub fn block_next_unavailable_scheduled_work(
        &self,
        run_id: &str,
        worker_id: &str,
        now: i64,
    ) -> Result<Option<ScheduledWorkBlock>, AuditStoreError> {
        let Some(run) = self.read_run(run_id)? else {
            return Err(AuditStoreError::RunNotFound(run_id.to_string()));
        };
        if run
            .work_items
            .iter()
            .any(|work_item| work_item.status == AuditWorkItemStatus::Claimed)
        {
            return Ok(None);
        }
        let Some(work_item) = run
            .work_items
            .iter()
            .find(|work_item| work_item.status == AuditWorkItemStatus::Pending)
        else {
            return Ok(None);
        };
        let Some(schedule) = schedule_metadata(&work_item.metadata) else {
            return Ok(None);
        };
        if schedule.action != AuditStageScheduleAction::RecordUnavailableAndContinue {
            return Ok(None);
        }
        let diagnostics = schedule
            .unavailable_capabilities
            .iter()
            .map(|capability| {
                format!(
                    "{} unavailable: {}",
                    capability.capability, capability.reason
                )
            })
            .collect::<Vec<_>>();
        let packet = json!({
            "schemaVersion": 1,
            "stage": work_item.stage,
            "workItemId": work_item.id,
            "action": "recordUnavailableAndContinue",
            "desiredCapabilities": schedule.desired_capabilities,
            "unavailableCapabilities": schedule.unavailable_capabilities,
            "diagnostics": diagnostics,
        });
        let Some((_, claimed)) = self.claim_work(run_id, worker_id, None, now)? else {
            return Ok(None);
        };
        let (_, artifact_ref) = self.record_packet(
            run_id,
            &claimed.id,
            "capabilityUnavailable",
            "Scheduled capability is unavailable; stage was blocked with a visible coverage gap.",
            packet,
            now,
        )?;
        let update = self.finish_work(
            run_id,
            &claimed.id,
            worker_id,
            AuditWorkItemStatus::Blocked,
            &[],
            now,
        )?;
        Ok(Some(ScheduledWorkBlock {
            run: update.run,
            work_item: update.work_item,
            artifact_ref,
            diagnostics,
        }))
    }

    pub fn finish_work(
        &self,
        run_id: &str,
        work_item_id: &str,
        worker_id: &str,
        status: AuditWorkItemStatus,
        evidence_refs: &[String],
        now: i64,
    ) -> Result<WorkUpdate, AuditStoreError> {
        if !matches!(
            status,
            AuditWorkItemStatus::Completed
                | AuditWorkItemStatus::Failed
                | AuditWorkItemStatus::Blocked
        ) {
            return Err(AuditStoreError::InvalidWorkStatus(format!("{status:?}")));
        }
        let (run, (work_item, previous_stage)) = self.mutate_run(run_id, |run| {
            ensure_running(run)?;
            let previous_stage = run.current_stage.clone();
            for evidence_ref in evidence_refs {
                if !run.evidence_refs.contains(evidence_ref) {
                    return Err(AuditStoreError::EvidenceNotFound(evidence_ref.clone()));
                }
            }
            ensure_agent_assignments_allow_work_status(run, work_item_id, &status)?;
            let work_item = ensure_work_item(run, work_item_id)?;
            if work_item.claimed_by.as_deref() != Some(worker_id) {
                return Err(AuditStoreError::WorkItemClaimedByOther {
                    work_item_id: work_item_id.to_string(),
                    claimed_by: work_item
                        .claimed_by
                        .clone()
                        .unwrap_or_else(|| "unclaimed".to_string()),
                });
            }
            self.ensure_schedule_allows_work_status(run_id, work_item, &status, evidence_refs)?;
            let schedule_gaps = if status == AuditWorkItemStatus::Blocked {
                schedule_coverage_gaps(work_item)
            } else {
                Vec::new()
            };
            work_item.status = status;
            work_item.updated_at = now;
            for evidence_ref in evidence_refs {
                if !work_item.evidence_refs.contains(evidence_ref) {
                    work_item.evidence_refs.push(evidence_ref.clone());
                }
            }
            stamp_schedule_outcome(work_item, now);
            let completed = work_item.clone();
            for gap in schedule_gaps {
                upsert_coverage_gap(&mut run.coverage_gaps, gap);
            }
            if let Some(next) = run
                .work_items
                .iter()
                .find(|item| item.status == AuditWorkItemStatus::Pending)
            {
                run.current_stage = next.stage.clone();
            }
            run.updated_at = now;
            Ok((completed, previous_stage))
        })?;
        self.publish_event(AuditStoreEvent::StageUpdated {
            audit_id: run.id.clone(),
            stage: work_item.stage.clone(),
            status: work_item_stage_status(&work_item),
            run: run.clone(),
        });
        self.publish_event(AuditStoreEvent::Activity {
            audit_id: run.id.clone(),
            category: "orchestrator".to_string(),
            message: format!("{:?} finished as {:?}", work_item.stage, work_item.status),
            stage: Some(format!("{:?}", work_item.stage)),
            work_item_id: Some(work_item.id.clone()),
            artifact_ref: None,
            agent_role: None,
            tool_name: None,
        });
        Ok(WorkUpdate {
            stage_changed: previous_stage != run.current_stage,
            run,
            work_item,
        })
    }

    fn ensure_schedule_allows_work_status(
        &self,
        run_id: &str,
        work_item: &AuditWorkItem,
        status: &AuditWorkItemStatus,
        evidence_refs: &[String],
    ) -> Result<(), AuditStoreError> {
        if *status != AuditWorkItemStatus::Completed {
            return Ok(());
        }
        if has_runtime_capability_gaps(work_item) {
            return Err(AuditStoreError::UnavailableScheduledCapabilities {
                work_item_id: work_item.id.clone(),
                capabilities: runtime_capability_gap_names(work_item),
            });
        }
        let Some(schedule) = schedule_metadata(&work_item.metadata) else {
            return Ok(());
        };
        if !schedule.unavailable_capabilities.is_empty() {
            let mut capabilities = schedule
                .unavailable_capabilities
                .into_iter()
                .map(|capability| capability.capability)
                .collect::<Vec<_>>();
            capabilities.sort();
            capabilities.dedup();
            return Err(AuditStoreError::UnavailableScheduledCapabilities {
                work_item_id: work_item.id.clone(),
                capabilities,
            });
        }
        if schedule.verification_methods.is_empty() {
            return Ok(());
        }

        let mut effective_evidence_refs = work_item.evidence_refs.clone();
        for evidence_ref in evidence_refs {
            if !effective_evidence_refs.contains(evidence_ref) {
                effective_evidence_refs.push(evidence_ref.clone());
            }
        }
        for evidence_ref in effective_evidence_refs {
            let evidence = self.read_work_evidence(run_id, &evidence_ref)?;
            if evidence.work_item_id.as_deref() == Some(work_item.id.as_str())
                && evidence.attestation != AuditEvidenceAttestation::ModelSubmitted
                && schedule
                    .verification_methods
                    .contains(&evidence.verification_method)
            {
                return Ok(());
            }
        }
        Err(AuditStoreError::MissingScheduledEvidence {
            work_item_id: work_item.id.clone(),
            verification_methods: schedule
                .verification_methods
                .into_iter()
                .map(|method| format!("{method:?}"))
                .collect(),
        })
    }

    fn read_work_evidence(
        &self,
        run_id: &str,
        evidence_ref: &str,
    ) -> Result<AuditEvidence, AuditStoreError> {
        let path = self.audits_root.join(run_id).join(evidence_ref);
        let body = std::fs::read_to_string(&path).map_err(|source| AuditStoreError::Io {
            action: "read audit evidence",
            source,
        })?;
        serde_json::from_str(&body).map_err(AuditStoreError::from)
    }

    pub(crate) fn mutate_run<T>(
        &self,
        run_id: &str,
        mutate: impl FnOnce(&mut AuditRun) -> Result<T, AuditStoreError>,
    ) -> Result<(AuditRun, T), AuditStoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let body = transaction
            .query_row(
                "SELECT body_json FROM audit_runs WHERE run_id = ?1",
                [run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| AuditStoreError::RunNotFound(run_id.to_string()))?;
        let mut run: AuditRun = serde_json::from_str(&body)?;
        let result = mutate(&mut run)?;
        let body_json = serde_json::to_string(&run)?;
        transaction.execute(
            "
            UPDATE audit_runs
            SET status = ?2, updated_at = ?3, body_json = ?4
            WHERE run_id = ?1
            ",
            params![run.id, status_name(&run)?, run.updated_at, body_json],
        )?;
        transaction.commit()?;
        Ok((run, result))
    }

    pub(crate) fn write_json(
        &self,
        path: &Path,
        value: &impl Serialize,
    ) -> Result<(), AuditStoreError> {
        let bytes = serde_json::to_vec_pretty(value)?;
        self.write_bytes(path, &bytes)
    }

    pub(crate) fn write_text(&self, path: &Path, value: &str) -> Result<(), AuditStoreError> {
        self.write_bytes(path, value.as_bytes())
    }

    pub fn read_artifact(
        &self,
        run_id: &str,
        artifact_ref: &str,
    ) -> Result<Vec<u8>, AuditStoreError> {
        validate_artifact_ref(artifact_ref)?;
        let audit_root = self
            .audits_root
            .join(run_id)
            .canonicalize()
            .map_err(|source| AuditStoreError::Io {
                action: "resolve audit root",
                source,
            })?;
        let artifact_path = audit_root.join(artifact_ref);
        let canonical_artifact =
            artifact_path
                .canonicalize()
                .map_err(|source| AuditStoreError::Io {
                    action: "resolve audit artifact",
                    source,
                })?;
        if !canonical_artifact.starts_with(&audit_root) || !canonical_artifact.is_file() {
            return Err(AuditStoreError::InvalidArtifactPath);
        }
        let metadata = fs::metadata(&canonical_artifact).map_err(|source| AuditStoreError::Io {
            action: "inspect audit artifact",
            source,
        })?;
        if metadata.len() > MAX_ARTIFACT_BYTES as u64 {
            return Err(AuditStoreError::ArtifactTooLarge(MAX_ARTIFACT_BYTES));
        }
        fs::read(&canonical_artifact).map_err(|source| AuditStoreError::Io {
            action: "read audit artifact",
            source,
        })
    }

    fn write_bytes(&self, path: &Path, bytes: &[u8]) -> Result<(), AuditStoreError> {
        if bytes.len() > MAX_ARTIFACT_BYTES {
            return Err(AuditStoreError::ArtifactTooLarge(MAX_ARTIFACT_BYTES));
        }
        let audit_root = path
            .parent()
            .and_then(Path::parent)
            .ok_or(AuditStoreError::InvalidArtifactPath)?
            .canonicalize()
            .map_err(|source| AuditStoreError::Io {
                action: "resolve audit root",
                source,
            })?;
        let parent = path
            .parent()
            .ok_or(AuditStoreError::InvalidArtifactPath)?
            .canonicalize()
            .map_err(|source| AuditStoreError::Io {
                action: "resolve audit artifact directory",
                source,
            })?;
        if !parent.starts_with(&audit_root) || path.exists() {
            return Err(AuditStoreError::InvalidArtifactPath);
        }
        let temporary = path.with_extension("json.tmp");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|source| AuditStoreError::Io {
                action: "create audit artifact",
                source,
            })?;
        file.write_all(bytes)
            .map_err(|source| AuditStoreError::Io {
                action: "write audit artifact",
                source,
            })?;
        file.sync_all().map_err(|source| AuditStoreError::Io {
            action: "sync audit artifact",
            source,
        })?;
        std::fs::rename(&temporary, path).map_err(|source| AuditStoreError::Io {
            action: "commit audit artifact",
            source,
        })
    }
}

fn validate_artifact_ref(artifact_ref: &str) -> Result<(), AuditStoreError> {
    let path = Path::new(artifact_ref);
    if artifact_ref.is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(AuditStoreError::InvalidArtifactPath);
    }
    Ok(())
}

fn work_item_stage_status(work_item: &AuditWorkItem) -> AuditStageStatus {
    if work_item.status == AuditWorkItemStatus::Blocked
        && (has_unavailable_schedule(work_item) || has_runtime_capability_gaps(work_item))
    {
        return AuditStageStatus::Unavailable;
    }
    match work_item.status {
        AuditWorkItemStatus::Pending => AuditStageStatus::Pending,
        AuditWorkItemStatus::Claimed => AuditStageStatus::Running,
        AuditWorkItemStatus::Completed => AuditStageStatus::Succeeded,
        AuditWorkItemStatus::Failed => AuditStageStatus::Failed,
        AuditWorkItemStatus::Blocked => AuditStageStatus::Blocked,
        AuditWorkItemStatus::Cancelled => AuditStageStatus::Cancelled,
    }
}

fn has_unavailable_schedule(work_item: &AuditWorkItem) -> bool {
    schedule_metadata(&work_item.metadata)
        .map(|schedule| !schedule.unavailable_capabilities.is_empty())
        .unwrap_or(false)
}

fn has_runtime_capability_gaps(work_item: &AuditWorkItem) -> bool {
    work_item
        .metadata
        .get("runtimeCapabilityGaps")
        .and_then(Value::as_array)
        .map(|gaps| !gaps.is_empty())
        .unwrap_or(false)
}

fn runtime_capability_gap_names(work_item: &AuditWorkItem) -> Vec<String> {
    work_item
        .metadata
        .get("runtimeCapabilityGaps")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|gap| gap.get("capability").and_then(Value::as_str))
        .map(str::to_string)
        .collect()
}

fn upsert_coverage_gap(coverage_gaps: &mut Vec<AuditCoverageGap>, gap: AuditCoverageGap) {
    if let Some(existing) = coverage_gaps
        .iter_mut()
        .find(|existing| existing.capability == gap.capability && existing.stage == gap.stage)
    {
        existing.reason = gap.reason;
        existing.affects_terminal_status = gap.affects_terminal_status;
        return;
    }
    coverage_gaps.push(gap);
}

fn schedule_coverage_gaps(work_item: &AuditWorkItem) -> Vec<AuditCoverageGap> {
    let Some(schedule) = schedule_metadata(&work_item.metadata) else {
        return Vec::new();
    };
    schedule
        .unavailable_capabilities
        .into_iter()
        .map(|capability| AuditCoverageGap {
            capability: capability.capability,
            stage: work_item.stage.clone(),
            reason: capability.reason,
            affects_terminal_status: true,
        })
        .collect()
}

fn append_runtime_capability_gap(
    metadata: &mut Metadata,
    capability: &str,
    reason: &str,
    provider_id: Option<&str>,
    tool_name: Option<&str>,
    now: i64,
) {
    let mut gaps = metadata
        .get("runtimeCapabilityGaps")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if let Some(existing) = gaps
        .iter_mut()
        .find(|gap| gap.get("capability").and_then(Value::as_str) == Some(capability))
    {
        *existing = json!({
            "schemaVersion": 1,
            "capability": capability,
            "reason": reason,
            "providerId": provider_id,
            "toolName": tool_name,
            "affectsTerminalStatus": true,
            "updatedAt": now,
        });
    } else {
        gaps.push(json!({
            "schemaVersion": 1,
            "capability": capability,
            "reason": reason,
            "providerId": provider_id,
            "toolName": tool_name,
            "affectsTerminalStatus": true,
            "updatedAt": now,
        }));
    }
    metadata.insert("runtimeCapabilityGaps".to_string(), Value::Array(gaps));
}

fn apply_active_capability_dispatch(evidence: &mut AuditEvidence, work_item: &AuditWorkItem) {
    let Some(dispatch) = work_item.metadata.get("activeCapabilityDispatch") else {
        return;
    };
    let Some(capability) = dispatch.get("capability").and_then(Value::as_str) else {
        return;
    };
    evidence.verification_method = verification_method_for_capability(capability);
    evidence
        .metadata
        .insert("capability".to_string(), json!(capability));
    if let Some(provider_id) = dispatch.get("providerId").and_then(Value::as_str) {
        evidence.provider_id = provider_id.to_string();
    }
    if let Some(adapter_id) = dispatch.get("adapterId").and_then(Value::as_str) {
        evidence.adapter_id = Some(adapter_id.to_string());
    }
}

fn verification_method_for_capability(capability: &str) -> VerificationMethod {
    match capability {
        "graph.analysis" => VerificationMethod::GraphAnalysis,
        "bytecode.analysis" => VerificationMethod::BytecodeAnalysis,
        "dynamic.fuzzing" => VerificationMethod::Fuzzing,
        "symbolic.execution" => VerificationMethod::SymbolicExecution,
        "formal.verification" => VerificationMethod::FormalVerification,
        "economic.simulation" => VerificationMethod::EconomicSimulation,
        "exploit.replay" => VerificationMethod::ExploitReplay,
        "static.analysis" => VerificationMethod::StaticAnalysis,
        "target.acquire" | "target.normalize" => VerificationMethod::StaticAnalysis,
        _ => VerificationMethod::StaticAnalysis,
    }
}

struct EvidenceActivity {
    category: String,
    message: String,
    tool_name: Option<String>,
}

fn evidence_activity(evidence: &AuditEvidence) -> EvidenceActivity {
    let category = if evidence.attestation == AuditEvidenceAttestation::RouterCaptured {
        "tool"
    } else {
        "evidence"
    }
    .to_string();
    let tool_name = (!evidence.tool_name.is_empty()).then(|| evidence.tool_name.clone());
    let mut source = evidence.provider_id.clone();
    if let Some(tool_name) = &tool_name {
        source = format!("{source}/{tool_name}");
    }
    EvidenceActivity {
        category,
        message: format!(
            "{:?} evidence recorded from {}: {}",
            evidence.verification_method, source, evidence.summary
        ),
        tool_name,
    }
}

fn packet_activity_category(packet_kind: &str) -> &'static str {
    match packet_kind {
        "capabilityDispatch" => "tool",
        "capabilityUnavailable" => "coverage",
        "findingCandidate" | "findingValidation" => "judging",
        "report" | "finalReport" => "report",
        _ => "orchestrator",
    }
}

fn packet_activity_message(packet_kind: &str, summary: &str, packet: &Value) -> String {
    if packet_kind == "capabilityDispatch"
        && let Some(capability) = packet.get("capability").and_then(Value::as_str)
    {
        let via = packet
            .get("toolName")
            .and_then(Value::as_str)
            .map(|tool_name| format!(" via {tool_name}"))
            .unwrap_or_else(|| " via announced tool discovery".to_string());
        return format!("preparing {capability}{via}");
    }
    if packet_kind == "agentReview"
        && let Some(role) = packet.get("role").and_then(Value::as_str)
    {
        return format!("{role} review packet recorded: {summary}");
    }
    format!("{packet_kind}: {summary}")
}

fn stage_for_work_item(run: &AuditRun, work_item_id: Option<&str>) -> Option<String> {
    let work_item_id = work_item_id?;
    run.work_items
        .iter()
        .find(|item| item.id == work_item_id)
        .map(|item| format!("{:?}", item.stage))
}

fn stamp_schedule_outcome(work_item: &mut AuditWorkItem, now: i64) {
    let Some(schedule) = schedule_metadata(&work_item.metadata) else {
        return;
    };
    let status = match work_item.status {
        AuditWorkItemStatus::Pending => "pending",
        AuditWorkItemStatus::Claimed => "claimed",
        AuditWorkItemStatus::Completed => "completed",
        AuditWorkItemStatus::Failed => "failed",
        AuditWorkItemStatus::Blocked => "blocked",
        AuditWorkItemStatus::Cancelled => "cancelled",
    };
    work_item.metadata.insert(
        "stageScheduleOutcome".to_string(),
        json!({
            "schemaVersion": 1,
            "status": status,
            "updatedAt": now,
            "desiredCapabilities": schedule.desired_capabilities,
            "unavailableCapabilities": schedule
                .unavailable_capabilities
                .into_iter()
                .map(|capability| json!({
                    "capability": capability.capability,
                    "reason": capability.reason,
                }))
                .collect::<Vec<_>>(),
            "evidenceRefs": work_item.evidence_refs.clone(),
            "runtimeCapabilityGaps": work_item
                .metadata
                .get("runtimeCapabilityGaps")
                .cloned()
                .unwrap_or_else(|| json!([])),
        }),
    );
}

fn ensure_running(run: &AuditRun) -> Result<(), AuditStoreError> {
    if run.status == AuditRunStatus::Running {
        Ok(())
    } else {
        Err(AuditStoreError::InvalidRunState {
            run_id: run.id.clone(),
            status: format!("{:?}", run.status),
        })
    }
}

fn ensure_work_item<'a>(
    run: &'a mut AuditRun,
    work_item_id: &str,
) -> Result<&'a mut AuditWorkItem, AuditStoreError> {
    run.work_items
        .iter_mut()
        .find(|item| item.id == work_item_id)
        .ok_or_else(|| AuditStoreError::WorkItemNotFound(work_item_id.to_string()))
}

fn ensure_claimed_work_item<'a>(
    run: &'a mut AuditRun,
    work_item_id: &str,
) -> Result<&'a mut AuditWorkItem, AuditStoreError> {
    let work_item = ensure_work_item(run, work_item_id)?;
    if work_item.status != AuditWorkItemStatus::Claimed {
        return Err(AuditStoreError::WorkItemNotClaimed(
            work_item_id.to_string(),
        ));
    }
    Ok(work_item)
}

fn ensure_agent_assignment<'a>(
    run: &'a mut AuditRun,
    work_item_id: &str,
    assignment_id: &str,
) -> Result<&'a mut AuditAgentAssignment, AuditStoreError> {
    run.agent_assignments
        .iter_mut()
        .find(|assignment| {
            assignment.id == assignment_id && assignment.work_item_id == work_item_id
        })
        .ok_or_else(|| AuditStoreError::AgentAssignmentNotFound(assignment_id.to_string()))
}

fn ensure_agent_assignment_claim_order(
    run: &AuditRun,
    work_item_id: &str,
    assignment_id: &str,
) -> Result<(), AuditStoreError> {
    let mut blocked_by = Vec::new();
    for assignment in run
        .agent_assignments
        .iter()
        .filter(|assignment| assignment.work_item_id == work_item_id)
    {
        if assignment.id == assignment_id {
            if blocked_by.is_empty() {
                return Ok(());
            }
            return Err(AuditStoreError::AgentAssignmentBlocked {
                assignment_id: assignment_id.to_string(),
                blocked_by,
            });
        }
        if assignment.status != AuditAgentAssignmentStatus::Completed {
            blocked_by.push(assignment.id.clone());
        }
    }
    Ok(())
}

fn ensure_agent_assignments_allow_work_status(
    run: &AuditRun,
    work_item_id: &str,
    status: &AuditWorkItemStatus,
) -> Result<(), AuditStoreError> {
    if *status != AuditWorkItemStatus::Completed {
        return Ok(());
    }
    let assignments = run
        .agent_assignments
        .iter()
        .filter(|assignment| assignment.work_item_id == work_item_id)
        .collect::<Vec<_>>();
    let incomplete = assignments
        .iter()
        .filter(|assignment| {
            matches!(
                assignment.status,
                AuditAgentAssignmentStatus::Pending | AuditAgentAssignmentStatus::Spawned
            )
        })
        .map(|assignment| assignment.id.clone())
        .collect::<Vec<_>>();
    if !incomplete.is_empty() {
        return Err(AuditStoreError::IncompleteAgentAssignments {
            work_item_id: work_item_id.to_string(),
            assignment_ids: incomplete,
        });
    }
    let failed = assignments
        .iter()
        .filter(|assignment| {
            matches!(
                assignment.status,
                AuditAgentAssignmentStatus::Failed | AuditAgentAssignmentStatus::Cancelled
            )
        })
        .map(|assignment| assignment.id.clone())
        .collect::<Vec<_>>();
    if !failed.is_empty() {
        return Err(AuditStoreError::FailedAgentAssignments {
            work_item_id: work_item_id.to_string(),
            assignment_ids: failed,
        });
    }
    Ok(())
}

fn current_claimed_work_item_index(run: &AuditRun) -> Option<usize> {
    run.work_items
        .iter()
        .position(|item| {
            item.status == AuditWorkItemStatus::Claimed && item.stage == run.current_stage
        })
        .or_else(|| {
            run.work_items
                .iter()
                .position(|item| item.status == AuditWorkItemStatus::Claimed)
        })
}
