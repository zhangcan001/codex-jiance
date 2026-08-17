use std::{path::Path, process::Stdio, time::Duration};

use tokio::{process::Command, time::timeout};

use crate::error::AppError;

pub const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessOutput {
    pub success: bool,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

pub async fn run_command(executable: &Path, args: &[&str]) -> Result<ProcessOutput, AppError> {
    run_command_with_timeout(executable, args, COMMAND_TIMEOUT).await
}

pub async fn run_command_with_timeout(
    executable: &Path,
    args: &[&str],
    command_timeout: Duration,
) -> Result<ProcessOutput, AppError> {
    let mut command = if is_windows_script(executable) {
        let mut command = Command::new("cmd.exe");
        let command_line = format!("\"{}\"", build_cmd_line(executable, args));
        #[cfg(windows)]
        command.raw_arg(format!("/D /S /C {command_line}"));
        #[cfg(not(windows))]
        command.args(["/D", "/S", "/C", &command_line]);
        command
    } else {
        let mut command = Command::new(executable);
        command.args(args);
        command
    };

    command
        .kill_on_drop(true)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = timeout(command_timeout, command.output())
        .await
        .map_err(|_| {
            AppError::ProcessTimeout(format!(
                "Codex command timed out after {} seconds.",
                command_timeout.as_secs()
            ))
        })?
        .map_err(|error| {
            AppError::Process(format!("Failed to run '{}': {error}", executable.display()))
        })?;

    Ok(ProcessOutput {
        success: output.status.success(),
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    })
}

pub(crate) fn is_windows_script(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| matches!(extension.to_ascii_lowercase().as_str(), "cmd" | "bat"))
}

pub(crate) fn build_cmd_line(executable: &Path, args: &[&str]) -> String {
    let mut command_line = vec![quote_cmd_arg(&executable.to_string_lossy())];
    command_line.extend(args.iter().map(|arg| quote_cmd_arg(arg)));
    command_line.join(" ")
}

fn quote_cmd_arg(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\\\""))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{build_cmd_line, is_windows_script};

    #[test]
    fn detects_windows_script_extensions_case_insensitively() {
        assert!(is_windows_script(Path::new("codex.cmd")));
        assert!(is_windows_script(Path::new("codex.BAT")));
        assert!(!is_windows_script(Path::new("codex.exe")));
    }

    #[test]
    fn quotes_script_paths_with_spaces() {
        assert_eq!(
            build_cmd_line(Path::new(r"C:\Program Files\npm\codex.cmd"), &["--version"]),
            r#""C:\Program Files\npm\codex.cmd" "--version""#
        );
    }
}
