// src/config.rs (or within daemon.rs/main.rs initially)

use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Clone)] // Clone might be useful
pub struct Settings {
    #[serde(default = "default_listen_port")] // Provide defaults
    pub listen_port: u16,
    #[serde(default = "default_save_directory")]
    pub save_directory: PathBuf,
    #[serde(default = "default_socket_path")]
    pub socket_path: String,
    #[serde(default = "default_max_peers")]
    pub max_peer_connections_per_torrent: usize,
    // Add other settings: log level, max download/upload speed, etc.
}

pub fn default_listen_port() -> u16 {
    6881
}
pub fn default_save_directory() -> PathBuf {
    PathBuf::from("/tmp/bittorrent_downloads")
} // Or use dirs crate
pub fn default_socket_path() -> String {
    "/tmp/bittorent-protocol.tmp".to_string()
}
pub fn default_max_peers() -> usize {
    50
}

// Implement a method to load the configuration
impl Settings {
    pub fn new() -> Result<Self, config::ConfigError> {
        // Find config directory (optional, using directories crate)
        let config_dir = directories::ProjectDirs::from("com", "YourOrg", "BittorrentDaemon")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from(".")); // Default to current dir if not found

        let config_file_path = config_dir.join("daemon.toml");
        let config_file_path_str = config_file_path.to_str().unwrap_or("daemon.toml"); // config needs &str

        println!(
            "Attempting to load configuration from: {}",
            config_file_path_str
        ); // Log path

        let s = config::Config::builder()
            // 1. Start with default values (built into your struct via #[serde(default = ...)])
            // (The config crate doesn't directly use struct defaults, but they apply during deserialization)
            // 2. Add configuration file(s).
            //    .add_source() returns the builder, files are optional.
            .add_source(config::File::with_name(config_file_path_str).required(false))
            // 3. Add environment variables (optional).
            //    Prefix with `APP_` (e.g., `APP_LISTEN_PORT=8000`), Clap uses `_` separators
            .add_source(config::Environment::with_prefix("BT_DAEMON").separator("_"))
            .build()?;

        // Deserialize the entire configuration
        s.try_deserialize()
    }
}
