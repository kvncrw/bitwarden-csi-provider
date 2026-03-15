use std::collections::HashMap;

use tracing::{info, warn};
use uuid::Uuid;

use crate::bitwarden::BitwardenClient;
use crate::error::ProviderError;
use crate::secret_map::parse_secret_specs;
use crate::types::{MountedFile, SecretSpec, DEFAULT_FILE_MODE};

/// Orchestrates the mount operation: parse params → auth → fetch → map to files.
pub async fn handle_mount(
    client: &dyn BitwardenClient,
    attributes_json: &str,
    secrets_json: &str,
) -> Result<Vec<MountedFile>, ProviderError> {
    // Parse the access token from the secrets JSON (nodePublishSecretRef)
    let access_token = extract_access_token(secrets_json)?;

    // Parse the objects list from the attributes JSON (SPC parameters)
    let objects_yaml = extract_objects_param(attributes_json)?;
    let specs = parse_secret_specs(&objects_yaml)?;

    // Authenticate with BSM
    info!("authenticating with Bitwarden Secrets Manager");
    client.authenticate(&access_token).await?;

    // Fetch and map secrets to files
    let mut files = Vec::new();
    for spec in &specs {
        let mut spec_files = fetch_spec(client, spec).await?;
        files.append(&mut spec_files);
    }

    info!(file_count = files.len(), "mount completed");
    Ok(files)
}

fn extract_access_token(secrets_json: &str) -> Result<String, ProviderError> {
    let map: HashMap<String, String> = serde_yaml::from_str(secrets_json)
        .map_err(|e| ProviderError::InvalidParams(format!("failed to parse secrets: {e}")))?;

    map.get("access_token")
        .or_else(|| map.get("accessToken"))
        .cloned()
        .ok_or_else(|| {
            ProviderError::InvalidParams(
                "secrets must contain 'access_token' or 'accessToken' key".into(),
            )
        })
}

fn extract_objects_param(attributes_json: &str) -> Result<String, ProviderError> {
    let map: HashMap<String, String> = serde_yaml::from_str(attributes_json).map_err(|e| {
        ProviderError::InvalidParams(format!("failed to parse attributes: {e}"))
    })?;

    map.get("objects")
        .cloned()
        .ok_or_else(|| ProviderError::InvalidParams("attributes must contain 'objects' key".into()))
}

async fn fetch_spec(
    client: &dyn BitwardenClient,
    spec: &SecretSpec,
) -> Result<Vec<MountedFile>, ProviderError> {
    if let Some(id) = spec.id {
        let path = spec.path.as_deref().expect("validated: id requires path");
        let secret = client.get_secret(id).await?;
        info!(secret_id = %id, path, "fetched secret");
        Ok(vec![MountedFile {
            path: path.to_string(),
            contents: secret.value.into_bytes(),
            mode: DEFAULT_FILE_MODE,
        }])
    } else if let Some(project_id) = spec.project {
        let secrets = client.list_secrets_by_project(project_id).await?;
        let prefix = spec.path_prefix.as_deref().unwrap_or("");
        info!(
            project_id = %project_id,
            secret_count = secrets.len(),
            "fetched project secrets"
        );

        if secrets.is_empty() {
            warn!(project_id = %project_id, "project returned zero secrets");
        }

        let files: Result<Vec<_>, _> = secrets
            .into_iter()
            .map(|s| {
                let path = format!("{}{}", prefix, sanitize_key(&s.key));
                crate::secret_map::validate_path_str(&path)?;
                Ok(MountedFile {
                    path,
                    contents: s.value.into_bytes(),
                    mode: DEFAULT_FILE_MODE,
                })
            })
            .collect();
        files
    } else {
        unreachable!("validated: must have id or project")
    }
}

/// Sanitize a secret key for use as a filename.
/// Replaces path separators and other problematic characters.
fn sanitize_key(key: &str) -> String {
    key.chars()
        .map(|c| match c {
            '/' | '\\' | '\0' => '_',
            _ => c,
        })
        .collect()
}

/// Parse a secret UUID from a string, with a clear error message.
pub fn parse_uuid(s: &str) -> Result<Uuid, ProviderError> {
    Uuid::parse_str(s)
        .map_err(|e| ProviderError::InvalidParams(format!("invalid UUID '{s}': {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_access_token_snake_case() {
        let json = r#"{"access_token": "my-token-123"}"#;
        let token = extract_access_token(json).unwrap();
        assert_eq!(token, "my-token-123");
    }

    #[test]
    fn extract_access_token_camel_case() {
        let json = r#"{"accessToken": "my-token-456"}"#;
        let token = extract_access_token(json).unwrap();
        assert_eq!(token, "my-token-456");
    }

    #[test]
    fn extract_access_token_missing() {
        let json = r#"{"other_key": "value"}"#;
        let err = extract_access_token(json).unwrap_err();
        assert!(err.to_string().contains("access_token"));
    }

    #[test]
    fn extract_objects_param_success() {
        let json = r#"{"objects": "- id: abc\n  path: x"}"#;
        let objects = extract_objects_param(json).unwrap();
        assert!(objects.contains("id: abc"));
    }

    #[test]
    fn extract_objects_param_missing() {
        let json = r#"{"other": "value"}"#;
        let err = extract_objects_param(json).unwrap_err();
        assert!(err.to_string().contains("objects"));
    }

    #[test]
    fn sanitize_key_replaces_slashes() {
        assert_eq!(sanitize_key("path/to/secret"), "path_to_secret");
        assert_eq!(sanitize_key("back\\slash"), "back_slash");
    }

    #[test]
    fn sanitize_key_preserves_normal() {
        assert_eq!(sanitize_key("db-password"), "db-password");
        assert_eq!(sanitize_key("API_KEY"), "API_KEY");
    }

    #[test]
    fn parse_uuid_valid() {
        let uuid = parse_uuid("d1b2c3a4-e5f6-7890-abcd-ef1234567890").unwrap();
        assert_eq!(uuid.to_string(), "d1b2c3a4-e5f6-7890-abcd-ef1234567890");
    }

    #[test]
    fn parse_uuid_invalid() {
        let err = parse_uuid("not-a-uuid").unwrap_err();
        assert!(err.to_string().contains("invalid UUID"));
    }
}
