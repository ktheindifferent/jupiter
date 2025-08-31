#[cfg(test)]
pub mod db_config {
    use crate::db_pool::DatabaseConfig;
    use std::env;
    use std::time::Duration;

    #[derive(Debug)]
    pub enum TestDbError {
        MissingRequiredVar(String),
        InvalidConfiguration(String),
    }

    impl std::fmt::Display for TestDbError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TestDbError::MissingRequiredVar(var) => {
                    write!(f, "Missing required environment variable: {}. Please set this variable or use the default test configuration.", var)
                }
                TestDbError::InvalidConfiguration(msg) => {
                    write!(f, "Invalid test database configuration: {}", msg)
                }
            }
        }
    }

    impl std::error::Error for TestDbError {}

    /// Get test database configuration with sensible defaults
    /// 
    /// This function will:
    /// 1. Try to read environment variables with the given prefix
    /// 2. Fall back to default values if not set
    /// 3. Return a properly configured DatabaseConfig for testing
    /// 
    /// # Arguments
    /// * `prefix` - The environment variable prefix (e.g., "HOMEBREW", "COMBO")
    /// * `require_real_db` - If true, will error if no env vars are set; if false, uses test defaults
    pub fn get_test_db_config(prefix: &str, require_real_db: bool) -> Result<DatabaseConfig, TestDbError> {
        let db_name_var = format!("{}_PG_DBNAME", prefix);
        let user_var = format!("{}_PG_USER", prefix);
        let pass_var = format!("{}_PG_PASS", prefix);
        let address_var = format!("{}_PG_ADDRESS", prefix);

        // Check if any environment variables are set
        let has_env_vars = env::var(&db_name_var).is_ok() 
            || env::var(&user_var).is_ok() 
            || env::var(&pass_var).is_ok() 
            || env::var(&address_var).is_ok();

        if require_real_db && !has_env_vars {
            return Err(TestDbError::MissingRequiredVar(db_name_var));
        }

        // Use environment variables if set, otherwise use test defaults
        let config = if has_env_vars {
            // If any env var is set, require all of them for consistency
            let host = env::var(&address_var)
                .map_err(|_| TestDbError::MissingRequiredVar(address_var.clone()))?;
            DatabaseConfig {
                db_name: env::var(&db_name_var)
                    .map_err(|_| TestDbError::MissingRequiredVar(db_name_var.clone()))?,
                username: env::var(&user_var)
                    .map_err(|_| TestDbError::MissingRequiredVar(user_var.clone()))?,
                password: env::var(&pass_var)
                    .map_err(|_| TestDbError::MissingRequiredVar(pass_var.clone()))?,
                host: host.clone(),
                address: host,  // For backward compatibility
                port: Some(5432),
                pool_size: Some(5),
                connection_timeout: Some(Duration::from_secs(5)),
                idle_timeout: Some(Duration::from_secs(60)),
                max_lifetime: Some(Duration::from_secs(180)),
                use_ssl: true,
            }
        } else {
            // Use test defaults for local testing
            DatabaseConfig {
                db_name: format!("test_{}_db", prefix.to_lowercase()),
                username: "postgres".to_string(),
                password: "password".to_string(),
                host: "localhost".to_string(),
                address: "localhost".to_string(),  // For backward compatibility
                port: Some(5432),
                pool_size: Some(5),
                connection_timeout: Some(Duration::from_secs(5)),
                idle_timeout: Some(Duration::from_secs(60)),
                max_lifetime: Some(Duration::from_secs(180)),
                use_ssl: false, // Local test databases usually don't need SSL
            }
        };

        Ok(config)
    }

    /// Get test database configuration with custom pool settings
    pub fn get_test_db_config_with_pool_settings(
        prefix: &str,
        pool_size: usize,
        connection_timeout_secs: u64,
        require_real_db: bool,
    ) -> Result<DatabaseConfig, TestDbError> {
        let mut config = get_test_db_config(prefix, require_real_db)?;
        config.pool_size = Some(pool_size);
        config.connection_timeout = Some(Duration::from_secs(connection_timeout_secs));
        Ok(config)
    }

    /// Check if database environment variables are available for testing
    pub fn has_test_db_env(prefix: &str) -> bool {
        let db_name_var = format!("{}_PG_DBNAME", prefix);
        env::var(db_name_var).is_ok()
    }

    /// Skip test if database is not available
    /// Returns true if test should be skipped
    pub fn should_skip_db_test(prefix: &str) -> bool {
        if !has_test_db_env(prefix) {
            println!(
                "Skipping test: Database environment variables for {} not set. \
                To run this test, set {}_PG_DBNAME, {}_PG_USER, {}_PG_PASS, and {}_PG_ADDRESS \
                or run with a local test database.",
                prefix, prefix, prefix, prefix, prefix
            );
            true
        } else {
            false
        }
    }
}