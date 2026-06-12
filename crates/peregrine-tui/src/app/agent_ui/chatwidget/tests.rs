//! Exercises `ChatWidget` event handling and rendering invariants.
//!
//! These tests cover both app-server-native inputs and focused widget helpers. Many assertions are
//! snapshot-based so that layout regressions and status/header changes show up as stable,
//! reviewable diffs.

pub(super) use super::*;
pub(super) use crate::agent::app_command::AppCommand as Op;
pub(super) use crate::agent::app_event::AppEvent;
pub(super) use crate::agent::app_event::ExitMode;
#[cfg(not(target_os = "linux"))]
pub(super) use crate::agent::app_event::RealtimeAudioDeviceKind;
pub(super) use crate::agent::app_event_sender::AppEventSender;
pub(super) use crate::agent::approval_events::ApplyPatchApprovalRequestEvent;
pub(super) use crate::agent::approval_events::ExecApprovalRequestEvent;
pub(super) use crate::agent::bottom_pane::LocalImageAttachment;
pub(super) use crate::agent::bottom_pane::MentionBinding;
pub(super) use crate::agent::bottom_pane::QueuedInputAction;
pub(super) use crate::agent::chatwidget::realtime::RealtimeConversationPhase;
pub(super) use crate::agent::diff_model::FileChange;
pub(super) use crate::agent::history_cell::UserHistoryCell;
pub(super) use crate::agent::legacy_core::config::Config;
pub(super) use crate::agent::legacy_core::config::ConfigBuilder;
pub(super) use crate::agent::model_catalog::ModelCatalog;
pub(super) use crate::agent::test_backend::VT100Backend;
pub(super) use crate::agent::test_support::PathBufExt;
pub(super) use crate::agent::test_support::test_path_buf;
pub(super) use crate::agent::test_support::test_path_display;
pub(super) use crate::agent::token_usage::TokenUsage;
pub(super) use crate::agent::token_usage::TokenUsageInfo;
pub(super) use crate::agent::tui::FrameRequester;
pub(super) use assert_matches::assert_matches;
pub(super) use codex_core_plugins::OPENAI_CURATED_MARKETPLACE_NAME;
pub(super) use codex_core_skills::model::SkillMetadata;
pub(super) use codex_features::FEATURES;
pub(super) use codex_features::Feature;
pub(super) use codex_git_utils::CommitLogEntry;
pub(super) use codex_models_manager::test_support::construct_model_info_offline_for_tests;
pub(super) use codex_models_manager::test_support::get_model_offline_for_tests;
pub(super) use codex_otel::RuntimeMetricsSummary;
pub(super) use codex_otel::SessionTelemetry;
pub(super) use codex_terminal_detection::Multiplexer;
pub(super) use codex_terminal_detection::TerminalInfo;
pub(super) use codex_terminal_detection::TerminalName;
pub(super) use codex_utils_absolute_path::AbsolutePathBuf;
pub(super) use codex_utils_approval_presets::builtin_approval_presets;
pub(super) use crossterm::event::KeyCode;
pub(super) use crossterm::event::KeyEvent;
pub(super) use crossterm::event::KeyModifiers;
pub(super) use insta::assert_snapshot;
pub(super) use peregrine_app_server_protocol::AddCreditsNudgeCreditType;
pub(super) use peregrine_app_server_protocol::AddCreditsNudgeEmailStatus;
pub(super) use peregrine_app_server_protocol::AdditionalFileSystemPermissions as AppServerAdditionalFileSystemPermissions;
pub(super) use peregrine_app_server_protocol::AdditionalNetworkPermissions as AppServerAdditionalNetworkPermissions;
pub(super) use peregrine_app_server_protocol::AdditionalPermissionProfile as AppServerAdditionalPermissionProfile;
pub(super) use peregrine_app_server_protocol::AppSummary;
pub(super) use peregrine_app_server_protocol::AutoReviewDecisionSource as AppServerGuardianApprovalReviewDecisionSource;
pub(super) use peregrine_app_server_protocol::CollabAgentState as AppServerCollabAgentState;
pub(super) use peregrine_app_server_protocol::CollabAgentStatus as AppServerCollabAgentStatus;
pub(super) use peregrine_app_server_protocol::CollabAgentTool as AppServerCollabAgentTool;
pub(super) use peregrine_app_server_protocol::CollabAgentToolCallStatus as AppServerCollabAgentToolCallStatus;
pub(super) use peregrine_app_server_protocol::CommandAction as AppServerCommandAction;
pub(super) use peregrine_app_server_protocol::CommandExecutionRequestApprovalParams as AppServerCommandExecutionRequestApprovalParams;
pub(super) use peregrine_app_server_protocol::CommandExecutionSource as ExecCommandSource;
pub(super) use peregrine_app_server_protocol::CommandExecutionSource as AppServerCommandExecutionSource;
pub(super) use peregrine_app_server_protocol::CommandExecutionStatus as AppServerCommandExecutionStatus;
pub(super) use peregrine_app_server_protocol::ConfigWarningNotification;
pub(super) use peregrine_app_server_protocol::CreditsSnapshot;
pub(super) use peregrine_app_server_protocol::ErrorNotification;
pub(super) use peregrine_app_server_protocol::ExecPolicyAmendment;
pub(super) use peregrine_app_server_protocol::FileUpdateChange;
pub(super) use peregrine_app_server_protocol::GuardianApprovalReview;
pub(super) use peregrine_app_server_protocol::GuardianApprovalReviewAction as AppServerGuardianApprovalReviewAction;
pub(super) use peregrine_app_server_protocol::GuardianApprovalReviewStatus;
pub(super) use peregrine_app_server_protocol::GuardianCommandSource as AppServerGuardianCommandSource;
pub(super) use peregrine_app_server_protocol::GuardianRiskLevel as AppServerGuardianRiskLevel;
pub(super) use peregrine_app_server_protocol::GuardianUserAuthorization as AppServerGuardianUserAuthorization;
pub(super) use peregrine_app_server_protocol::GuardianWarningNotification;
pub(super) use peregrine_app_server_protocol::HookCompletedNotification as AppServerHookCompletedNotification;
pub(super) use peregrine_app_server_protocol::HookEventName as AppServerHookEventName;
pub(super) use peregrine_app_server_protocol::HookExecutionMode as AppServerHookExecutionMode;
pub(super) use peregrine_app_server_protocol::HookHandlerType as AppServerHookHandlerType;
pub(super) use peregrine_app_server_protocol::HookOutputEntry as AppServerHookOutputEntry;
pub(super) use peregrine_app_server_protocol::HookOutputEntryKind as AppServerHookOutputEntryKind;
pub(super) use peregrine_app_server_protocol::HookRunStatus as AppServerHookRunStatus;
pub(super) use peregrine_app_server_protocol::HookRunSummary as AppServerHookRunSummary;
pub(super) use peregrine_app_server_protocol::HookScope as AppServerHookScope;
pub(super) use peregrine_app_server_protocol::HookStartedNotification as AppServerHookStartedNotification;
pub(super) use peregrine_app_server_protocol::ItemCompletedNotification;
pub(super) use peregrine_app_server_protocol::ItemGuardianApprovalReviewCompletedNotification;
pub(super) use peregrine_app_server_protocol::ItemGuardianApprovalReviewStartedNotification;
pub(super) use peregrine_app_server_protocol::ItemStartedNotification;
pub(super) use peregrine_app_server_protocol::MarketplaceAddResponse;
pub(super) use peregrine_app_server_protocol::MarketplaceInterface;
pub(super) use peregrine_app_server_protocol::MarketplaceUpgradeErrorInfo;
pub(super) use peregrine_app_server_protocol::MarketplaceUpgradeResponse;
pub(super) use peregrine_app_server_protocol::McpServerStartupState;
pub(super) use peregrine_app_server_protocol::McpServerStatusDetail;
pub(super) use peregrine_app_server_protocol::McpServerStatusUpdatedNotification;
pub(super) use peregrine_app_server_protocol::ModelVerification as AppServerModelVerification;
pub(super) use peregrine_app_server_protocol::ModelVerificationNotification;
pub(super) use peregrine_app_server_protocol::NonSteerableTurnKind;
pub(super) use peregrine_app_server_protocol::PatchApplyStatus as AppServerPatchApplyStatus;
pub(super) use peregrine_app_server_protocol::PatchChangeKind;
pub(super) use peregrine_app_server_protocol::PeregrineErrorInfo;
pub(super) use peregrine_app_server_protocol::PermissionsRequestApprovalParams as AppServerPermissionsRequestApprovalParams;
pub(super) use peregrine_app_server_protocol::PluginAuthPolicy;
pub(super) use peregrine_app_server_protocol::PluginDetail;
pub(super) use peregrine_app_server_protocol::PluginInstallPolicy;
pub(super) use peregrine_app_server_protocol::PluginInterface;
pub(super) use peregrine_app_server_protocol::PluginListResponse;
pub(super) use peregrine_app_server_protocol::PluginMarketplaceEntry;
pub(super) use peregrine_app_server_protocol::PluginReadResponse;
pub(super) use peregrine_app_server_protocol::PluginSource;
pub(super) use peregrine_app_server_protocol::PluginSummary;
pub(super) use peregrine_app_server_protocol::RateLimitReachedType;
pub(super) use peregrine_app_server_protocol::RateLimitSnapshot;
pub(super) use peregrine_app_server_protocol::RateLimitWindow;
pub(super) use peregrine_app_server_protocol::ReasoningSummaryTextDeltaNotification;
pub(super) use peregrine_app_server_protocol::ReviewTarget;
pub(super) use peregrine_app_server_protocol::ServerNotification;
pub(super) use peregrine_app_server_protocol::SkillSummary;
pub(super) use peregrine_app_server_protocol::ThreadClosedNotification;
pub(super) use peregrine_app_server_protocol::ThreadItem as AppServerThreadItem;
pub(super) use peregrine_app_server_protocol::ThreadRealtimeClosedNotification;
pub(super) use peregrine_app_server_protocol::ThreadRealtimeErrorNotification;
pub(super) use peregrine_app_server_protocol::ToolRequestUserInputOption;
pub(super) use peregrine_app_server_protocol::ToolRequestUserInputParams;
pub(super) use peregrine_app_server_protocol::ToolRequestUserInputQuestion;
pub(super) use peregrine_app_server_protocol::Turn as AppServerTurn;
pub(super) use peregrine_app_server_protocol::TurnCompletedNotification;
pub(super) use peregrine_app_server_protocol::TurnError as AppServerTurnError;
pub(super) use peregrine_app_server_protocol::TurnStartedNotification;
pub(super) use peregrine_app_server_protocol::TurnStatus as AppServerTurnStatus;
pub(super) use peregrine_app_server_protocol::UserInput;
pub(super) use peregrine_app_server_protocol::UserInput as AppServerUserInput;
pub(super) use peregrine_app_server_protocol::WarningNotification;
pub(super) use peregrine_config::ConfigLayerStack;
pub(super) use peregrine_config::Constrained;
pub(super) use peregrine_config::ConstraintError;
pub(super) use peregrine_config::RequirementSource;
pub(super) use peregrine_config::types::ApprovalsReviewer;
pub(super) use peregrine_config::types::Notifications;
pub(super) use peregrine_config::types::WindowsSandboxModeToml;
pub(super) use peregrine_types::ThreadId;
pub(super) use peregrine_types::account::PlanType;
pub(super) use peregrine_types::approvals::GuardianAssessmentAction;
pub(super) use peregrine_types::approvals::GuardianAssessmentDecisionSource;
pub(super) use peregrine_types::approvals::GuardianAssessmentEvent;
pub(super) use peregrine_types::approvals::GuardianAssessmentStatus;
pub(super) use peregrine_types::approvals::GuardianCommandSource;
pub(super) use peregrine_types::approvals::GuardianRiskLevel;
pub(super) use peregrine_types::approvals::GuardianUserAuthorization;
pub(super) use peregrine_types::config_types::CollaborationMode;
pub(super) use peregrine_types::config_types::ModeKind;
pub(super) use peregrine_types::config_types::Personality;
pub(super) use peregrine_types::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
pub(super) use peregrine_types::config_types::ServiceTier;
pub(super) use peregrine_types::models::ActivePermissionProfile;
pub(super) use peregrine_types::models::BUILT_IN_PERMISSION_PROFILE_WORKSPACE;
pub(super) use peregrine_types::models::FileSystemPermissions;
pub(super) use peregrine_types::models::MessagePhase;
pub(super) use peregrine_types::models::NetworkPermissions;
pub(super) use peregrine_types::models::PermissionProfile;
pub(super) use peregrine_types::openai_models::ModelInfo;
pub(super) use peregrine_types::openai_models::ModelPreset;
pub(super) use peregrine_types::openai_models::ModelsResponse;
pub(super) use peregrine_types::openai_models::ReasoningEffortPreset;
pub(super) use peregrine_types::openai_models::default_input_modalities;
pub(super) use peregrine_types::parse_command::ParsedCommand;
pub(super) use peregrine_types::plan_tool::PlanItemArg;
pub(super) use peregrine_types::plan_tool::StepStatus;
pub(super) use peregrine_types::plan_tool::UpdatePlanArgs;
pub(super) use peregrine_types::request_permissions::RequestPermissionProfile;
pub(super) use peregrine_types::user_input::TextElement;
pub(super) use serde_json::json;
#[cfg(target_os = "windows")]
pub(super) use serial_test::serial;
pub(super) use std::collections::HashMap;
pub(super) use std::path::PathBuf;
pub(super) use tempfile::NamedTempFile;
pub(super) use tempfile::tempdir;
pub(super) use tokio::sync::mpsc::error::TryRecvError;
pub(super) use tokio::sync::mpsc::unbounded_channel;
pub(super) use toml::Value as TomlValue;

pub(super) fn chatwidget_snapshot_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/app/agent_ui/chatwidget/snapshots")
}

macro_rules! assert_chatwidget_snapshot {
    ($name:expr, $value:expr $(,)?) => {{
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        settings.set_snapshot_path(crate::agent::chatwidget::tests::chatwidget_snapshot_dir());
        settings.bind(|| {
            insta::assert_snapshot!(format!("peregrine_tui__chatwidget__tests__{}", $name), $value);
        });
    }};
    ($name:expr, $value:expr, @$snapshot:literal $(,)?) => {{
        let mut settings = insta::Settings::clone_current();
        settings.set_prepend_module_to_snapshot(false);
        settings.set_snapshot_path(crate::agent::chatwidget::tests::chatwidget_snapshot_dir());
        settings.bind(|| {
            insta::assert_snapshot!(
                format!("peregrine_tui__chatwidget__tests__{}", $name),
                &($value),
                @$snapshot
            );
        });
    }};
}

mod app_server;
mod approval_requests;
mod composer_submission;
mod exec_flow;
mod goal_menu;
mod goal_validation;
mod guardian;
mod helpers;
mod history_replay;
mod mcp_startup;
mod permissions;
mod plan_mode;
mod popups_and_settings;
mod review_mode;
mod side;
mod slash_commands;
mod status_and_layout;
mod status_command_tests;
mod status_surface_previews;
mod terminal_title;

pub(crate) use helpers::make_chatwidget_manual_with_sender;
pub(crate) use helpers::set_chatgpt_auth;
pub(crate) use helpers::set_fast_mode_test_catalog;
pub(super) use helpers::*;
