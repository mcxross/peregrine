use crate::exec::{ExecCapturePolicy, ExecExpiration, ExecParams};
use crate::exec_env::create_env;
use crate::function_tool::FunctionCallError;
use crate::sandboxing::SandboxPermissions;
use crate::tools::context::{ToolInvocation, ToolOutput, ToolPayload, boxed_tool_output};
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::security::read_tools::{resolve_project_root, to_model_error};
use crate::tools::handlers::shell::{RunExecLikeArgs, run_exec_like};
use crate::tools::registry::{CoreToolRuntime, ToolExecutor};
use crate::tools::runtimes::shell::ShellRuntimeBackend;
use crate::tools::tool_search_entry::ToolSearchInfo;
use codex_tools::{ToolName, ToolSpec};
use codex_utils_absolute_path::AbsolutePathBuf;
use peregrine_security_tools::{
    SecurityCommand, SecuritySuiCommandKind, build_formal_verify_command, build_movy_fuzz_command,
    build_sui_package_command, resolve_move_package,
};
use serde::Deserialize;
use std::time::Duration;

use super::{spec, tool_search_info};

const DEFAULT_SUI_COMMAND_TIMEOUT_MS: u64 = 120_000;
const DEFAULT_MOVY_FUZZ_TIME_LIMIT_SECONDS: u64 = 30;
const DEFAULT_MOVY_FUZZ_SEED: u64 = 1;
const COMMAND_TIMEOUT_CUSHION_SECONDS: u64 = 30;

pub(crate) struct SuiCommandHandler;
pub(crate) struct MovyFuzzHandler;
pub(crate) struct FormalVerifyHandler;

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for SuiCommandHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(spec::SUI_COMMAND)
    }

    fn spec(&self) -> ToolSpec {
        spec::create_sui_command_tool()
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let args: SuiCommandArgs = parse_function_args(&invocation.payload, spec::SUI_COMMAND)?;
        let project_root =
            resolve_project_root(invocation.turn.as_ref(), args.project_root.as_deref())?;
        let ctx = resolve_move_package(project_root, args.package_path.as_deref())
            .map_err(to_model_error)?;
        let kind = SecuritySuiCommandKind::parse(&args.command_kind).map_err(to_model_error)?;
        let command = build_sui_package_command(
            &ctx,
            &invocation.turn.config.sui_security_tools.adapter,
            kind,
            args.publish_build_env.as_deref(),
            args.with_unpublished_dependencies.unwrap_or(false),
        )
        .map_err(to_model_error)?;

        run_security_command(
            invocation,
            command,
            args.timeout_ms.unwrap_or(DEFAULT_SUI_COMMAND_TIMEOUT_MS),
        )
        .await
    }
}

impl CoreToolRuntime for SuiCommandHandler {
    fn waits_for_runtime_cancellation(&self) -> bool {
        true
    }

    fn search_info(&self) -> Option<ToolSearchInfo> {
        tool_search_info(
            self.spec(),
            "sui move build test coverage dry-run randomized tests package command adapter bundled system security".to_string(),
        )
    }
}

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for MovyFuzzHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(spec::MOVY_FUZZ)
    }

    fn spec(&self) -> ToolSpec {
        spec::create_movy_fuzz_tool()
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let args: MovyFuzzArgs = parse_function_args(&invocation.payload, spec::MOVY_FUZZ)?;
        let project_root =
            resolve_project_root(invocation.turn.as_ref(), args.project_root.as_deref())?;
        let ctx = resolve_move_package(project_root, args.package_path.as_deref())
            .map_err(to_model_error)?;
        let time_limit_seconds = args
            .time_limit_seconds
            .unwrap_or(DEFAULT_MOVY_FUZZ_TIME_LIMIT_SECONDS);
        let seed = args.seed.unwrap_or(DEFAULT_MOVY_FUZZ_SEED);
        let command =
            build_movy_fuzz_command(&ctx, time_limit_seconds, seed).map_err(to_model_error)?;
        let timeout_ms =
            timeout_seconds_to_ms(time_limit_seconds + COMMAND_TIMEOUT_CUSHION_SECONDS);

        run_security_command(invocation, command, timeout_ms).await
    }
}

impl CoreToolRuntime for MovyFuzzHandler {
    fn waits_for_runtime_cancellation(&self) -> bool {
        true
    }

    fn search_info(&self) -> Option<ToolSearchInfo> {
        tool_search_info(
            self.spec(),
            "sui move movy local executor fuzz fuzzing public functions dynamic analysis security"
                .to_string(),
        )
    }
}

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for FormalVerifyHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(spec::FORMAL_VERIFY)
    }

    fn spec(&self) -> ToolSpec {
        spec::create_formal_verify_tool()
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let args: FormalVerifyArgs = parse_function_args(&invocation.payload, spec::FORMAL_VERIFY)?;
        let project_root =
            resolve_project_root(invocation.turn.as_ref(), args.project_root.as_deref())?;
        let ctx = resolve_move_package(project_root, args.package_path.as_deref())
            .map_err(to_model_error)?;
        let command = build_formal_verify_command(
            &ctx,
            &args.file_path,
            &args.module_name,
            args.timeout_seconds,
            false,
            false,
        )
        .map_err(to_model_error)?;
        let timeout_seconds =
            args.timeout_seconds.unwrap_or(45) as u64 + COMMAND_TIMEOUT_CUSHION_SECONDS;

        run_security_command(invocation, command, timeout_seconds_to_ms(timeout_seconds)).await
    }
}

impl CoreToolRuntime for FormalVerifyHandler {
    fn waits_for_runtime_cancellation(&self) -> bool {
        true
    }

    fn search_info(&self) -> Option<ToolSearchInfo> {
        tool_search_info(
            self.spec(),
            "sui move formal verification prover sui-prover module specification security"
                .to_string(),
        )
    }
}

async fn run_security_command(
    invocation: ToolInvocation,
    command: SecurityCommand,
    timeout_ms: u64,
) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
    let ToolInvocation {
        session,
        turn,
        cancellation_token,
        tracker,
        call_id,
        ..
    } = invocation;
    let cwd = AbsolutePathBuf::from_absolute_path(&command.cwd).map_err(|error| {
        FunctionCallError::RespondToModel(format!(
            "failed to resolve security command working directory {}: {error}",
            command.cwd.display()
        ))
    })?;
    let exec_params = ExecParams {
        command: command.command,
        cwd,
        expiration: ExecExpiration::Timeout(Duration::from_millis(timeout_ms)),
        capture_policy: ExecCapturePolicy::ShellTool,
        env: create_env(
            &turn.shell_environment_policy,
            Some(session.conversation_id),
        ),
        network: turn.network.clone(),
        sandbox_permissions: SandboxPermissions::UseDefault,
        windows_sandbox_level: turn.windows_sandbox_level,
        windows_sandbox_private_desktop: turn.config.permissions.windows_sandbox_private_desktop,
        justification: None,
        arg0: None,
    };
    let tool_name = ToolName::plain(match command.execution {
        peregrine_security_tools::SecurityCommandExecution::BundledSui
        | peregrine_security_tools::SecurityCommandExecution::SystemSui => spec::SUI_COMMAND,
        peregrine_security_tools::SecurityCommandExecution::MovyFuzzHelper => spec::MOVY_FUZZ,
        peregrine_security_tools::SecurityCommandExecution::FormalVerificationHelper => {
            spec::FORMAL_VERIFY
        }
    });

    run_exec_like(RunExecLikeArgs {
        tool_name,
        exec_params,
        cancellation_token,
        hook_command: command.display,
        shell_type: None,
        additional_permissions: None,
        prefix_rule: None,
        session,
        turn,
        tracker,
        call_id,
        shell_runtime_backend: ShellRuntimeBackend::ShellCommandClassic,
    })
    .await
    .map(boxed_tool_output)
}

fn parse_function_args<T>(payload: &ToolPayload, tool_name: &str) -> Result<T, FunctionCallError>
where
    T: for<'de> Deserialize<'de>,
{
    let ToolPayload::Function { arguments } = payload else {
        return Err(FunctionCallError::RespondToModel(format!(
            "{tool_name} handler received unsupported payload"
        )));
    };
    parse_arguments(arguments)
}

fn timeout_seconds_to_ms(seconds: u64) -> u64 {
    seconds.saturating_mul(1000).max(1000)
}

#[derive(Debug, Deserialize)]
struct SuiCommandArgs {
    #[serde(default)]
    project_root: Option<String>,
    #[serde(default)]
    package_path: Option<String>,
    command_kind: String,
    #[serde(default)]
    publish_build_env: Option<String>,
    #[serde(default)]
    with_unpublished_dependencies: Option<bool>,
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct MovyFuzzArgs {
    #[serde(default)]
    project_root: Option<String>,
    #[serde(default)]
    package_path: Option<String>,
    #[serde(default)]
    time_limit_seconds: Option<u64>,
    #[serde(default)]
    seed: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct FormalVerifyArgs {
    #[serde(default)]
    project_root: Option<String>,
    #[serde(default)]
    package_path: Option<String>,
    file_path: String,
    module_name: String,
    #[serde(default)]
    timeout_seconds: Option<usize>,
}
