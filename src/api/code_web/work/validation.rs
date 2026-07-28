use std::time::{SystemTime, UNIX_EPOCH};

use a3s_boot::{BootError, Result as BootResult};

const MAX_ID_LENGTH: usize = 128;
const MAX_NAME_LENGTH: usize = 255;

pub(super) fn validate_id(id: &str, label: &str) -> BootResult<()> {
    if id.is_empty()
        || id.len() > MAX_ID_LENGTH
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(BootError::BadRequest(format!(
            "{label} id must contain only ASCII letters, numbers, hyphens, or underscores"
        )));
    }
    Ok(())
}

pub(super) fn validate_name(value: &str, label: &str) -> BootResult<()> {
    let value = value.trim();
    if value.is_empty() {
        return Err(BootError::BadRequest(format!("{label} cannot be empty")));
    }
    if value.chars().count() > MAX_NAME_LENGTH {
        return Err(BootError::BadRequest(format!(
            "{label} cannot exceed {MAX_NAME_LENGTH} characters"
        )));
    }
    if value.chars().any(char::is_control) {
        return Err(BootError::BadRequest(format!(
            "{label} cannot contain control characters"
        )));
    }
    Ok(())
}

pub(super) fn validate_file_name(value: String) -> BootResult<String> {
    let value = value.trim().to_string();
    validate_name(&value, "source file name")?;
    if value.contains('/') || value.contains('\\') {
        return Err(BootError::BadRequest(
            "source file name cannot contain path separators".to_string(),
        ));
    }
    Ok(value)
}

pub(super) fn validate_content_type(value: Option<String>) -> BootResult<String> {
    let value = value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("application/octet-stream");
    if value.len() > 255 || value.chars().any(char::is_control) {
        return Err(BootError::BadRequest(
            "source content type is invalid".to_string(),
        ));
    }
    Ok(value.to_string())
}

pub(super) fn ensure_revision(
    resource: &str,
    id: &str,
    expected_revision: u64,
    actual_revision: u64,
) -> BootResult<()> {
    if expected_revision == actual_revision {
        Ok(())
    } else {
        Err(revision_conflict(resource, id, actual_revision))
    }
}

pub(super) fn revision_conflict(resource: &str, id: &str, actual_revision: u64) -> BootError {
    BootError::Conflict(format!(
        "Work {resource} `{id}` changed on the server; current revision is {actual_revision}"
    ))
}

pub(super) fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
