use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "bitwarden-csi-provider", version, about = "Bitwarden Secrets Manager CSI Provider")]
pub struct Config {
    /// Path to the provider socket directory.
    #[arg(
        long,
        default_value = "/etc/kubernetes/secrets-store-csi-providers"
    )]
    pub provider_dir: String,

    /// Socket filename within the provider directory.
    #[arg(long, default_value = "bitwarden.sock")]
    pub socket_name: String,

    /// Health probe bind address.
    #[arg(long, default_value = "0.0.0.0:8080")]
    pub health_addr: String,

    /// Log format: "json" or "text".
    #[arg(long, default_value = "json")]
    pub log_format: String,
}

impl Config {
    pub fn socket_path(&self) -> std::path::PathBuf {
        std::path::Path::new(&self.provider_dir).join(&self.socket_name)
    }
}
