use std::env;
use std::fmt;

#[derive(Debug)]
pub enum ConfigError {
    Missing(String),
    Invalid(String),
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ConfigError::Missing(var) => write!(f, "Required environment variable {} is not set", var),
            ConfigError::Invalid(msg) => write!(f, "Invalid configuration: {}", msg),
        }
    }
}

impl std::error::Error for ConfigError {}

#[derive(Debug, Clone)]
pub struct DatabaseConfig {
    pub db_name: String,
    pub username: String,
    pub password: String,
    pub address: String,
}

impl DatabaseConfig {
    pub fn homebrew_from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            db_name: env::var("HOMEBREW_PG_DBNAME")
                .map_err(|_| ConfigError::Missing("HOMEBREW_PG_DBNAME".to_string()))?,
            username: env::var("HOMEBREW_PG_USER")
                .map_err(|_| ConfigError::Missing("HOMEBREW_PG_USER".to_string()))?,
            password: env::var("HOMEBREW_PG_PASS")
                .map_err(|_| ConfigError::Missing("HOMEBREW_PG_PASS".to_string()))?,
            address: env::var("HOMEBREW_PG_ADDRESS")
                .unwrap_or_else(|_| "localhost:5432".to_string()),
        })
    }
    
    pub fn combo_from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            db_name: env::var("COMBO_PG_DBNAME")
                .map_err(|_| ConfigError::Missing("COMBO_PG_DBNAME".to_string()))?,
            username: env::var("COMBO_PG_USER")
                .map_err(|_| ConfigError::Missing("COMBO_PG_USER".to_string()))?,
            password: env::var("COMBO_PG_PASS")
                .map_err(|_| ConfigError::Missing("COMBO_PG_PASS".to_string()))?,
            address: env::var("COMBO_PG_ADDRESS")
                .unwrap_or_else(|_| "localhost:5432".to_string()),
        })
    }
}

#[derive(Debug, Clone)]
pub struct WeatherConfig {
    pub accu_key: String,
    pub zip_code: String,
}

impl WeatherConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        Ok(Self {
            accu_key: env::var("ACCUWEATHERKEY")
                .map_err(|_| ConfigError::Missing("ACCUWEATHERKEY".to_string()))?,
            zip_code: env::var("ZIP_CODE")
                .map_err(|_| ConfigError::Missing("ZIP_CODE".to_string()))?,
        })
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub homebrew_database: Option<DatabaseConfig>,
    pub combo_database: Option<DatabaseConfig>,
    pub weather: WeatherConfig,
}

impl Config {
    pub fn from_env() -> Result<Self, ConfigError> {
        load_env_file();
        
        // Try to load both database configs, but allow them to be optional
        let homebrew_database = DatabaseConfig::homebrew_from_env().ok();
        let combo_database = DatabaseConfig::combo_from_env().ok();
        
        Ok(Self {
            homebrew_database,
            combo_database,
            weather: WeatherConfig::from_env()?,
        })
    }
    
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Validate homebrew database if present
        if let Some(db) = &self.homebrew_database {
            if db.address.is_empty() {
                return Err(ConfigError::Invalid("Homebrew database address cannot be empty".to_string()));
            }
        }
        
        // Validate combo database if present
        if let Some(db) = &self.combo_database {
            if db.address.is_empty() {
                return Err(ConfigError::Invalid("Combo database address cannot be empty".to_string()));
            }
        }
        
        // Validate ZIP code format (basic US ZIP code validation)
        if self.weather.zip_code.len() != 5 || !self.weather.zip_code.chars().all(|c| c.is_numeric()) {
            return Err(ConfigError::Invalid("ZIP_CODE must be a 5-digit US ZIP code".to_string()));
        }
        
        // Validate API key is not empty
        if self.weather.accu_key.is_empty() {
            return Err(ConfigError::Invalid("ACCUWEATHERKEY cannot be empty".to_string()));
        }
        
        Ok(())
    }
}

fn load_env_file() {
    // Try to load .env file if it exists
    if let Ok(contents) = std::fs::read_to_string(".env") {
        for line in contents.lines() {
            if line.starts_with('#') || line.is_empty() {
                continue;
            }
            
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim().trim_matches('"').trim_matches('\'');
                
                // Only set if not already set (environment variables take precedence)
                if env::var(key).is_err() {
                    env::set_var(key, value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_validation() {
        let config = Config {
            homebrew_database: Some(DatabaseConfig {
                db_name: "test".to_string(),
                username: "user".to_string(),
                password: "pass".to_string(),
                address: "localhost:5432".to_string(),
            }),
            combo_database: None,
            weather: WeatherConfig {
                accu_key: "test_key".to_string(),
                zip_code: "12345".to_string(),
            },
        };
        
        assert!(config.validate().is_ok());
    }
    
    #[test]
    fn test_invalid_zip_code() {
        let config = Config {
            homebrew_database: Some(DatabaseConfig {
                db_name: "test".to_string(),
                username: "user".to_string(),
                password: "pass".to_string(),
                address: "localhost:5432".to_string(),
            }),
            combo_database: None,
            weather: WeatherConfig {
                accu_key: "test_key".to_string(),
                zip_code: "123".to_string(), // Invalid ZIP
            },
        };
        
        assert!(config.validate().is_err());
    }
    
    #[test]
    fn test_empty_api_key() {
        let config = Config {
            homebrew_database: None,
            combo_database: Some(DatabaseConfig {
                db_name: "test".to_string(),
                username: "user".to_string(),
                password: "pass".to_string(),
                address: "localhost:5432".to_string(),
            }),
            weather: WeatherConfig {
                accu_key: "".to_string(), // Empty API key
                zip_code: "12345".to_string(),
            },
        };
        
        assert!(config.validate().is_err());
    }
}