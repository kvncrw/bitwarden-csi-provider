use std::fmt;

/// Error codes returned in gRPC MountResponse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    InvalidArgument,
    AuthenticationFailed,
    SecretNotFound,
    ProviderError,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidArgument => write!(f, "INVALID_ARGUMENT"),
            Self::AuthenticationFailed => write!(f, "AUTHENTICATION_FAILED"),
            Self::SecretNotFound => write!(f, "SECRET_NOT_FOUND"),
            Self::ProviderError => write!(f, "PROVIDER_ERROR"),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ProviderError {
    #[error("invalid parameters: {0}")]
    InvalidParams(String),

    #[error("authentication failed: {0}")]
    AuthFailed(String),

    #[error("secret not found: {0}")]
    SecretNotFound(String),

    #[error("bitwarden SDK error: {0}")]
    SdkError(String),

    #[error("path validation failed: {0}")]
    PathValidation(String),

    #[error("YAML parse error: {0}")]
    YamlParse(String),
}

impl ProviderError {
    pub fn error_code(&self) -> ErrorCode {
        match self {
            Self::InvalidParams(_) | Self::PathValidation(_) | Self::YamlParse(_) => {
                ErrorCode::InvalidArgument
            }
            Self::AuthFailed(_) => ErrorCode::AuthenticationFailed,
            Self::SecretNotFound(_) => ErrorCode::SecretNotFound,
            Self::SdkError(_) => ErrorCode::ProviderError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_display() {
        assert_eq!(ErrorCode::InvalidArgument.to_string(), "INVALID_ARGUMENT");
        assert_eq!(
            ErrorCode::AuthenticationFailed.to_string(),
            "AUTHENTICATION_FAILED"
        );
        assert_eq!(ErrorCode::SecretNotFound.to_string(), "SECRET_NOT_FOUND");
        assert_eq!(ErrorCode::ProviderError.to_string(), "PROVIDER_ERROR");
    }

    #[test]
    fn error_to_code_mapping() {
        assert_eq!(
            ProviderError::InvalidParams("bad".into()).error_code(),
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            ProviderError::AuthFailed("denied".into()).error_code(),
            ErrorCode::AuthenticationFailed
        );
        assert_eq!(
            ProviderError::SecretNotFound("missing".into()).error_code(),
            ErrorCode::SecretNotFound
        );
        assert_eq!(
            ProviderError::SdkError("timeout".into()).error_code(),
            ErrorCode::ProviderError
        );
        assert_eq!(
            ProviderError::PathValidation("traversal".into()).error_code(),
            ErrorCode::InvalidArgument
        );
        assert_eq!(
            ProviderError::YamlParse("syntax".into()).error_code(),
            ErrorCode::InvalidArgument
        );
    }

    #[test]
    fn error_display_messages() {
        let err = ProviderError::InvalidParams("missing id".into());
        assert!(err.to_string().contains("missing id"));

        let err = ProviderError::AuthFailed("bad token".into());
        assert!(err.to_string().contains("bad token"));
    }
}
