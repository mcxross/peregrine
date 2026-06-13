use crate::adapter::SecurityCommand;
use peregrine_sui_mcp_protocol::{CommandResult, CommandStatus, MAX_OUTPUT_BYTES, PackageSummary};
use std::{process::Stdio, time::Duration};
use tokio::{io::AsyncReadExt, process::Command, time::timeout};

pub(crate) async fn run(
    command: SecurityCommand,
    package: PackageSummary,
    timeout_ms: u64,
) -> CommandResult {
    let display = command.display.clone();
    let Some((program, args)) = command.command.split_first() else {
        return failure(package, display, "security command was empty");
    };
    let mut process = Command::new(program);
    process
        .args(args)
        .current_dir(&command.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(/*kill_on_drop*/ true);
    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(error) => return failure(package, display, &error.to_string()),
    };
    let Some(mut stdout) = child.stdout.take() else {
        let _ = child.kill().await;
        return failure(package, display, "security command stdout was unavailable");
    };
    let Some(mut stderr) = child.stderr.take() else {
        let _ = child.kill().await;
        return failure(package, display, "security command stderr was unavailable");
    };
    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    let execution = async {
        let (status, stdout_result, stderr_result) = tokio::join!(
            child.wait(),
            stdout.read_to_end(&mut stdout_bytes),
            stderr.read_to_end(&mut stderr_bytes),
        );
        stdout_result?;
        stderr_result?;
        status
    };

    match timeout(Duration::from_millis(timeout_ms.max(1)), execution).await {
        Ok(Ok(status)) => {
            let (stdout, stdout_truncated) = bounded_text(stdout_bytes);
            let (stderr, stderr_truncated) = bounded_text(stderr_bytes);
            CommandResult {
                status: if status.success() {
                    CommandStatus::Completed
                } else {
                    CommandStatus::Failed
                },
                package,
                command: display,
                exit_code: status.code(),
                stdout,
                stderr,
                truncated: stdout_truncated || stderr_truncated,
            }
        }
        Ok(Err(error)) => failure(package, display, &error.to_string()),
        Err(_) => {
            let _ = child.kill().await;
            CommandResult {
                status: CommandStatus::TimedOut,
                package,
                command: display,
                exit_code: None,
                stdout: String::new(),
                stderr: format!("command timed out after {timeout_ms}ms"),
                truncated: false,
            }
        }
    }
}

fn failure(package: PackageSummary, command: String, error: &str) -> CommandResult {
    CommandResult {
        status: CommandStatus::Failed,
        package,
        command,
        exit_code: None,
        stdout: String::new(),
        stderr: error.to_string(),
        truncated: false,
    }
}

fn bounded_text(bytes: Vec<u8>) -> (String, bool) {
    let truncated = bytes.len() > MAX_OUTPUT_BYTES;
    let end = bytes.len().min(MAX_OUTPUT_BYTES);
    (
        String::from_utf8_lossy(&bytes[..end]).into_owned(),
        truncated,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use crate::adapter::SecurityCommandExecution;
    #[cfg(unix)]
    use peregrine_sui_mcp_protocol::CommandStatus;
    #[cfg(unix)]
    use std::{fs, process::Command as StdCommand, thread, time::Instant};

    #[test]
    fn output_is_bounded() {
        let (text, truncated) = bounded_text(vec![b'x'; MAX_OUTPUT_BYTES + 1]);

        assert_eq!(text.len(), MAX_OUTPUT_BYTES);
        assert!(truncated);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timed_out_command_is_killed() {
        let temp = tempfile::tempdir().expect("temp dir");
        let pid_path = temp.path().join("command.pid");
        let command = SecurityCommand {
            command: vec![
                "/bin/sh".to_string(),
                "-c".to_string(),
                format!("echo $$ > {}; exec sleep 60", pid_path.display()),
            ],
            cwd: temp.path().to_path_buf(),
            display: "sleep 60".to_string(),
            execution: SecurityCommandExecution::SystemSui,
        };
        let package = PackageSummary {
            project_root: temp.path().display().to_string(),
            package_root: temp.path().display().to_string(),
            package_path: ".".to_string(),
            package_name: "test".to_string(),
        };

        let result = run(command, package, 500).await;

        assert_eq!(result.status, CommandStatus::TimedOut);
        let pid = fs::read_to_string(pid_path)
            .expect("pid file")
            .trim()
            .to_string();
        let deadline = Instant::now() + Duration::from_secs(2);
        while process_is_running(&pid) && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(10));
        }
        assert!(
            !process_is_running(&pid),
            "timed-out process {pid} survived"
        );
    }

    #[cfg(unix)]
    fn process_is_running(pid: &str) -> bool {
        StdCommand::new("ps")
            .args(["-p", pid, "-o", "pid="])
            .output()
            .is_ok_and(|output| !output.stdout.is_empty())
    }
}
