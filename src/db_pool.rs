use deadpool_postgres::{Config, ManagerConfig, Pool, RecyclingMethod, Runtime};
use tokio_postgres::NoTls;
use std::time::Duration;
use log::{info, error, warn};

use crate::ssl_config::{create_homebrew_connector, create_combo_connector};

#[derive(Clone)]
pub struct DatabasePool {
    pool: Pool,
    name: String,
}

#[derive(Clone, Debug)]
pub struct DatabaseConfig {
    pub db_name: String,
    pub username: String,
    pub password: String,
    pub host: String,
    pub port: Option<u16>,
    pub pool_size: Option<usize>,
    pub connection_timeout: Option<Duration>,
    pub idle_timeout: Option<Duration>,
    pub max_lifetime: Option<Duration>,
    pub use_ssl: bool,
}

impl DatabasePool {
    pub async fn new_homebrew(config: DatabaseConfig) -> Result<Self, String> {
        let connector = create_homebrew_connector()
            .map_err(|e| format!("Failed to create homebrew connector: {}", e))?;
        Self::create_pool("homebrew", config, connector).await
    }

    pub async fn new_combo(config: DatabaseConfig) -> Result<Self, String> {
        let connector = create_combo_connector()
            .map_err(|e| format!("Failed to create combo connector: {}", e))?;
        Self::create_pool("combo", config, connector).await
    }

    async fn create_pool<T>(
        name: &str,
        config: DatabaseConfig,
        tls: T,
    ) -> Result<Self, String>
    where
        T: tokio_postgres::tls::MakeTlsConnect<tokio_postgres::Socket> + Clone + Send + Sync + 'static,
        <T as tokio_postgres::tls::MakeTlsConnect<tokio_postgres::Socket>>::Stream: Send + Sync,
        <T as tokio_postgres::tls::MakeTlsConnect<tokio_postgres::Socket>>::TlsConnect: Send + Sync,
        <<T as tokio_postgres::tls::MakeTlsConnect<tokio_postgres::Socket>>::TlsConnect as tokio_postgres::tls::TlsConnect<tokio_postgres::Socket>>::Future: Send,
    {
        let mut cfg = Config::new();
        cfg.dbname = Some(config.db_name.clone());
        cfg.user = Some(config.username.clone());
        cfg.password = Some(config.password.clone());
        cfg.host = Some(config.host.clone());
        cfg.port = config.port;
        
        // Configure pool settings
        cfg.pool = Some(deadpool_postgres::PoolConfig {
            max_size: config.pool_size.unwrap_or(10),
            timeouts: deadpool_postgres::Timeouts {
                wait: config.connection_timeout,
                create: config.connection_timeout,
                recycle: config.connection_timeout,
            },
        });

        // Configure manager settings
        cfg.manager = Some(ManagerConfig {
            recycling_method: RecyclingMethod::Fast,
        });

        // Create the pool
        let pool = cfg.create_pool(Some(Runtime::Tokio1), tls)
            .map_err(|e| format!("Failed to create pool: {}", e))?;
        
        // Test the connection
        info!("[{}] Testing database connection...", name);
        let client = pool.get().await
            .map_err(|e| format!("Failed to get test connection: {}", e))?;
        let row = client.query_one("SELECT 1 as test", &[]).await
            .map_err(|e| format!("Failed to execute test query: {}", e))?;
        let test_result: i32 = row.get("test");
        if test_result != 1 {
            return Err("Database connection test failed".to_string());
        }
        info!("[{}] Database connection test successful", name);
        
        Ok(Self {
            pool,
            name: name.to_string(),
        })
    }

    pub async fn get_connection(&self) -> Result<deadpool_postgres::Client, String> {
        match self.pool.get().await {
            Ok(client) => {
                // Perform a health check
                match tokio::time::timeout(Duration::from_secs(1), client.query_one("SELECT 1", &[])).await {
                    Ok(Ok(_)) => Ok(client),
                    Ok(Err(e)) => {
                        error!("[{}] Connection health check failed: {}", self.name, e);
                        Err(format!("Connection health check failed: {}", e).into())
                    }
                    Err(_) => {
                        error!("[{}] Connection health check timed out", self.name);
                        Err("Connection health check timed out".into())
                    }
                }
            }
            Err(e) => {
                error!("[{}] Failed to get connection from pool: {}", self.name, e);
                Err(format!("Failed to get connection from pool: {}", e).into())
            }
        }
    }

    pub async fn get_connection_with_retry(&self, max_retries: u32) -> Result<deadpool_postgres::Client, String> {
        let mut retries = 0;
        let mut last_error = None;

        while retries < max_retries {
            match self.get_connection().await {
                Ok(client) => return Ok(client),
                Err(e) => {
                    warn!("[{}] Connection attempt {} failed: {}", self.name, retries + 1, e);
                    last_error = Some(e);
                    retries += 1;
                    
                    if retries < max_retries {
                        // Exponential backoff
                        let delay = Duration::from_millis(100 * 2_u64.pow(retries));
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or_else(|| "All connection attempts failed".to_string()))
    }

    pub fn status(&self) -> PoolStatus {
        let status = self.pool.status();
        PoolStatus {
            size: status.size as usize,
            available: status.available.max(0) as usize,
            waiting: 0, // deadpool 0.9 doesn't have waiting field
        }
    }

    pub async fn close(self) {
        info!("[{}] Closing database connection pool...", self.name);
        self.pool.close();
        // Wait for all connections to be closed
        tokio::time::sleep(Duration::from_millis(100)).await;
        info!("[{}] Database connection pool closed", self.name);
    }
}

#[derive(Debug, Clone)]
pub struct PoolStatus {
    pub size: usize,
    pub available: usize,
    pub waiting: usize,
}

impl PoolStatus {
    pub fn log(&self, pool_name: &str) {
        info!(
            "[{}] Pool status - Size: {}, Available: {}, Waiting: {}",
            pool_name, self.size, self.available, self.waiting
        );
    }
}

// Global pool managers for singleton pattern
use std::sync::Arc;
use tokio::sync::{OnceCell, Mutex};
use once_cell::sync::Lazy;

static HOMEBREW_POOL: Lazy<OnceCell<Arc<DatabasePool>>> = Lazy::new(|| OnceCell::new());
static COMBO_POOL: Lazy<OnceCell<Arc<DatabasePool>>> = Lazy::new(|| OnceCell::new());

pub async fn init_homebrew_pool(config: DatabaseConfig) -> Result<Arc<DatabasePool>, String> {
    HOMEBREW_POOL.get_or_try_init(|| async {
        let pool = DatabasePool::new_homebrew(config).await?;
        Ok::<Arc<DatabasePool>, String>(Arc::new(pool))
    }).await.map(|pool| Arc::clone(pool))
}

pub async fn init_combo_pool(config: DatabaseConfig) -> Result<Arc<DatabasePool>, String> {
    COMBO_POOL.get_or_try_init(|| async {
        let pool = DatabasePool::new_combo(config).await?;
        Ok::<Arc<DatabasePool>, String>(Arc::new(pool))
    }).await.map(|pool| Arc::clone(pool))
}

pub fn get_homebrew_pool() -> Option<Arc<DatabasePool>> {
    HOMEBREW_POOL.get().map(|pool| Arc::clone(pool))
}

pub fn get_combo_pool() -> Option<Arc<DatabasePool>> {
    COMBO_POOL.get().map(|pool| Arc::clone(pool))
}

// Cleanup function for graceful shutdown
pub async fn shutdown_pools() {
    info!("Shutting down database connection pools...");
    
    if let Some(pool) = HOMEBREW_POOL.get() {
        if let Ok(pool) = Arc::try_unwrap(Arc::clone(pool)) {
            pool.close().await;
        }
    }
    
    if let Some(pool) = COMBO_POOL.get() {
        if let Ok(pool) = Arc::try_unwrap(Arc::clone(pool)) {
            pool.close().await;
        }
    }
    
    info!("All database connection pools shut down");
}