use std::path::{Path, PathBuf};

use peregrine_app_server_protocol::{AuditProfileParams, AuditReportFormat, AuditTargetParams};
use peregrine_types::harness::AuditProfile;

use super::{AUDIT_USAGE, AuditCommand, AuditLifecycleAction, AuditTargetRequest};

const DEFAULT_CHAIN_ID: &str = "sui";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TargetCommandKind {
    Plan,
    Run,
}

struct TargetParser {
    chain_id: String,
    local_path: Option<String>,
    remote: bool,
    network_id: Option<String>,
    package_ref: Option<String>,
    source_uri: Option<String>,
    state_ref: Option<String>,
    profile: AuditProfileParams,
    profile_overridden: bool,
    positionals: Vec<String>,
}

impl TargetParser {
    fn new() -> Self {
        let profile = AuditProfile::default();
        Self {
            chain_id: DEFAULT_CHAIN_ID.to_string(),
            local_path: None,
            remote: false,
            network_id: None,
            package_ref: None,
            source_uri: None,
            state_ref: None,
            profile: AuditProfileParams {
                model_token_budget: profile.model_token_budget,
                wall_time_seconds: profile.wall_time_seconds,
                max_hypotheses: profile.max_hypotheses,
                max_dependency_depth: profile.max_dependency_depth,
                max_dependency_packages: profile.max_dependency_packages,
            },
            profile_overridden: false,
            positionals: Vec::new(),
        }
    }

    fn into_request(mut self, cwd: &Path) -> std::result::Result<AuditTargetRequest, String> {
        if self.remote || self.network_id.is_some() || self.package_ref.is_some() {
            self.remote = true;
        }

        let target = if self.remote {
            if !self.positionals.is_empty() {
                return Err(
                    "remote audit targets require --network and --package flags".to_string()
                );
            }
            AuditTargetParams::RemotePackage {
                chain_id: self.chain_id,
                network_id: self
                    .network_id
                    .ok_or_else(|| "remote audit target is missing --network".to_string())?,
                package_ref: self
                    .package_ref
                    .ok_or_else(|| "remote audit target is missing --package".to_string())?,
                source_uri: self.source_uri,
                state_ref: self.state_ref,
                metadata: None,
            }
        } else {
            if self.local_path.is_none() && self.positionals.len() == 1 {
                self.local_path = self.positionals.pop();
            }
            if !self.positionals.is_empty() {
                return Err("local audit targets accept exactly one path".to_string());
            }
            let path = self
                .local_path
                .ok_or_else(|| "local audit target is missing a path".to_string())?;
            AuditTargetParams::LocalPackage {
                chain_id: self.chain_id,
                path: absolute_path_string(cwd, &path),
                metadata: None,
            }
        };

        Ok(AuditTargetRequest {
            target,
            profile: self.profile_overridden.then_some(self.profile),
        })
    }
}

pub(crate) fn parse_audit_command(
    args: &str,
    cwd: &Path,
) -> std::result::Result<AuditCommand, String> {
    let Some(mut tokens) = shlex::split(args) else {
        return Err("could not parse /audit arguments; check quoting".to_string());
    };
    if tokens.is_empty() {
        return Err(AUDIT_USAGE.to_string());
    }

    let head = tokens[0].to_ascii_lowercase();
    match head.as_str() {
        "plan" | "preflight" => {
            tokens.remove(0);
            return parse_target_command(TargetCommandKind::Plan, &tokens, cwd);
        }
        "--plan" => {
            tokens.remove(0);
            return parse_target_command(TargetCommandKind::Plan, &tokens, cwd);
        }
        "run" => {
            tokens.remove(0);
            return parse_target_command(TargetCommandKind::Run, &tokens, cwd);
        }
        "start" => {
            return parse_single_arg_command(&tokens, "fingerprint", |fingerprint| {
                AuditCommand::Start { fingerprint }
            });
        }
        "read" | "status" => {
            return parse_single_arg_command(&tokens, "auditId", |audit_id| AuditCommand::Read {
                audit_id,
            });
        }
        "report" => {
            return parse_report_command(&tokens);
        }
        "artifact" => {
            return parse_artifact_command(&tokens);
        }
        "list" => {
            return parse_list_command(&tokens);
        }
        "pause" => {
            return parse_lifecycle_command(&tokens, AuditLifecycleAction::Pause);
        }
        "resume" => {
            return parse_lifecycle_command(&tokens, AuditLifecycleAction::Resume);
        }
        "cancel" => {
            return parse_lifecycle_command(&tokens, AuditLifecycleAction::Cancel);
        }
        "delete" => {
            return parse_lifecycle_command(&tokens, AuditLifecycleAction::Delete);
        }
        _ => {}
    }

    parse_target_command(TargetCommandKind::Run, &tokens, cwd)
}

fn parse_target_command(
    kind: TargetCommandKind,
    tokens: &[String],
    cwd: &Path,
) -> std::result::Result<AuditCommand, String> {
    if tokens.is_empty() {
        return Err(AUDIT_USAGE.to_string());
    }

    let mut parser = TargetParser::new();
    let mut index = 0;
    while index < tokens.len() {
        let token = &tokens[index];
        if token == "local" && parser.positionals.is_empty() && parser.local_path.is_none() {
            index += 1;
            continue;
        }
        if token == "remote" {
            parser.remote = true;
            index += 1;
            continue;
        }
        if let Some((flag, value)) = token.split_once('=') {
            apply_flag_value(&mut parser, flag, value)?;
            index += 1;
            continue;
        }
        match token.as_str() {
            "--remote" => parser.remote = true,
            "--local" => parser.remote = false,
            "--chain" => parser.chain_id = take_value(tokens, &mut index, "--chain")?,
            "--path" => parser.local_path = Some(take_value(tokens, &mut index, "--path")?),
            "--network" => parser.network_id = Some(take_value(tokens, &mut index, "--network")?),
            "--package" => parser.package_ref = Some(take_value(tokens, &mut index, "--package")?),
            "--source-uri" => {
                parser.source_uri = Some(take_value(tokens, &mut index, "--source-uri")?);
            }
            "--state-ref" => {
                parser.state_ref = Some(take_value(tokens, &mut index, "--state-ref")?);
            }
            "--tokens" => {
                parser.profile.model_token_budget =
                    parse_i64(&take_value(tokens, &mut index, "--tokens")?, "--tokens")?;
                parser.profile_overridden = true;
            }
            "--seconds" => {
                parser.profile.wall_time_seconds =
                    parse_i64(&take_value(tokens, &mut index, "--seconds")?, "--seconds")?;
                parser.profile_overridden = true;
            }
            "--hours" => {
                let hours = parse_i64(&take_value(tokens, &mut index, "--hours")?, "--hours")?;
                parser.profile.wall_time_seconds = hours
                    .checked_mul(60 * 60)
                    .ok_or_else(|| "--hours is too large".to_string())?;
                parser.profile_overridden = true;
            }
            "--hypotheses" | "--max-hypotheses" => {
                parser.profile.max_hypotheses =
                    parse_u32(&take_value(tokens, &mut index, token)?, token)?;
                parser.profile_overridden = true;
            }
            "--max-dependency-depth" => {
                parser.profile.max_dependency_depth =
                    parse_u32(&take_value(tokens, &mut index, token)?, token)?;
                parser.profile_overridden = true;
            }
            "--max-dependency-packages" => {
                parser.profile.max_dependency_packages =
                    parse_u32(&take_value(tokens, &mut index, token)?, token)?;
                parser.profile_overridden = true;
            }
            _ if token.starts_with('-') => return Err(format!("unknown /audit flag: {token}")),
            _ => parser.positionals.push(token.clone()),
        }
        index += 1;
    }

    let request = parser.into_request(cwd)?;
    Ok(match kind {
        TargetCommandKind::Plan => AuditCommand::Plan(request),
        TargetCommandKind::Run => AuditCommand::Run(request),
    })
}

fn apply_flag_value(
    parser: &mut TargetParser,
    flag: &str,
    value: &str,
) -> std::result::Result<(), String> {
    match flag {
        "--chain" => parser.chain_id = value.to_string(),
        "--path" => parser.local_path = Some(value.to_string()),
        "--network" => parser.network_id = Some(value.to_string()),
        "--package" => parser.package_ref = Some(value.to_string()),
        "--source-uri" => parser.source_uri = Some(value.to_string()),
        "--state-ref" => parser.state_ref = Some(value.to_string()),
        "--tokens" => {
            parser.profile.model_token_budget = parse_i64(value, flag)?;
            parser.profile_overridden = true;
        }
        "--seconds" => {
            parser.profile.wall_time_seconds = parse_i64(value, flag)?;
            parser.profile_overridden = true;
        }
        "--hours" => {
            parser.profile.wall_time_seconds = parse_i64(value, flag)?
                .checked_mul(60 * 60)
                .ok_or_else(|| "--hours is too large".to_string())?;
            parser.profile_overridden = true;
        }
        "--hypotheses" | "--max-hypotheses" => {
            parser.profile.max_hypotheses = parse_u32(value, flag)?;
            parser.profile_overridden = true;
        }
        "--max-dependency-depth" => {
            parser.profile.max_dependency_depth = parse_u32(value, flag)?;
            parser.profile_overridden = true;
        }
        "--max-dependency-packages" => {
            parser.profile.max_dependency_packages = parse_u32(value, flag)?;
            parser.profile_overridden = true;
        }
        _ => return Err(format!("unknown /audit flag: {flag}")),
    }
    Ok(())
}

fn parse_lifecycle_command(
    tokens: &[String],
    action: AuditLifecycleAction,
) -> std::result::Result<AuditCommand, String> {
    parse_single_arg_command(tokens, "auditId", |audit_id| AuditCommand::Lifecycle {
        action,
        audit_id,
    })
}

fn parse_report_command(tokens: &[String]) -> std::result::Result<AuditCommand, String> {
    if tokens.len() < 2 {
        return Err("/audit report requires one auditId".to_string());
    }
    let audit_id = tokens[1].clone();
    let mut format = AuditReportFormat::Markdown;
    for token in &tokens[2..] {
        match token.as_str() {
            "--json" | "json" => format = AuditReportFormat::Json,
            "--markdown" | "--md" | "markdown" | "md" => format = AuditReportFormat::Markdown,
            _ => return Err(format!("unknown /audit report flag: {token}")),
        }
    }
    Ok(AuditCommand::Report { audit_id, format })
}

fn parse_artifact_command(tokens: &[String]) -> std::result::Result<AuditCommand, String> {
    if tokens.len() != 3 {
        return Err("/audit artifact requires auditId and artifact ref".to_string());
    }
    Ok(AuditCommand::Artifact {
        audit_id: tokens[1].clone(),
        artifact_ref: tokens[2].clone(),
    })
}

fn parse_list_command(tokens: &[String]) -> std::result::Result<AuditCommand, String> {
    let mut cursor = None;
    let mut limit = None;
    let mut index = 1;
    while index < tokens.len() {
        let token = &tokens[index];
        if let Some((flag, value)) = token.split_once('=') {
            match flag {
                "--cursor" => cursor = Some(value.to_string()),
                "--limit" => limit = Some(parse_u32(value, flag)?),
                _ => return Err(format!("unknown /audit list flag: {flag}")),
            }
            index += 1;
            continue;
        }
        match token.as_str() {
            "--cursor" => cursor = Some(take_value(tokens, &mut index, "--cursor")?),
            "--limit" => {
                limit = Some(parse_u32(
                    &take_value(tokens, &mut index, "--limit")?,
                    "--limit",
                )?)
            }
            _ => return Err(format!("unknown /audit list flag: {token}")),
        }
        index += 1;
    }
    Ok(AuditCommand::List { cursor, limit })
}

fn parse_single_arg_command<F>(
    tokens: &[String],
    arg_name: &str,
    build: F,
) -> std::result::Result<AuditCommand, String>
where
    F: FnOnce(String) -> AuditCommand,
{
    if tokens.len() != 2 {
        return Err(format!("/audit {} requires one {arg_name}", tokens[0]));
    }
    Ok(build(tokens[1].clone()))
}

fn take_value(
    tokens: &[String],
    index: &mut usize,
    flag: &str,
) -> std::result::Result<String, String> {
    *index += 1;
    tokens
        .get(*index)
        .filter(|value| !value.starts_with('-'))
        .cloned()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn parse_i64(value: &str, flag: &str) -> std::result::Result<i64, String> {
    value
        .parse::<i64>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{flag} must be a positive integer"))
}

fn parse_u32(value: &str, flag: &str) -> std::result::Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|value| *value > 0)
        .ok_or_else(|| format!("{flag} must be a positive integer"))
}

fn absolute_path_string(cwd: &Path, value: &str) -> String {
    let path = PathBuf::from(value);
    let absolute = if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    };
    absolute.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_local_plan_target() {
        let cwd = Path::new("/tmp/work");
        let command = parse_audit_command("--plan ./pkg --chain sui --tokens 42", cwd)
            .expect("parse audit command");

        let AuditCommand::Plan(request) = command else {
            panic!("expected plan command");
        };
        assert_eq!(
            request.target,
            AuditTargetParams::LocalPackage {
                chain_id: "sui".to_string(),
                path: "/tmp/work/./pkg".to_string(),
                metadata: None,
            }
        );
        assert_eq!(
            request.profile,
            Some(AuditProfileParams {
                model_token_budget: 42,
                wall_time_seconds: 14_400,
                max_hypotheses: 500,
                max_dependency_depth: 3,
                max_dependency_packages: 64,
            })
        );
    }

    #[test]
    fn parses_remote_run_target() {
        let command = parse_audit_command(
            "run --remote --network mainnet --package 0x2 --source-uri https://example.invalid/graphql",
            Path::new("/tmp/work"),
        )
        .expect("parse audit command");

        let AuditCommand::Run(request) = command else {
            panic!("expected run command");
        };
        assert_eq!(
            request.target,
            AuditTargetParams::RemotePackage {
                chain_id: "sui".to_string(),
                network_id: "mainnet".to_string(),
                package_ref: "0x2".to_string(),
                source_uri: Some("https://example.invalid/graphql".to_string()),
                state_ref: None,
                metadata: None,
            }
        );
        assert_eq!(request.profile, None);
    }

    #[test]
    fn parses_lifecycle_commands() {
        assert_eq!(
            parse_audit_command("start abc123", Path::new("/tmp")).expect("parse start"),
            AuditCommand::Start {
                fingerprint: "abc123".to_string(),
            }
        );
        assert_eq!(
            parse_audit_command("pause audit-1", Path::new("/tmp")).expect("parse pause"),
            AuditCommand::Lifecycle {
                action: AuditLifecycleAction::Pause,
                audit_id: "audit-1".to_string(),
            }
        );
        assert_eq!(
            parse_audit_command("list", Path::new("/tmp")).expect("parse list"),
            AuditCommand::List {
                cursor: None,
                limit: None,
            }
        );
        assert_eq!(
            parse_audit_command("list --cursor 25 --limit=10", Path::new("/tmp"))
                .expect("parse paged list"),
            AuditCommand::List {
                cursor: Some("25".to_string()),
                limit: Some(10),
            }
        );
    }

    #[test]
    fn parses_report_and_artifact_commands() {
        assert_eq!(
            parse_audit_command("report audit-1 --json", Path::new("/tmp")).expect("parse report"),
            AuditCommand::Report {
                audit_id: "audit-1".to_string(),
                format: AuditReportFormat::Json,
            }
        );
        assert_eq!(
            parse_audit_command("artifact audit-1 artifacts/example.json", Path::new("/tmp"))
                .expect("parse artifact"),
            AuditCommand::Artifact {
                audit_id: "audit-1".to_string(),
                artifact_ref: "artifacts/example.json".to_string(),
            }
        );
    }
}
