use bitwarden_csi_core::bitwarden::SdkBitwardenClient;
use bitwarden_csi_core::provider::handle_mount;
use bitwarden_csi_proto::v1alpha1::csi_driver_provider_server::CsiDriverProvider;
use bitwarden_csi_proto::v1alpha1::{
    File, MountRequest, MountResponse, VersionRequest, VersionResponse,
};
use tonic::{Request, Response, Status};
use tracing::{error, info};

pub struct BwsCsiProviderService;

#[tonic::async_trait]
impl CsiDriverProvider for BwsCsiProviderService {
    async fn version(
        &self,
        _request: Request<VersionRequest>,
    ) -> Result<Response<VersionResponse>, Status> {
        info!("version RPC called");
        Ok(Response::new(VersionResponse {
            version: "v1alpha1".into(),
            runtime_name: "bitwarden".into(),
            runtime_version: env!("CARGO_PKG_VERSION").into(),
        }))
    }

    async fn mount(
        &self,
        request: Request<MountRequest>,
    ) -> Result<Response<MountResponse>, Status> {
        let req = request.into_inner();
        info!("mount RPC called");

        let client = SdkBitwardenClient::new();

        match handle_mount(&client, &req.attributes, &req.secrets).await {
            Ok(mounted_files) => {
                let object_versions: Vec<_> = mounted_files
                    .iter()
                    .map(|f| bitwarden_csi_proto::v1alpha1::ObjectVersion {
                        id: f.path.clone(),
                        version: String::new(),
                    })
                    .collect();

                let files: Vec<File> = mounted_files
                    .into_iter()
                    .map(|f| File {
                        path: f.path,
                        mode: f.mode,
                        contents: f.contents,
                    })
                    .collect();

                Ok(Response::new(MountResponse {
                    object_version: object_versions,
                    error: None,
                    files,
                }))
            }
            Err(e) => {
                error!(error = %e, code = %e.error_code(), "mount failed");
                Ok(Response::new(MountResponse {
                    object_version: vec![],
                    error: Some(bitwarden_csi_proto::v1alpha1::Error {
                        code: e.error_code().to_string(),
                    }),
                    files: vec![],
                }))
            }
        }
    }
}
