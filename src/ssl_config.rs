use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres_openssl::MakeTlsConnector;
use std::env;
use std::path::Path;
use std::error::Error as StdError;

/// SSL/TLS configuration for secure database connections
pub struct SslConfig {
    /// Optional path to custom CA certificate
    pub ca_cert_path: Option<String>,
    /// Whether to verify peer certificates (should always be true in production)
    pub verify_peer: bool,
    /// Environment prefix for configuration (e.g., "HOMEBREW", "COMBO")
    pub env_prefix: String,
}

impl SslConfig {
    /// Create a new SSL configuration with the given environment prefix
    pub fn new(env_prefix: &str) -> Self {
        let ca_cert_env = format!("{}_CA_CERT_PATH", env_prefix);
        let verify_env = format!("{}_SSL_VERIFY_PEER", env_prefix);
        let dev_mode_env = format!("{}_DEV_MODE", env_prefix);
        
        // Check if we're in development mode (for local testing with self-signed certs)
        let is_dev_mode = env::var(dev_mode_env).unwrap_or_default() == "true";
        
        // By default, verify peer certificates unless explicitly disabled for development
        let verify_peer = if is_dev_mode {
            env::var(verify_env).unwrap_or_else(|_| "false".to_string()) == "true"
        } else {
            env::var(verify_env).unwrap_or_else(|_| "true".to_string()) == "true"
        };
        
        Self {
            ca_cert_path: env::var(ca_cert_env).ok(),
            verify_peer,
            env_prefix: env_prefix.to_string(),
        }
    }
    
    /// Build an SSL connector with the configured settings
    pub fn build_connector(&self) -> Result<MakeTlsConnector, Box<dyn StdError>> {
        let mut builder = SslConnector::builder(SslMethod::tls())?;
        
        // Set verification mode
        if self.verify_peer {
            builder.set_verify(SslVerifyMode::PEER);
            log::info!("{}: SSL certificate verification enabled", self.env_prefix);
        } else {
            // WARNING: Only for development/testing - never use in production!
            log::warn!("{}: SSL certificate verification DISABLED - This is insecure and should only be used in development!", self.env_prefix);
            builder.set_verify(SslVerifyMode::NONE);
        }
        
        // Load custom CA certificate if provided
        if let Some(ref ca_path) = self.ca_cert_path {
            if Path::new(ca_path).exists() {
                match builder.set_ca_file(ca_path) {
                    Ok(_) => log::info!("{}: Loaded custom CA certificate from {}", self.env_prefix, ca_path),
                    Err(e) => {
                        log::error!("{}: Failed to load CA certificate from {}: {}", self.env_prefix, ca_path, e);
                        return Err(Box::new(e));
                    }
                }
            } else {
                log::warn!("{}: CA certificate path {} does not exist", self.env_prefix, ca_path);
            }
        }
        
        // Additional security settings
        // Set minimum TLS version to 1.2
        builder.set_min_proto_version(Some(openssl::ssl::SslVersion::TLS1_2))?;
        
        Ok(MakeTlsConnector::new(builder.build()))
    }
    
    /// Get the appropriate sslmode parameter for PostgreSQL connection string
    pub fn get_sslmode(&self) -> &str {
        if self.verify_peer {
            "require" // or "verify-full" for hostname verification
        } else {
            "prefer"
        }
    }
}

/// Create a secure SSL connector for Homebrew provider
pub fn create_homebrew_connector() -> Result<MakeTlsConnector, Box<dyn StdError>> {
    let config = SslConfig::new("HOMEBREW");
    config.build_connector()
}

/// Create a secure SSL connector for Combo provider
pub fn create_combo_connector() -> Result<MakeTlsConnector, Box<dyn StdError>> {
    let config = SslConfig::new("COMBO");
    config.build_connector()
}