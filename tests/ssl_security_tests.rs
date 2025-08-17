#[cfg(test)]
mod ssl_tests {
    use std::env;
    
    #[test]
    fn test_ssl_config_default_verification() {
        // Test that SSL verification is enabled by default in production
        env::remove_var("TEST_SSL_VERIFY_PEER");
        env::remove_var("TEST_DEV_MODE");
        
        let config = jupiter::ssl_config::SslConfig::new("TEST");
        assert!(config.verify_peer, "SSL verification should be enabled by default");
    }
    
    #[test]
    fn test_ssl_config_dev_mode() {
        // Test that dev mode allows disabling SSL verification
        env::set_var("TEST_DEV_MODE", "true");
        env::remove_var("TEST_SSL_VERIFY_PEER");
        
        let config = jupiter::ssl_config::SslConfig::new("TEST");
        assert!(!config.verify_peer, "SSL verification should be disabled in dev mode by default");
        
        // Clean up
        env::remove_var("TEST_DEV_MODE");
    }
    
    #[test]
    fn test_ssl_config_explicit_verification() {
        // Test that explicit verification setting overrides defaults
        env::set_var("TEST_SSL_VERIFY_PEER", "true");
        env::set_var("TEST_DEV_MODE", "true");
        
        let config = jupiter::ssl_config::SslConfig::new("TEST");
        assert!(config.verify_peer, "Explicit SSL verification setting should override dev mode");
        
        // Clean up
        env::remove_var("TEST_SSL_VERIFY_PEER");
        env::remove_var("TEST_DEV_MODE");
    }
    
    #[test]
    fn test_ssl_config_ca_cert_path() {
        // Test that CA certificate path is loaded from environment
        let test_path = "/path/to/ca/cert.pem";
        env::set_var("TEST_CA_CERT_PATH", test_path);
        
        let config = jupiter::ssl_config::SslConfig::new("TEST");
        assert_eq!(config.ca_cert_path, Some(test_path.to_string()));
        
        // Clean up
        env::remove_var("TEST_CA_CERT_PATH");
    }
    
    #[test]
    fn test_sslmode_selection() {
        // Test that correct sslmode is returned based on verification setting
        env::remove_var("TEST_DEV_MODE");
        env::set_var("TEST_SSL_VERIFY_PEER", "true");
        
        let config = jupiter::ssl_config::SslConfig::new("TEST");
        assert_eq!(config.get_sslmode(), "require");
        
        env::set_var("TEST_SSL_VERIFY_PEER", "false");
        let config = jupiter::ssl_config::SslConfig::new("TEST");
        assert_eq!(config.get_sslmode(), "prefer");
        
        // Clean up
        env::remove_var("TEST_SSL_VERIFY_PEER");
    }
    
    #[test]
    fn test_homebrew_connector_creation() {
        // Test that Homebrew connector can be created
        env::set_var("HOMEBREW_SSL_VERIFY_PEER", "true");
        
        let result = jupiter::ssl_config::create_homebrew_connector();
        assert!(result.is_ok(), "Should be able to create Homebrew SSL connector");
        
        // Clean up
        env::remove_var("HOMEBREW_SSL_VERIFY_PEER");
    }
    
    #[test]
    fn test_combo_connector_creation() {
        // Test that Combo connector can be created
        env::set_var("COMBO_SSL_VERIFY_PEER", "true");
        
        let result = jupiter::ssl_config::create_combo_connector();
        assert!(result.is_ok(), "Should be able to create Combo SSL connector");
        
        // Clean up
        env::remove_var("COMBO_SSL_VERIFY_PEER");
    }
}

#[cfg(test)]
mod integration_tests {
    use std::env;
    use std::process::Command;
    
    #[test]
    #[ignore] // Run with: cargo test -- --ignored
    fn test_ssl_connection_with_invalid_cert() {
        // This test requires a test PostgreSQL server with an invalid certificate
        // It verifies that connections fail when certificate verification is enabled
        
        env::set_var("TEST_PG_ADDRESS", "localhost:5432");
        env::set_var("TEST_PG_DBNAME", "testdb");
        env::set_var("TEST_PG_USER", "testuser");
        env::set_var("TEST_PG_PASS", "testpass");
        env::set_var("TEST_SSL_VERIFY_PEER", "true");
        
        // Attempt to connect should fail with invalid certificate
        // This is a placeholder - actual implementation would depend on test infrastructure
        
        // Clean up
        env::remove_var("TEST_PG_ADDRESS");
        env::remove_var("TEST_PG_DBNAME");
        env::remove_var("TEST_PG_USER");
        env::remove_var("TEST_PG_PASS");
        env::remove_var("TEST_SSL_VERIFY_PEER");
    }
    
    #[test]
    #[ignore] // Run with: cargo test -- --ignored
    fn test_ssl_connection_with_valid_cert() {
        // This test requires a test PostgreSQL server with a valid certificate
        // It verifies that connections succeed when certificate verification is enabled
        
        env::set_var("TEST_PG_ADDRESS", "localhost:5432");
        env::set_var("TEST_PG_DBNAME", "testdb");
        env::set_var("TEST_PG_USER", "testuser");
        env::set_var("TEST_PG_PASS", "testpass");
        env::set_var("TEST_SSL_VERIFY_PEER", "true");
        env::set_var("TEST_CA_CERT_PATH", "/path/to/valid/ca.pem");
        
        // Attempt to connect should succeed with valid certificate
        // This is a placeholder - actual implementation would depend on test infrastructure
        
        // Clean up
        env::remove_var("TEST_PG_ADDRESS");
        env::remove_var("TEST_PG_DBNAME");
        env::remove_var("TEST_PG_USER");
        env::remove_var("TEST_PG_PASS");
        env::remove_var("TEST_SSL_VERIFY_PEER");
        env::remove_var("TEST_CA_CERT_PATH");
    }
}