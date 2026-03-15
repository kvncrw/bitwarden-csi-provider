use std::sync::Arc;

use async_trait::async_trait;
use bitwarden::auth::login::AccessTokenLoginRequest;
use bitwarden::secrets_manager::secrets::{
    SecretGetRequest, SecretIdentifiersByProjectRequest, SecretsGetRequest,
};
use bitwarden::secrets_manager::ClientSecretsExt;
use bitwarden::{Client, ClientSettings, DeviceType};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::error::ProviderError;
use crate::types::SecretData;

/// Client interface for Bitwarden Secrets Manager.
///
/// The trait exists for URL injection — tests point at the fake-server,
/// production points at api.bitwarden.com. Both sides use the real SDK.
#[async_trait]
pub trait BitwardenClient: Send + Sync {
    async fn authenticate(&self, access_token: &str) -> Result<(), ProviderError>;
    async fn get_secret(&self, id: Uuid) -> Result<SecretData, ProviderError>;
    async fn list_secrets_by_project(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<SecretData>, ProviderError>;
}

/// Real implementation wrapping the Bitwarden SDK.
pub struct SdkBitwardenClient {
    api_url: String,
    identity_url: String,
    client: Arc<Mutex<Option<Client>>>,
}

impl SdkBitwardenClient {
    /// Create a client pointing at the official Bitwarden API.
    pub fn new() -> Self {
        Self {
            api_url: "https://api.bitwarden.com".into(),
            identity_url: "https://identity.bitwarden.com".into(),
            client: Arc::new(Mutex::new(None)),
        }
    }

    /// Create a client pointing at a custom server (e.g., fake-server for tests).
    pub fn with_urls(api_url: String, identity_url: String) -> Self {
        Self {
            api_url,
            identity_url,
            client: Arc::new(Mutex::new(None)),
        }
    }

    fn create_sdk_client(&self) -> Client {
        let settings = ClientSettings {
            identity_url: self.identity_url.clone(),
            api_url: self.api_url.clone(),
            user_agent: "bws-csi-provider".to_string(),
            device_type: DeviceType::SDK,
            ..Default::default()
        };
        Client::new(Some(settings))
    }
}

impl Default for SdkBitwardenClient {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BitwardenClient for SdkBitwardenClient {
    async fn authenticate(&self, access_token: &str) -> Result<(), ProviderError> {
        let sdk_client = self.create_sdk_client();

        let req = AccessTokenLoginRequest {
            access_token: access_token.to_string(),
            state_file: None,
        };

        let response = sdk_client
            .auth()
            .login_access_token(&req)
            .await
            .map_err(|e| ProviderError::AuthFailed(e.to_string()))?;

        if !response.authenticated {
            return Err(ProviderError::AuthFailed(
                "authentication returned false".into(),
            ));
        }

        let mut guard = self.client.lock().await;
        *guard = Some(sdk_client);
        Ok(())
    }

    async fn get_secret(&self, id: Uuid) -> Result<SecretData, ProviderError> {
        let guard = self.client.lock().await;
        let client = guard
            .as_ref()
            .ok_or_else(|| ProviderError::AuthFailed("not authenticated".into()))?;

        let req = SecretGetRequest { id };
        let response = client
            .secrets()
            .get(&req)
            .await
            .map_err(|e| ProviderError::SdkError(e.to_string()))?;

        Ok(SecretData {
            id: response.id,
            key: response.key,
            value: response.value,
        })
    }

    async fn list_secrets_by_project(
        &self,
        project_id: Uuid,
    ) -> Result<Vec<SecretData>, ProviderError> {
        let guard = self.client.lock().await;
        let client = guard
            .as_ref()
            .ok_or_else(|| ProviderError::AuthFailed("not authenticated".into()))?;

        // First get the secret identifiers for the project
        let list_req = SecretIdentifiersByProjectRequest { project_id };
        let identifiers = client
            .secrets()
            .list_by_project(&list_req)
            .await
            .map_err(|e| ProviderError::SdkError(e.to_string()))?;

        if identifiers.data.is_empty() {
            return Ok(vec![]);
        }

        // Then fetch full secrets by their IDs
        let ids: Vec<Uuid> = identifiers.data.iter().map(|s| s.id).collect();
        let get_req = SecretsGetRequest { ids };
        let secrets = client
            .secrets()
            .get_by_ids(get_req)
            .await
            .map_err(|e| ProviderError::SdkError(e.to_string()))?;

        Ok(secrets
            .data
            .into_iter()
            .map(|s| SecretData {
                id: s.id,
                key: s.key,
                value: s.value,
            })
            .collect())
    }
}
