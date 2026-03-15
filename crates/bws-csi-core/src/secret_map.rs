use crate::error::ProviderError;
use crate::types::SecretSpec;

/// Parse the `objects` parameter from a SecretProviderClass into a list of SecretSpecs.
///
/// The `objects` field is a YAML string embedded in the SPC parameters JSON.
pub fn parse_secret_specs(objects_yaml: &str) -> Result<Vec<SecretSpec>, ProviderError> {
    let specs: Vec<SecretSpec> =
        serde_yaml::from_str(objects_yaml).map_err(|e| ProviderError::YamlParse(e.to_string()))?;

    if specs.is_empty() {
        return Err(ProviderError::InvalidParams(
            "objects list must not be empty".into(),
        ));
    }

    for (i, spec) in specs.iter().enumerate() {
        validate_spec(spec, i)?;
    }

    check_duplicate_paths(&specs)?;

    Ok(specs)
}

fn validate_spec(spec: &SecretSpec, index: usize) -> Result<(), ProviderError> {
    match (&spec.id, &spec.project) {
        (None, None) => {
            return Err(ProviderError::InvalidParams(format!(
                "object[{index}]: must specify either 'id' or 'project'"
            )));
        }
        (Some(_), Some(_)) => {
            return Err(ProviderError::InvalidParams(format!(
                "object[{index}]: cannot specify both 'id' and 'project'"
            )));
        }
        _ => {}
    }

    // id-based spec requires path
    if spec.id.is_some() && spec.path.is_none() {
        return Err(ProviderError::InvalidParams(format!(
            "object[{index}]: 'id' requires 'path'"
        )));
    }

    // project-based spec should not have path (uses pathPrefix)
    if spec.project.is_some() && spec.path.is_some() {
        return Err(ProviderError::InvalidParams(format!(
            "object[{index}]: 'project' cannot use 'path', use 'pathPrefix' instead"
        )));
    }

    // Validate paths for traversal
    if let Some(path) = &spec.path {
        validate_path(path)?;
    }
    if let Some(prefix) = &spec.path_prefix {
        validate_path(prefix)?;
    }

    Ok(())
}

/// Validate a path string for use as a mount file path.
pub fn validate_path_str(path: &str) -> Result<(), ProviderError> {
    validate_path(path)
}

fn validate_path(path: &str) -> Result<(), ProviderError> {
    if path.is_empty() {
        return Err(ProviderError::PathValidation("path must not be empty".into()));
    }
    if path.starts_with('/') {
        return Err(ProviderError::PathValidation(format!(
            "path must be relative, got: {path}"
        )));
    }
    if path.contains("..") {
        return Err(ProviderError::PathValidation(format!(
            "path must not contain '..': {path}"
        )));
    }
    if path.contains('\0') {
        return Err(ProviderError::PathValidation(
            "path must not contain null bytes".into(),
        ));
    }
    Ok(())
}

fn check_duplicate_paths(specs: &[SecretSpec]) -> Result<(), ProviderError> {
    let mut seen = std::collections::HashSet::new();
    for spec in specs {
        if let Some(path) = &spec.path {
            if !seen.insert(path.as_str()) {
                return Err(ProviderError::InvalidParams(format!(
                    "duplicate path: {path}"
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_single_by_id() {
        let yaml = r#"
- id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
  path: "my-secret"
"#;
        let specs = parse_secret_specs(yaml).unwrap();
        assert_eq!(specs.len(), 1);
        assert!(specs[0].id.is_some());
        assert_eq!(specs[0].path.as_deref(), Some("my-secret"));
    }

    #[test]
    fn parse_single_by_project() {
        let yaml = r#"
- project: "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
  pathPrefix: "proj/"
"#;
        let specs = parse_secret_specs(yaml).unwrap();
        assert_eq!(specs.len(), 1);
        assert!(specs[0].project.is_some());
        assert_eq!(specs[0].path_prefix.as_deref(), Some("proj/"));
    }

    #[test]
    fn parse_multiple_mixed() {
        let yaml = r#"
- id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
  path: "secret-a"
- id: "e2c3d4b5-f6a7-8901-bcde-f12345678901"
  path: "secret-b"
- project: "f3d4e5c6-a7b8-9012-cdef-123456789012"
  pathPrefix: "bulk/"
"#;
        let specs = parse_secret_specs(yaml).unwrap();
        assert_eq!(specs.len(), 3);
    }

    #[test]
    fn reject_empty_list() {
        let yaml = "[]";
        let err = parse_secret_specs(yaml).unwrap_err();
        assert!(err.to_string().contains("must not be empty"));
    }

    #[test]
    fn reject_neither_id_nor_project() {
        let yaml = r#"
- path: "orphan"
"#;
        let err = parse_secret_specs(yaml).unwrap_err();
        assert!(err.to_string().contains("must specify either"));
    }

    #[test]
    fn reject_both_id_and_project() {
        let yaml = r#"
- id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
  project: "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
  path: "conflict"
"#;
        let err = parse_secret_specs(yaml).unwrap_err();
        assert!(err.to_string().contains("cannot specify both"));
    }

    #[test]
    fn reject_id_without_path() {
        let yaml = r#"
- id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
"#;
        let err = parse_secret_specs(yaml).unwrap_err();
        assert!(err.to_string().contains("requires 'path'"));
    }

    #[test]
    fn reject_project_with_path() {
        let yaml = r#"
- project: "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
  path: "wrong"
"#;
        let err = parse_secret_specs(yaml).unwrap_err();
        assert!(err.to_string().contains("cannot use 'path'"));
    }

    #[test]
    fn reject_absolute_path() {
        let yaml = r#"
- id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
  path: "/etc/shadow"
"#;
        let err = parse_secret_specs(yaml).unwrap_err();
        assert!(err.to_string().contains("must be relative"));
    }

    #[test]
    fn reject_path_traversal() {
        let yaml = r#"
- id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
  path: "../escape"
"#;
        let err = parse_secret_specs(yaml).unwrap_err();
        assert!(err.to_string().contains(".."));
    }

    #[test]
    fn reject_path_traversal_nested() {
        let yaml = r#"
- id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
  path: "subdir/../../escape"
"#;
        let err = parse_secret_specs(yaml).unwrap_err();
        assert!(err.to_string().contains(".."));
    }

    #[test]
    fn reject_null_byte_in_path() {
        let yaml = "- id: \"d1b2c3a4-e5f6-7890-abcd-ef1234567890\"\n  path: \"bad\\0path\"\n";
        let err = parse_secret_specs(yaml).unwrap_err();
        assert!(err.to_string().contains("null bytes"));
    }

    #[test]
    fn reject_duplicate_paths() {
        let yaml = r#"
- id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
  path: "same"
- id: "e2c3d4b5-f6a7-8901-bcde-f12345678901"
  path: "same"
"#;
        let err = parse_secret_specs(yaml).unwrap_err();
        assert!(err.to_string().contains("duplicate path"));
    }

    #[test]
    fn reject_malformed_yaml() {
        let yaml = "not: valid: yaml: [";
        let err = parse_secret_specs(yaml).unwrap_err();
        assert!(matches!(err, ProviderError::YamlParse(_)));
    }

    #[test]
    fn reject_unknown_fields() {
        let yaml = r#"
- id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
  path: "ok"
  hackme: "nope"
"#;
        let err = parse_secret_specs(yaml).unwrap_err();
        assert!(matches!(err, ProviderError::YamlParse(_)));
    }

    #[test]
    fn valid_nested_path() {
        let yaml = r#"
- id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
  path: "subdir/nested/secret.txt"
"#;
        let specs = parse_secret_specs(yaml).unwrap();
        assert_eq!(specs[0].path.as_deref(), Some("subdir/nested/secret.txt"));
    }

    #[test]
    fn project_with_prefix_and_trailing_slash() {
        let yaml = r#"
- project: "a1b2c3d4-e5f6-7890-abcd-ef1234567890"
  pathPrefix: "secrets/db/"
"#;
        let specs = parse_secret_specs(yaml).unwrap();
        assert_eq!(
            specs[0].path_prefix.as_deref(),
            Some("secrets/db/")
        );
    }
}
