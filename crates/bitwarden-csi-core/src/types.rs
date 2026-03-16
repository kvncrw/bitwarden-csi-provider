use serde::Deserialize;
use uuid::Uuid;

/// A single secret specification from the SecretProviderClass parameters.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SecretSpec {
    /// Fetch a single secret by UUID.
    pub id: Option<Uuid>,
    /// Fetch all secrets in a project by UUID.
    pub project: Option<Uuid>,
    /// Filename (relative path) for single-secret mounts.
    pub path: Option<String>,
    /// Prefix prepended to secret keys for project-based mounts.
    #[serde(rename = "pathPrefix")]
    pub path_prefix: Option<String>,
}

/// Data returned from the Bitwarden Secrets Manager API.
#[derive(Debug, Clone)]
pub struct SecretData {
    pub id: Uuid,
    pub key: String,
    pub value: String,
}

/// A file to be mounted into the pod.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountedFile {
    pub path: String,
    pub contents: Vec<u8>,
    pub mode: i32,
}

/// Default file mode for mounted secrets (0444 — read-only for all).
pub const DEFAULT_FILE_MODE: i32 = 0o444;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_spec_deserialize_by_id() {
        let yaml = r#"
id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
path: "my-secret"
"#;
        let spec: SecretSpec = serde_yaml::from_str(yaml).unwrap();
        assert!(spec.id.is_some());
        assert_eq!(spec.path.as_deref(), Some("my-secret"));
        assert!(spec.project.is_none());
    }

    #[test]
    fn secret_spec_deserialize_by_project() {
        let yaml = r#"
project: "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
pathPrefix: "proj/"
"#;
        let spec: SecretSpec = serde_yaml::from_str(yaml).unwrap();
        assert!(spec.project.is_some());
        assert_eq!(spec.path_prefix.as_deref(), Some("proj/"));
        assert!(spec.id.is_none());
    }

    #[test]
    fn secret_spec_rejects_unknown_fields() {
        let yaml = r#"
id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
unknown_field: "value"
"#;
        let result: Result<SecretSpec, _> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn mounted_file_default_mode() {
        assert_eq!(DEFAULT_FILE_MODE, 0o444);
        assert_eq!(DEFAULT_FILE_MODE, 292); // decimal
    }

    #[test]
    fn secret_data_construction() {
        let data = SecretData {
            id: Uuid::nil(),
            key: "db-password".into(),
            value: "hunter2".into(),
        };
        assert_eq!(data.key, "db-password");
        assert_eq!(data.value, "hunter2");
    }
}
