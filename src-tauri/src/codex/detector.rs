use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{
    codex::process::{run_command, ProcessOutput},
    error::AppError,
    models::codex::CodexInstallationInfo,
};

#[derive(Debug, Clone, PartialEq, Eq)]
struct Candidate {
    path: PathBuf,
    source: &'static str,
}

pub async fn detect() -> Result<CodexInstallationInfo, AppError> {
    let candidates = discover_candidates()?;

    if candidates.is_empty() {
        log::info!("Codex CLI was not found on PATH or in the Windows npm fallback path");
        let mut result = info(false, "notFound");
        result.message = Some("Codex CLI was not found.".to_owned());
        return Ok(result);
    }

    let mut last_failure = None;
    let mut last_candidate = None;

    for candidate in candidates {
        last_candidate = Some(candidate.clone());
        match run_command(&candidate.path, &["--version"]).await {
            Ok(output) if output.success && !display_output(&output).is_empty() => {
                return detect_app_server(candidate, output).await;
            }
            Ok(output) => {
                last_failure = Some(format_command_failure("codex --version", &output));
            }
            Err(error @ AppError::ProcessTimeout(_)) => return Err(error),
            Err(error) => {
                last_failure = Some(error.to_string());
            }
        }
    }

    let Some(candidate) = last_candidate else {
        let mut result = info(false, "notFound");
        result.message = Some("Codex CLI was not found.".to_owned());
        return Ok(result);
    };

    let mut result = info(false, "versionError");
    result.executable_path = Some(candidate.path.to_string_lossy().into_owned());
    result.detection_source = Some(candidate.source.to_owned());
    result.message =
        last_failure.or_else(|| Some("Codex CLI version detection failed.".to_owned()));
    Ok(result)
}

async fn detect_app_server(
    candidate: Candidate,
    version_output: ProcessOutput,
) -> Result<CodexInstallationInfo, AppError> {
    let version_raw = display_output(&version_output).trim().to_owned();
    let version = parse_version(&version_raw);

    let app_server = run_command(&candidate.path, &["app-server", "--help"]).await;
    let (app_server_supported, app_server_status, app_server_message) = match app_server {
        Ok(output) if output.success => (true, "ready", None),
        Ok(output) => (
            false,
            "appServerUnavailable",
            Some(format_command_failure("codex app-server --help", &output)),
        ),
        Err(AppError::ProcessTimeout(error)) => return Err(AppError::ProcessTimeout(error)),
        Err(error) => (false, "appServerUnavailable", Some(error.to_string())),
    };
    let (status, message) = if version.is_none() {
        (
            "versionError",
            Some(match app_server_message {
                Some(message) => format!("Codex CLI version output was not recognized. {message}"),
                None => "Codex CLI version output was not recognized.".to_owned(),
            }),
        )
    } else {
        (app_server_status, app_server_message)
    };

    log::info!(
        "Codex CLI detected at {} (version: {}, app server: {})",
        candidate.path.display(),
        version.as_deref().unwrap_or("unparsed"),
        app_server_supported
    );

    let mut result = info(true, status);
    result.executable_path = Some(candidate.path.to_string_lossy().into_owned());
    result.version = version;
    result.version_raw = Some(version_raw);
    result.app_server_supported = app_server_supported;
    result.detection_source = Some(candidate.source.to_owned());
    result.message = message;
    Ok(result)
}

fn discover_candidates() -> Result<Vec<Candidate>, AppError> {
    let mut candidates = Vec::new();

    match which::which_all("codex") {
        Ok(paths) => {
            for path in paths {
                push_candidate(&mut candidates, path, "path");
            }
        }
        Err(which::Error::CannotFindBinaryPath) => {}
        Err(error) => {
            return Err(AppError::CodexDetection(format!(
                "PATH inspection failed: {error}"
            )))
        }
    }

    if let Some(app_data) = std::env::var_os("APPDATA") {
        let npm_dir = PathBuf::from(app_data).join("npm");
        for file_name in ["codex.cmd", "codex.exe"] {
            let path = npm_dir.join(file_name);
            if path.is_file() {
                push_candidate(&mut candidates, path, "windowsNpm");
            }
        }
    }

    Ok(candidates)
}

fn push_candidate(candidates: &mut Vec<Candidate>, path: PathBuf, source: &'static str) {
    let key = path_key(&path);
    if candidates
        .iter()
        .all(|candidate| path_key(&candidate.path) != key)
    {
        candidates.push(Candidate { path, source });
    }
}

fn path_key(path: &Path) -> String {
    path.to_string_lossy().to_ascii_lowercase()
}

fn info(installed: bool, status: &str) -> CodexInstallationInfo {
    CodexInstallationInfo {
        installed,
        status: status.to_owned(),
        executable_path: None,
        version: None,
        version_raw: None,
        app_server_supported: false,
        detection_source: None,
        detected_at: unix_timestamp(),
        message: None,
    }
}

fn display_output(output: &ProcessOutput) -> String {
    let stdout = output.stdout.trim();
    if !stdout.is_empty() {
        return stdout.to_owned();
    }

    output.stderr.trim().to_owned()
}

fn format_command_failure(command: &str, output: &ProcessOutput) -> String {
    let detail = display_output(output);
    if detail.is_empty() {
        format!("'{command}' exited with code {:?}.", output.exit_code)
    } else {
        format!(
            "'{command}' exited with code {:?}: {detail}",
            output.exit_code
        )
    }
}

pub(crate) fn parse_version(raw: &str) -> Option<String> {
    raw.split_whitespace().find_map(|token| {
        let token = token.trim_matches(|character: char| {
            !character.is_ascii_alphanumeric() && !matches!(character, '.' | '-' | '+')
        });
        let token = token.strip_prefix('v').unwrap_or(token);
        let core = token.split(['-', '+']).next()?;
        let core_segments: Vec<&str> = core.split('.').collect();
        if core_segments.len() < 2
            || core_segments.iter().any(|segment| {
                segment.is_empty() || !segment.chars().all(|character| character.is_ascii_digit())
            })
        {
            return None;
        }
        if token.chars().any(|character| {
            !character.is_ascii_alphanumeric() && !matches!(character, '.' | '-' | '+')
        }) {
            return None;
        }
        Some(token.to_owned())
    })
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{parse_version, push_candidate};

    #[test]
    fn parses_codex_cli_version_output() {
        assert_eq!(
            parse_version("codex-cli 0.147.0-alpha.6.6"),
            Some("0.147.0-alpha.6.6".to_owned())
        );
    }

    #[test]
    fn parses_codex_version_output() {
        assert_eq!(parse_version("codex 1.2.3"), Some("1.2.3".to_owned()));
    }

    #[test]
    fn leaves_malformed_version_unparsed() {
        assert_eq!(parse_version("Codex Development Build"), None);
        assert_eq!(parse_version("codex 1.x.3"), None);
    }

    #[test]
    fn deduplicates_candidate_paths_without_case_sensitivity() {
        let mut candidates = Vec::new();
        push_candidate(
            &mut candidates,
            PathBuf::from(r"C:\Users\Admin\AppData\Roaming\npm\codex.cmd"),
            "path",
        );
        push_candidate(
            &mut candidates,
            PathBuf::from(r"c:\users\admin\appdata\roaming\npm\CODEX.CMD"),
            "windowsNpm",
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].source, "path");
    }
}
