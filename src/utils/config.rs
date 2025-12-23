use dotenvy::dotenv;
use once_cell::sync::Lazy;
use std::env;

// Define the Config struct
#[derive(Clone, Debug)]
pub struct Config {
    pub output_folder: String,
    pub db_url: String,
    pub download_source_url: String,
    pub download_source_token: String,
    pub service_name: String,
    pub debug_traces: bool,
    pub otlp_endpoint: Option<String>,
    pub traces_endpoint: Option<String>,
}

// Initialize dotenv and config only once
pub static CONFIG: Lazy<Config> = Lazy::new(|| {
    dotenv().ok(); // Loads .env (only the first time it's called)

    Config {
        output_folder: get_non_empty_env_var("TEMP_FOLDER")
            .unwrap_or(String::from("/tmp/racemap-cell-service/data")),
        db_url: get_non_empty_env_var("DATABASE_URL").expect("DATABASE_URL must be set"),
        download_source_token: get_non_empty_env_var("DOWNLOAD_SOURCE_TOKEN")
            .expect("DOWNLOAD_SOURCE_TOKEN must be set"),
        download_source_url: get_non_empty_env_var("DOWNLOAD_SOURCE_URL")
            .unwrap_or(String::from("https://opencellid.org/ocid/downloads")),
        service_name: std::env::var("SERVICE_NAME").unwrap_or_else(|_| "cell-service".to_string()),
        debug_traces: std::env::var("OTEL_DEBUG_TRACES").is_ok(),
        otlp_endpoint: get_non_empty_env_var("OTEL_EXPORTER_OTLP_ENDPOINT"),
        traces_endpoint: get_non_empty_env_var("OTEL_TRACES_COLLECTOR_URL"),
    }
});

/// Helper function to get environment variable, treating empty strings as None
/// This handles Docker Compose behavior where unset variables become empty strings
fn get_non_empty_env_var(key: &str) -> Option<String> {
    env::var(key).ok().and_then(|s| {
        let trimmed = s.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}
