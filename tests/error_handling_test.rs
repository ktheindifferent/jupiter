use jupiter::error::{JupiterError, Result as JupiterResult};
use std::env;

#[test]
fn test_configuration_error_from_env_var() {
    // Test that missing environment variables return proper errors
    env::remove_var("TEST_VAR_THAT_DOESNT_EXIST");
    
    let result: JupiterResult<String> = env::var("TEST_VAR_THAT_DOESNT_EXIST")
        .map_err(|e| JupiterError::from(e));
    
    assert!(result.is_err());
    match result {
        Err(JupiterError::ConfigurationError(msg)) => {
            assert!(msg.contains("Environment variable error"));
        },
        _ => panic!("Expected ConfigurationError"),
    }
}

#[test]
fn test_error_display() {
    // Test that errors display properly formatted messages
    let error = JupiterError::ConfigurationError("Test config error".to_string());
    let display = format!("{}", error);
    assert_eq!(display, "Configuration error: Test config error");
    
    let error = JupiterError::ValidationError("Invalid input".to_string());
    let display = format!("{}", error);
    assert_eq!(display, "Validation error: Invalid input");
    
    let error = JupiterError::AuthenticationError("Unauthorized".to_string());
    let display = format!("{}", error);
    assert_eq!(display, "Authentication error: Unauthorized");
    
    let error = JupiterError::RateLimitError("Too many requests".to_string());
    let display = format!("{}", error);
    assert_eq!(display, "Rate limit error: Too many requests");
}

#[test]
fn test_error_conversion_from_io_error() {
    use std::io;
    
    let io_error = io::Error::new(io::ErrorKind::NotFound, "File not found");
    let jupiter_error: JupiterError = io_error.into();
    
    match jupiter_error {
        JupiterError::IoError(_) => {}, // Expected
        _ => panic!("Expected IoError variant"),
    }
}

#[test]
fn test_error_conversion_from_serde_json() {
    let invalid_json = "{ invalid json }";
    let result: Result<serde_json::Value, serde_json::Error> = serde_json::from_str(invalid_json);
    
    assert!(result.is_err());
    
    let jupiter_error: JupiterError = result.unwrap_err().into();
    match jupiter_error {
        JupiterError::SerializationError(_) => {}, // Expected
        _ => panic!("Expected SerializationError variant"),
    }
}

#[cfg(test)]
mod postgres_server_tests {
    use jupiter::provider::homebrew::PostgresServer;
    use jupiter::provider::combo::PostgresServer as ComboPostgresServer;
    use std::env;
    
    #[test]
    fn test_homebrew_postgres_server_missing_env_vars() {
        // Remove all required environment variables
        env::remove_var("HOMEBREW_PG_DBNAME");
        env::remove_var("HOMEBREW_PG_USER");
        env::remove_var("HOMEBREW_PG_PASS");
        env::remove_var("HOMEBREW_PG_ADDRESS");
        
        let result = PostgresServer::new();
        assert!(result.is_err());
        
        match result {
            Err(e) => {
                let error_msg = format!("{}", e);
                assert!(error_msg.contains("Configuration error"));
            },
            Ok(_) => panic!("Expected error when environment variables are missing"),
        }
    }
    
    #[test]
    fn test_combo_postgres_server_missing_env_vars() {
        // Remove all required environment variables
        env::remove_var("COMBO_PG_DBNAME");
        env::remove_var("COMBO_PG_USER");
        env::remove_var("COMBO_PG_PASS");
        env::remove_var("COMBO_PG_ADDRESS");
        
        let result = ComboPostgresServer::new();
        assert!(result.is_err());
        
        match result {
            Err(e) => {
                let error_msg = format!("{}", e);
                assert!(error_msg.contains("Configuration error"));
            },
            Ok(_) => panic!("Expected error when environment variables are missing"),
        }
    }
    
    #[test]
    fn test_postgres_server_with_valid_env_vars() {
        // Set all required environment variables
        env::set_var("HOMEBREW_PG_DBNAME", "test_db");
        env::set_var("HOMEBREW_PG_USER", "test_user");
        env::set_var("HOMEBREW_PG_PASS", "test_pass");
        env::set_var("HOMEBREW_PG_ADDRESS", "localhost:5432");
        
        let result = PostgresServer::new();
        assert!(result.is_ok());
        
        if let Ok(server) = result {
            assert_eq!(server.db_name, "test_db");
            assert_eq!(server.username, "test_user");
            assert_eq!(server.password, "test_pass");
            assert_eq!(server.address, "localhost:5432");
        }
    }
}

#[cfg(test)]
mod rate_limiter_tests {
    use jupiter::auth::RateLimiter;
    use std::thread;
    use std::time::Duration;
    
    #[test]
    fn test_rate_limiter_allows_requests_within_limit() {
        let limiter = RateLimiter::new(3, 60);
        
        // First 3 requests should be allowed
        assert!(limiter.check_rate_limit("client1"));
        assert!(limiter.check_rate_limit("client1"));
        assert!(limiter.check_rate_limit("client1"));
        
        // 4th request should be denied
        assert!(!limiter.check_rate_limit("client1"));
    }
    
    #[test]
    fn test_rate_limiter_different_clients() {
        let limiter = RateLimiter::new(2, 60);
        
        // Each client should have its own limit
        assert!(limiter.check_rate_limit("client1"));
        assert!(limiter.check_rate_limit("client1"));
        assert!(!limiter.check_rate_limit("client1")); // client1 exhausted
        
        // client2 should still be allowed
        assert!(limiter.check_rate_limit("client2"));
        assert!(limiter.check_rate_limit("client2"));
        assert!(!limiter.check_rate_limit("client2")); // client2 exhausted
    }
    
    #[test]
    fn test_rate_limiter_window_reset() {
        let limiter = RateLimiter::new(2, 1); // 2 requests per second
        
        // Use up the limit
        assert!(limiter.check_rate_limit("client1"));
        assert!(limiter.check_rate_limit("client1"));
        assert!(!limiter.check_rate_limit("client1"));
        
        // Wait for window to reset
        thread::sleep(Duration::from_millis(1100));
        
        // Should be allowed again
        assert!(limiter.check_rate_limit("client1"));
    }
}

#[cfg(test)]
mod auth_tests {
    use jupiter::auth::constant_time_eq;
    
    #[test]
    fn test_constant_time_comparison() {
        // Equal strings
        assert!(constant_time_eq(b"hello", b"hello"));
        assert!(constant_time_eq(b"", b""));
        assert!(constant_time_eq(b"test123", b"test123"));
        
        // Different strings
        assert!(!constant_time_eq(b"hello", b"world"));
        assert!(!constant_time_eq(b"hello", b"hello!"));
        assert!(!constant_time_eq(b"", b"hello"));
        
        // Different lengths
        assert!(!constant_time_eq(b"short", b"longer string"));
    }
}