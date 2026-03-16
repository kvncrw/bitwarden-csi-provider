//! Integration tests for the bitwarden-csi-provider gRPC server.
//!
//! These tests start the gRPC server on a temp Unix socket and exercise
//! the Version and Mount RPCs. Mount tests require the bitwarden fake-server
//! to be running (see `FakeServer` helper).

use std::sync::Once;

use bitwarden_csi_proto::v1alpha1::csi_driver_provider_server::CsiDriverProviderServer;
use bitwarden_csi_proto::v1alpha1::{MountRequest, VersionRequest};
use tempfile::TempDir;
use tokio::net::UnixListener;
use tokio_stream::wrappers::UnixListenerStream;
use hyper_util::rt::TokioIo;
use tonic::transport::{Channel, Endpoint, Server, Uri};
use tower::service_fn;

mod fake_server;
use fake_server::FakeServer;

static INIT: Once = Once::new();

fn init_tracing() {
    INIT.call_once(|| {
        tracing_subscriber::fmt()
            .with_test_writer()
            .with_env_filter("bitwarden_csi=debug")
            .try_init()
            .ok();
    });
}

/// Helper: start the gRPC provider server on a temp Unix socket and return a client.
async fn start_server_and_client(
    fake: &FakeServer,
) -> bitwarden_csi_proto::v1alpha1::csi_driver_provider_client::CsiDriverProviderClient<Channel> {
    let tmp = TempDir::new().unwrap();
    let socket_path = tmp.path().join("test.sock");

    let uds = UnixListener::bind(&socket_path).unwrap();
    let uds_stream = UnixListenerStream::new(uds);

    let service = TestProviderService::new(fake.base_url());

    tokio::spawn(async move {
        Server::builder()
            .add_service(CsiDriverProviderServer::new(service))
            .serve_with_incoming(uds_stream)
            .await
            .unwrap();
    });

    // Give the server a moment to bind
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // Connect via Unix socket
    let socket_path_clone = socket_path.clone();
    let channel = Endpoint::try_from("http://[::]:50051")
        .unwrap()
        .connect_with_connector(service_fn(move |_: Uri| {
            let path = socket_path_clone.clone();
            async move {
                let stream = tokio::net::UnixStream::connect(path).await?;
                Ok::<_, std::io::Error>(TokioIo::new(stream))
            }
        }))
        .await
        .unwrap();

    bitwarden_csi_proto::v1alpha1::csi_driver_provider_client::CsiDriverProviderClient::new(channel)
}

/// A test-specific provider service that uses a custom BSM URL.
struct TestProviderService {
    bsm_base_url: String,
}

impl TestProviderService {
    fn new(bsm_base_url: String) -> Self {
        Self { bsm_base_url }
    }
}

#[tonic::async_trait]
impl bitwarden_csi_proto::v1alpha1::csi_driver_provider_server::CsiDriverProvider
    for TestProviderService
{
    async fn version(
        &self,
        _request: tonic::Request<VersionRequest>,
    ) -> Result<tonic::Response<bitwarden_csi_proto::v1alpha1::VersionResponse>, tonic::Status> {
        Ok(tonic::Response::new(
            bitwarden_csi_proto::v1alpha1::VersionResponse {
                version: "v1alpha1".into(),
                runtime_name: "bitwarden".into(),
                runtime_version: env!("CARGO_PKG_VERSION").into(),
            },
        ))
    }

    async fn mount(
        &self,
        request: tonic::Request<MountRequest>,
    ) -> Result<tonic::Response<bitwarden_csi_proto::v1alpha1::MountResponse>, tonic::Status> {
        let req = request.into_inner();

        let client = bitwarden_csi_core::bitwarden::SdkBitwardenClient::with_urls(
            format!("{}/api", self.bsm_base_url),
            format!("{}/identity", self.bsm_base_url),
        );

        match bitwarden_csi_core::provider::handle_mount(&client, &req.attributes, &req.secrets).await {
            Ok(files) => {
                let object_versions: Vec<_> = files
                    .iter()
                    .map(|f| bitwarden_csi_proto::v1alpha1::ObjectVersion {
                        id: f.path.clone(),
                        version: String::new(),
                    })
                    .collect();

                let grpc_files: Vec<_> = files
                    .into_iter()
                    .map(|f| bitwarden_csi_proto::v1alpha1::File {
                        path: f.path,
                        mode: f.mode,
                        contents: f.contents,
                    })
                    .collect();

                Ok(tonic::Response::new(
                    bitwarden_csi_proto::v1alpha1::MountResponse {
                        object_version: object_versions,
                        error: None,
                        files: grpc_files,
                    },
                ))
            }
            Err(e) => Ok(tonic::Response::new(
                bitwarden_csi_proto::v1alpha1::MountResponse {
                    object_version: vec![],
                    error: Some(bitwarden_csi_proto::v1alpha1::Error {
                        code: e.error_code().to_string(),
                    }),
                    files: vec![],
                },
            )),
        }
    }
}

// ── Version RPC ──

#[tokio::test]
async fn grpc_version_returns_correct_info() {
    init_tracing();
    let fake = FakeServer::start().await;
    let mut client = start_server_and_client(&fake).await;

    let response = client
        .version(VersionRequest {
            version: "v1alpha1".into(),
        })
        .await
        .unwrap()
        .into_inner();

    assert_eq!(response.version, "v1alpha1");
    assert_eq!(response.runtime_name, "bitwarden");
    assert!(!response.runtime_version.is_empty());
}

// ── Mount RPC — invalid params ──

#[tokio::test]
async fn grpc_mount_missing_objects_param() {
    init_tracing();
    let fake = FakeServer::start().await;
    let mut client = start_server_and_client(&fake).await;

    let response = client
        .mount(MountRequest {
            attributes: r#"{"no_objects": "here"}"#.into(),
            secrets: format!(r#"{{"access_token": "{}"}}"#, fake.access_token()),
            target_path: String::new(),
            permission: String::new(),
            current_object_version: vec![],
        })
        .await
        .unwrap()
        .into_inner();

    assert!(response.error.is_some());
    assert_eq!(response.error.unwrap().code, "INVALID_ARGUMENT");
}

#[tokio::test]
async fn grpc_mount_missing_access_token() {
    init_tracing();
    let fake = FakeServer::start().await;
    let mut client = start_server_and_client(&fake).await;

    let objects = r#"- id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
  path: "test""#;

    let response = client
        .mount(MountRequest {
            attributes: format!(r#"{{"objects": {}}}"#, serde_json::to_string(objects).unwrap()),
            secrets: r#"{"wrong_key": "value"}"#.into(),
            target_path: String::new(),
            permission: String::new(),
            current_object_version: vec![],
        })
        .await
        .unwrap()
        .into_inner();

    assert!(response.error.is_some());
    assert_eq!(response.error.unwrap().code, "INVALID_ARGUMENT");
}

#[tokio::test]
async fn grpc_mount_bad_access_token() {
    init_tracing();
    let fake = FakeServer::start().await;
    let mut client = start_server_and_client(&fake).await;

    let objects = r#"- id: "d1b2c3a4-e5f6-7890-abcd-ef1234567890"
  path: "test""#;

    let response = client
        .mount(MountRequest {
            attributes: format!(r#"{{"objects": {}}}"#, serde_json::to_string(objects).unwrap()),
            secrets: r#"{"access_token": "invalid-token"}"#.into(),
            target_path: String::new(),
            permission: String::new(),
            current_object_version: vec![],
        })
        .await
        .unwrap()
        .into_inner();

    assert!(response.error.is_some());
    assert_eq!(response.error.unwrap().code, "AUTHENTICATION_FAILED");
}

// ── Mount RPC — success (requires fake-server with secrets) ──

#[tokio::test]
async fn grpc_mount_single_secret() {
    init_tracing();
    let fake = FakeServer::start().await;

    if !fake.is_available() {
        eprintln!("SKIP: fake-server not available");
        return;
    }

    let mut client = start_server_and_client(&fake).await;

    // The fake-server has a pre-seeded secret. Use its known UUID.
    let secret_id = fake.seeded_secret_id();
    let objects = format!(
        r#"- id: "{}"
  path: "my-secret""#,
        secret_id
    );

    let response = client
        .mount(MountRequest {
            attributes: format!(r#"{{"objects": {}}}"#, serde_json::to_string(&objects).unwrap()),
            secrets: format!(r#"{{"access_token": "{}"}}"#, fake.access_token()),
            target_path: String::new(),
            permission: String::new(),
            current_object_version: vec![],
        })
        .await
        .unwrap()
        .into_inner();

    if let Some(err) = &response.error {
        panic!("mount returned error: {}", err.code);
    }

    assert_eq!(response.files.len(), 1);
    assert_eq!(response.files[0].path, "my-secret");
    assert!(!response.files[0].contents.is_empty());
}

// ── Socket lifecycle ──

#[tokio::test]
async fn socket_cleanup_on_start() {
    let tmp = TempDir::new().unwrap();
    let socket_path = tmp.path().join("cleanup-test.sock");

    // Create a stale socket file
    std::fs::write(&socket_path, b"stale").unwrap();
    assert!(socket_path.exists());

    // Remove it like main.rs does
    std::fs::remove_file(&socket_path).unwrap();
    assert!(!socket_path.exists());

    // Verify we can bind to it now
    let _uds = UnixListener::bind(&socket_path).unwrap();
    assert!(socket_path.exists());
}
