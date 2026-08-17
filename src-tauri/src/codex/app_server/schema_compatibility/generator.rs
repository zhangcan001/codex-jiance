use std::{path::Path, time::Duration};

use serde_json::Value;
use tempfile::tempdir;
use tokio::task;

use crate::{
    codex::process::{run_command_with_timeout, ProcessOutput},
    error::AppError,
};

pub(crate) const SCHEMA_GENERATION_TIMEOUT: Duration = Duration::from_secs(15);
pub(crate) const MAX_SCHEMA_FILES: usize = 512;
pub(crate) const MAX_TOTAL_SCHEMA_BYTES: u64 = 64 * 1024 * 1024;
pub(crate) const MAX_SINGLE_SCHEMA_BYTES: u64 = 16 * 1024 * 1024;
const MAX_SCHEMA_DEPTH: usize = 8;

#[derive(Debug)]
pub(crate) struct GeneratedSchemaData {
    pub(crate) json_documents: Vec<Value>,
    pub(crate) file_count: usize,
    pub(crate) total_bytes: u64,
}

pub(crate) fn build_schema_generation_args(output_dir: &Path) -> Vec<String> {
    vec![
        "app-server".to_owned(),
        "generate-json-schema".to_owned(),
        "--out".to_owned(),
        output_dir.to_string_lossy().into_owned(),
    ]
}

pub(crate) async fn generate_stable_schema(
    executable: &Path,
) -> Result<GeneratedSchemaData, AppError> {
    log::info!("Codex App Server schema compatibility check started");

    let temporary_directory = tempdir().map_err(|error| {
        AppError::SchemaCompatibility(format!(
            "Could not create a temporary schema directory: {error}"
        ))
    })?;
    let output_directory = temporary_directory.path().to_owned();
    let arguments = build_schema_generation_args(&output_directory);
    let argument_references = arguments.iter().map(String::as_str).collect::<Vec<_>>();

    let output =
        run_command_with_timeout(executable, &argument_references, SCHEMA_GENERATION_TIMEOUT)
            .await?;

    if !output.success {
        let detail = concise_process_detail(&output);
        if schema_generation_command_is_unavailable(&detail) {
            return Err(AppError::SchemaGenerationUnavailable(detail));
        }

        return Err(AppError::SchemaCompatibility(format!(
            "Schema generation command failed: {detail}"
        )));
    }

    let parsed = task::spawn_blocking(move || read_schema_files(&output_directory))
        .await
        .map_err(|error| {
            AppError::SchemaCompatibility(format!("Schema parsing task failed: {error}"))
        })??;

    log::info!(
        "Stable App Server schema generated: files={}, bytes={}",
        parsed.file_count,
        parsed.total_bytes
    );

    Ok(parsed)
}

fn read_schema_files(root: &Path) -> Result<GeneratedSchemaData, AppError> {
    let mut json_documents = Vec::new();
    let mut total_bytes = 0;
    walk_schema_directory(root, 0, &mut json_documents, &mut total_bytes)?;

    if json_documents.is_empty() {
        return Err(AppError::SchemaCompatibility(
            "No JSON Schema files were generated.".to_owned(),
        ));
    }

    Ok(GeneratedSchemaData {
        file_count: json_documents.len(),
        json_documents,
        total_bytes,
    })
}

fn walk_schema_directory(
    directory: &Path,
    depth: usize,
    json_documents: &mut Vec<Value>,
    total_bytes: &mut u64,
) -> Result<(), AppError> {
    if depth > MAX_SCHEMA_DEPTH {
        return Err(AppError::SchemaCompatibility(format!(
            "Schema directory depth exceeds the limit of {MAX_SCHEMA_DEPTH}."
        )));
    }

    let entries = std::fs::read_dir(directory).map_err(|error| {
        AppError::SchemaCompatibility(format!("Schema directory could not be read: {error}"))
    })?;

    for entry in entries {
        let entry = entry.map_err(|error| {
            AppError::SchemaCompatibility(format!(
                "Schema directory entry could not be read: {error}"
            ))
        })?;
        let file_type = entry.file_type().map_err(|error| {
            AppError::SchemaCompatibility(format!("Schema file type could not be read: {error}"))
        })?;
        let path = entry.path();

        if file_type.is_dir() {
            walk_schema_directory(&path, depth + 1, json_documents, total_bytes)?;
            continue;
        }

        if !file_type.is_file()
            || !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            continue;
        }

        validate_file_count(json_documents.len() + 1)?;

        let file_size = std::fs::metadata(&path)
            .map_err(|error| {
                AppError::SchemaCompatibility(format!(
                    "Schema file metadata could not be read: {error}"
                ))
            })?
            .len();
        validate_single_file_size(file_size)?;

        let bytes = std::fs::read(&path).map_err(|error| {
            AppError::SchemaCompatibility(format!("Schema file could not be read: {error}"))
        })?;
        validate_single_file_size(bytes.len() as u64)?;
        let next_total = total_bytes.checked_add(bytes.len() as u64).ok_or_else(|| {
            AppError::SchemaCompatibility("Schema byte count overflowed.".to_owned())
        })?;
        validate_total_size(next_total)?;

        let document = serde_json::from_slice::<Value>(&bytes).map_err(|error| {
            AppError::SchemaCompatibility(format!("Malformed JSON Schema file: {error}"))
        })?;

        *total_bytes = next_total;
        json_documents.push(document);
    }

    Ok(())
}

fn validate_single_file_size(file_size: u64) -> Result<(), AppError> {
    if file_size > MAX_SINGLE_SCHEMA_BYTES {
        return Err(AppError::SchemaCompatibility(format!(
            "A JSON Schema file exceeds the limit of {MAX_SINGLE_SCHEMA_BYTES} bytes."
        )));
    }

    Ok(())
}

fn validate_file_count(file_count: usize) -> Result<(), AppError> {
    if file_count > MAX_SCHEMA_FILES {
        return Err(AppError::SchemaCompatibility(format!(
            "Schema file count exceeds the limit of {MAX_SCHEMA_FILES}."
        )));
    }

    Ok(())
}

fn validate_total_size(total_bytes: u64) -> Result<(), AppError> {
    if total_bytes > MAX_TOTAL_SCHEMA_BYTES {
        return Err(AppError::SchemaCompatibility(format!(
            "Generated JSON Schema exceeds the total limit of {MAX_TOTAL_SCHEMA_BYTES} bytes."
        )));
    }

    Ok(())
}

fn concise_process_detail(output: &ProcessOutput) -> String {
    let detail = if output.stdout.trim().is_empty() {
        output.stderr.trim()
    } else {
        output.stdout.trim()
    };

    let detail = detail.chars().take(512).collect::<String>();
    if detail.is_empty() {
        format!("process exited with code {:?}", output.exit_code)
    } else {
        detail
    }
}

fn schema_generation_command_is_unavailable(detail: &str) -> bool {
    let detail = detail.to_ascii_lowercase();
    [
        "unknown command",
        "unrecognized command",
        "not a valid command",
        "no such command",
        "unexpected argument",
        "found argument",
    ]
    .iter()
    .any(|marker| detail.contains(marker))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::Path};

    use tempfile::tempdir;

    use super::{
        build_schema_generation_args, read_schema_files, validate_file_count,
        validate_single_file_size, validate_total_size, MAX_SCHEMA_FILES, MAX_SINGLE_SCHEMA_BYTES,
        MAX_TOTAL_SCHEMA_BYTES,
    };
    use crate::error::AppError;

    #[test]
    fn uses_stable_schema_generation_arguments_without_experimental_flag() {
        let arguments = build_schema_generation_args(Path::new(r"C:\temp\schema"));

        assert_eq!(arguments[0], "app-server");
        assert_eq!(arguments[1], "generate-json-schema");
        assert_eq!(arguments[2], "--out");
        assert!(arguments[3].contains("schema"));
        assert!(!arguments
            .iter()
            .any(|argument| argument == "--experimental"));
    }

    #[test]
    fn reads_nested_json_files_and_ignores_other_extensions() {
        let directory = tempdir().expect("temporary directory");
        let nested = directory.path().join("nested");
        fs::create_dir(&nested).expect("nested directory");
        fs::write(directory.path().join("one.json"), br#"{"one":1}"#).expect("json file");
        fs::write(nested.join("two.JSON"), br#"{"two":2}"#).expect("json file");
        fs::write(nested.join("ignored.txt"), b"not json").expect("text file");

        let result = read_schema_files(directory.path()).expect("schema files");

        assert_eq!(result.file_count, 2);
        assert_eq!(result.json_documents.len(), 2);
        assert_eq!(result.total_bytes, 18);
    }

    #[test]
    fn malformed_json_returns_an_error() {
        let directory = tempdir().expect("temporary directory");
        fs::write(directory.path().join("broken.json"), b"{").expect("json file");

        let error = read_schema_files(directory.path()).expect_err("malformed JSON should fail");

        assert!(
            matches!(error, AppError::SchemaCompatibility(message) if message.contains("Malformed JSON"))
        );
    }

    #[test]
    fn empty_schema_directory_returns_clear_error() {
        let directory = tempdir().expect("temporary directory");

        let error = read_schema_files(directory.path()).expect_err("empty output should fail");

        assert!(
            matches!(error, AppError::SchemaCompatibility(message) if message == "No JSON Schema files were generated.")
        );
    }

    #[test]
    fn enforces_single_file_and_total_size_limits() {
        let file_count_error =
            validate_file_count(MAX_SCHEMA_FILES + 1).expect_err("file count limit should fail");
        let single_file_error = validate_single_file_size(MAX_SINGLE_SCHEMA_BYTES + 1)
            .expect_err("single file limit should fail");
        let total_error = validate_total_size(MAX_TOTAL_SCHEMA_BYTES + 1)
            .expect_err("total size limit should fail");

        assert!(matches!(file_count_error, AppError::SchemaCompatibility(_)));
        assert!(matches!(
            single_file_error,
            AppError::SchemaCompatibility(_)
        ));
        assert!(matches!(total_error, AppError::SchemaCompatibility(_)));
    }
}
