use serde_json::json;

use serde::{Serialize, Deserialize};
use std::convert::TryInto;
use std::env;
use std::thread;
use rand::{thread_rng, Rng};
use rand::distributions::Alphanumeric;
use rouille::Request;
use rouille::Response;
use rouille::post_input;
use rouille::session;
use rouille::try_or_400;
use std::time::{SystemTime, UNIX_EPOCH};
use crate::auth::{validate_auth_header, RateLimiter};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use tokio::sync::broadcast;

use tokio_postgres::{Error, Row};
use crate::error::{JupiterError, Result as JupiterResult};
use crate::ssl_config::{create_homebrew_connector, SslConfig};
use crate::input_sanitizer::{InputSanitizer, DatabaseInputValidator, ValidationError};
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres_openssl::MakeTlsConnector;
use crate::db_pool::{DatabasePool, DatabaseConfig, init_homebrew_pool, get_homebrew_pool};
use crate::config::{ConfigError};

// Can have multiple homebrew instruments
// Support temperature humidity, windspeed, wind direction, percipitation, PM2.5, PM10, C02, TVOC, etc.
// Must select storage location for homebrew instruments (local, postgres, etc.)
// Multiple instruments can form an inside/outside average
// Instrument can be inside or outside
// Instruments POST to homebrew API using an API key





// Secure filter parameters for database queries
#[derive(Debug, Clone)]
pub struct FilterParams {
    pub oid: Option<String>,
    // Add more filter fields as needed
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub apikey: String,
    pub pg: PostgresServer,
    pub port: u16,
    #[serde(skip)]
    pub server_handle: Option<Arc<std::sync::Mutex<Option<JoinHandle<()>>>>>,
    #[serde(skip)]
    pub shutdown_flag: Arc<AtomicBool>,
    #[serde(skip)]
    pub shutdown_tx: Option<broadcast::Sender<()>>
}
impl std::fmt::Debug for Config {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Config")
            .field("apikey", &self.apikey)
            .field("pg", &self.pg)
            .field("port", &self.port)
            .finish()
    }
}

impl Config {
    pub fn new(apikey: String, pg: PostgresServer, port: u16) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Config {
            apikey,
            pg,
            port,
            server_handle: Some(Arc::new(std::sync::Mutex::new(None))),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            shutdown_tx: Some(shutdown_tx),
        }
    }

    pub async fn init(&mut self) -> JupiterResult<()> {
        // Initialize connection pool
        let db_config = DatabaseConfig {
            db_name: self.pg.db_name.clone(),
            username: self.pg.username.clone(),
            password: self.pg.password.clone(),
            host: self.pg.address.clone(),
            port: Some(5432),
            pool_size: Some(20),
            connection_timeout: Some(std::time::Duration::from_secs(5)),
            idle_timeout: Some(std::time::Duration::from_secs(600)),
            max_lifetime: Some(std::time::Duration::from_secs(1800)),
            use_ssl: true,
        };
        
        match init_homebrew_pool(db_config).await {
            Ok(pool) => {
                log::info!("[homebrew] Database connection pool initialized successfully");
                // Log initial pool status
                let status = pool.status();
                status.log("homebrew");
            },
            Err(e) => {
                log::error!("[homebrew] Failed to initialize database connection pool: {}", e);
                return Err(JupiterError::DatabaseError(format!("Unable to initialize database connection pool: {}", e)));
            }
        }

        self.build_tables().await?;

        let config = self.clone();
        let shutdown_flag = self.shutdown_flag.clone();
        let _shutdown_rx = self.shutdown_tx.as_ref()
            .ok_or_else(|| JupiterError::ConfigurationError("Shutdown channel not initialized".into()))?
            .subscribe();
        let server_port = config.port;
        
        let handle = thread::spawn(move || {
            // Create rate limiter: max 10 attempts per minute per IP
            let rate_limiter = Arc::new(RateLimiter::new(10, 60));
            
            let server = rouille::Server::new(format!("0.0.0.0:{}", server_port).as_str(), move |request| {
    
                // Validate authentication with rate limiting
                if let Err(response) = validate_auth_header(request, &config.apikey, Some(&rate_limiter)) {
                    return response;
                }
    
                if request.url() == "/api/weather_reports" {
                    if request.method() == "POST" {

                        // Collect input params from post request
                        let input = try_or_400!(post_input!(request, {
                            temperature: Option<f64>,
                            humidity: Option<f64>,
                            percipitation: Option<f64>,
                            pm10: Option<f64>,
                            pm25: Option<f64>,
                            co2: Option<f64>,
                            tvoc: Option<f64>,
                            device_type: String,
                        }));

                        let mut obj = WeatherReport::new();
                        obj.temperature = input.temperature;
                        obj.humidity = input.humidity;
                        obj.percipitation = input.percipitation;
                        obj.pm10 = input.pm10;
                        obj.pm25 = input.pm25;
                        obj.co2 = input.co2;
                        obj.tvoc = input.tvoc;
                        obj.device_type = input.device_type.to_string();
                        obj.save(config.clone());
                        return Response::json(&obj);
                    }
                    if request.method() == "GET" {
                        let objects = match WeatherReport::select(config.clone(), Some(1), None, Some(format!("timestamp DESC")), None) {
                            Ok(objs) => objs,
                            Err(e) => {
                                log::error!("Failed to select weather reports: {}", e);
                                return Response::text("Database error").with_status_code(500);
                            }
                        };
                        
                        // Check if we have any results before accessing
                        if let Some(first) = objects.first() {
                            return Response::json(&first.clone());
                        } else {
                            // Log empty result scenario
                            eprintln!("[homebrew] Warning: No weather data found in database for GET request");
                            // Return a proper error response when no data is available
                            return Response::text("No weather data available").with_status_code(404);
                        }
                    }
                }
    
    
                let mut response = Response::text("hello world");

                return response;
            }).unwrap_or_else(|e| {
                log::error!("Failed to create server: {}", e);
                panic!("Failed to create server: {}", e);
            });
            
            log::info!("Homebrew server started on port {}", server_port);
            
            // Run server with shutdown support
            while !shutdown_flag.load(Ordering::Relaxed) {
                server.poll_timeout(std::time::Duration::from_millis(100));
            }
            
            log::info!("Homebrew server shutting down...");
        });
        
        if let Some(handle_mutex) = &self.server_handle {
            let mut handle_guard = handle_mutex.lock()
                .map_err(|e| JupiterError::LockError(format!("Failed to acquire server handle lock: {}", e)))?;
            *handle_guard = Some(handle);
        }
        
        Ok(())
    }

    pub async fn shutdown(&mut self) {
        self.shutdown_with_timeout(std::time::Duration::from_secs(10)).await;
    }

    pub async fn shutdown_with_timeout(&mut self, timeout: std::time::Duration) {
        log::info!("Initiating graceful shutdown of homebrew server...");
        
        // Signal the server thread to stop
        self.shutdown_flag.store(true, Ordering::Relaxed);
        
        // Send shutdown signal via broadcast channel
        if let Some(tx) = &self.shutdown_tx {
            let _ = tx.send(());
        }
        
        // Wait for the server thread to finish with timeout
        if let Some(handle_mutex) = &self.server_handle {
            let handle_mutex_clone = handle_mutex.clone();
            
            // Try to join with timeout
            let join_result = tokio::time::timeout(timeout, async move {
                if let Ok(mut handle_guard) = handle_mutex_clone.lock() {
                    if let Some(handle) = handle_guard.take() {
                        // Since we can't directly join std::thread in async context,
                        // we'll use a different approach
                        let _ = tokio::task::spawn_blocking(move || {
                            handle.join()
                        }).await;
                    }
                }
            }).await;
            
            match join_result {
                Ok(_) => log::info!("Homebrew server thread joined successfully"),
                Err(_) => {
                    log::warn!("Homebrew server shutdown timed out after {:?}", timeout);
                    // Force cleanup if needed
                    if let Ok(mut handle_guard) = handle_mutex.lock() {
                        handle_guard.take(); // Drop the handle
                    }
                }
            }
        }
        
        log::info!("Homebrew server shutdown complete");
    }

    pub async fn build_tables(&self) -> JupiterResult<()> {
        // Get connection from pool
        let pool = get_homebrew_pool()
            .ok_or_else(|| JupiterError::DatabaseError("Database pool not initialized".to_string()))?;
        
        let client = pool.get_connection_with_retry(3).await
            .map_err(|e| JupiterError::DatabaseError(format!("Failed to get database connection: {}", e)))?;
    
        // Build WeatherReport Table
        // ---------------------------------------------------------------
        let db = client.batch_execute(WeatherReport::sql_build_statement()).await;
        match db {
            Ok(_v) => log::info!("POSTGRES: CREATED WeatherReport Table"),
            Err(e) => log::error!("POSTGRES: {:?}", e),
        }
        let db_migrations = WeatherReport::migrations();
        for migration in db_migrations {
            let migrations_db = client.batch_execute(migration).await;
            match migrations_db {
                Ok(_v) => log::info!("POSTGRES: Migration Successful"),
                Err(e) => log::error!("POSTGRES: {:?}", e),
            }
        }

        return Ok(());
    }    

}

// Stored in SQL in cache_timeout is set
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct WeatherReport {
    pub id: i32,
    pub oid: String,
    pub temperature: Option<f64>, // Stored in celcius....api converts to F/C
    pub humidity: Option<f64>,
    pub percipitation: Option<f64>,
    pub pm10: Option<f64>,
    pub pm25: Option<f64>,
    pub co2: Option<f64>,
    pub tvoc: Option<f64>,
    pub device_type: String, // indoor, outdoor, other
    pub timestamp: i64
}
impl WeatherReport {
    pub fn new() -> WeatherReport {
        let oid: String = thread_rng().sample_iter(&Alphanumeric).take(15).map(char::from).collect();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap_or_else(|e| {
                log::error!("System time error: {}", e);
                std::time::Duration::from_secs(0)
            })
            .as_secs() as i64;

        WeatherReport { 
            id: 0,
            oid: oid,
            temperature: None,
            humidity: None,
            percipitation: None,
            pm10: None,
            pm25: None,
            co2: None,
            tvoc: None,
            device_type: String::from("other"),
            timestamp: timestamp
        }
    }
    pub fn sql_table_name() -> String {
        return format!("weather_reports")
    }
    pub fn sql_build_statement() -> &'static str {
        "CREATE TABLE public.weather_reports (
            id serial NOT NULL,
            oid varchar NOT NULL UNIQUE,
            temperature DOUBLE PRECISION NULL,
            humidity DOUBLE PRECISION NULL,
            percipitation DOUBLE PRECISION NULL,
            pm10 DOUBLE PRECISION NULL,
            pm25 DOUBLE PRECISION NULL,
            co2 DOUBLE PRECISION NULL,
            tvoc DOUBLE PRECISION NULL,
            device_type VARCHAR NULL,
            timestamp BIGINT DEFAULT 0,
            CONSTRAINT weather_reports_pkey PRIMARY KEY (id));"
    }
    pub fn migrations() -> Vec<&'static str> {
        vec![
            "",
        ]
    }
    pub fn save(&self, config: Config) -> JupiterResult<&Self> {
        // Use async runtime to get connection from pool
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| {
                log::error!("Failed to create tokio runtime: {}", e);
                JupiterError::RuntimeError(format!("Failed to create runtime: {}", e))
            })?;
        
        let client = runtime.block_on(async {
            let pool = get_homebrew_pool()
                .ok_or_else(|| JupiterError::DatabaseError("Database pool not initialized".into()))?;
            
            pool.get_connection_with_retry(3).await
                .map_err(|e| {
                    log::error!("Failed to get database connection: {}", e);
                    JupiterError::DatabaseError(format!("Connection pool exhausted: {}", e))
                })
        })?;

        // Search for OID matches using secure parameterized query
        let rows = Self::select_by_oid(
            config.clone(),
            &self.oid
        )?;

        if rows.len() == 0 {
            runtime.block_on(client.execute("INSERT INTO weather_reports (oid, device_type, timestamp) VALUES ($1, $2, $3)",
                &[&self.oid.clone(),
                &self.device_type,
                &self.timestamp]
            ))?;
        } 

        if self.temperature.is_some() {
            runtime.block_on(client.execute("UPDATE weather_reports SET temperature = $1 WHERE oid = $2;", 
            &[
                &self.temperature,
                &self.oid
            ]))?;
        }

        if self.humidity.is_some() {
            runtime.block_on(client.execute("UPDATE weather_reports SET humidity = $1 WHERE oid = $2;", 
            &[
                &self.humidity,
                &self.oid
            ]))?;
        }

        if self.percipitation.is_some() {
            runtime.block_on(client.execute("UPDATE weather_reports SET percipitation = $1 WHERE oid = $2;", 
            &[
                &self.percipitation,
                &self.oid
            ]))?;
        }

        if self.pm10.is_some() {
            runtime.block_on(client.execute("UPDATE weather_reports SET pm10 = $1 WHERE oid = $2;", 
            &[
                &self.pm10,
                &self.oid
            ]))?;
        }

        if self.pm25.is_some() {
            runtime.block_on(client.execute("UPDATE weather_reports SET pm25 = $1 WHERE oid = $2;", 
            &[
                &self.pm25,
                &self.oid
            ]))?;
        }

        if self.co2.is_some() {
            runtime.block_on(client.execute("UPDATE weather_reports SET co2 = $1 WHERE oid = $2;", 
            &[
                &self.co2,
                &self.oid
            ]))?;
        }

        if self.tvoc.is_some() {
            runtime.block_on(client.execute("UPDATE weather_reports SET tvoc = $1 WHERE oid = $2;", 
            &[
                &self.tvoc,
                &self.oid
            ]))?;
        }

        return Ok(self);
    }
    // Secure method to select by OID using parameterized query
    pub fn select_by_oid(config: Config, oid: &str) -> JupiterResult<Vec<Self>> {
        // Validate OID input before using in query
        if !InputSanitizer::validate_oid(oid) {
            log::error!("Invalid OID format detected: {}", oid);
        }
        
        if !InputSanitizer::check_for_sql_keywords(oid) {
            log::error!("Potential SQL injection detected in OID: {}", oid);
        }
        
        // Use async runtime to get connection from pool
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| JupiterError::DatabaseError(format!("Failed to create runtime: {}", e)))?;
        runtime.block_on(async {
            let pool = get_homebrew_pool()
                .ok_or_else(|| JupiterError::DatabaseError("Database pool not initialized".to_string()))?;
            
            let client = pool.get_connection_with_retry(3).await
                .map_err(|e| JupiterError::DatabaseError(format!("Failed to get database connection: {}", e)))?;
            
            let query = "SELECT * FROM weather_reports WHERE oid = $1 ORDER BY id DESC";
            let rows = client.query(query, &[&oid]).await
                .map_err(|e| JupiterError::DatabaseError(format!("Query failed: {}", e)))?;
            
            let mut parsed_rows: Vec<Self> = Vec::new();
            for row in rows {
                parsed_rows.push(Self::from_row(&row)
                    .map_err(|e| JupiterError::DatabaseError(format!("Failed to parse row: {}", e)))?);
            }
            
            Ok(parsed_rows)
        })
    }
    
    // Secure select method with parameterized queries
    pub fn select(config: Config, limit: Option<usize>, offset: Option<usize>, order_column: Option<String>, filter_params: Option<FilterParams>) -> JupiterResult<Vec<Self>> {
        // Build secure query with parameterized placeholders
        let mut query = String::from("SELECT * FROM weather_reports");
        let mut param_count = 0;
        
        // Add WHERE clause if filter parameters provided
        if let Some(ref filters) = filter_params {
            if let Some(ref oid) = filters.oid {
                param_count += 1;
                query.push_str(&format!(" WHERE oid = ${}", param_count));
            }
        }
        
        // Add ORDER BY clause (validate column name against whitelist)
        let valid_order_columns = vec!["id", "timestamp", "temperature", "humidity", "oid"];
        match order_column {
            Some(col) if valid_order_columns.contains(&col.as_str()) => {
                query.push_str(&format!(" ORDER BY {} DESC", col));
            },
            _ => {
                query.push_str(" ORDER BY id DESC");
            }
        }
        
        // Add LIMIT and OFFSET
        if let Some(limit_val) = limit {
            query.push_str(&format!(" LIMIT {}", limit_val));
        }
        if let Some(offset_val) = offset {
            query.push_str(&format!(" OFFSET {}", offset_val));
        }
        
        // Use async runtime to get connection from pool
        let runtime = tokio::runtime::Runtime::new()
            .map_err(|e| JupiterError::DatabaseError(format!("Failed to create runtime: {}", e)))?;
        runtime.block_on(async {
            let pool = get_homebrew_pool()
                .ok_or_else(|| JupiterError::DatabaseError("Database pool not initialized".to_string()))?;
            
            let client = pool.get_connection_with_retry(3).await
                .map_err(|e| JupiterError::DatabaseError(format!("Failed to get database connection: {}", e)))?;
            
            // Execute query with appropriate parameters
            let rows = if let Some(ref filters) = filter_params {
                if let Some(ref oid) = filters.oid {
                    client.query(&query, &[oid]).await
                        .map_err(|e| JupiterError::DatabaseError(format!("Query failed: {}", e)))?
                } else {
                    client.query(&query, &[]).await
                        .map_err(|e| JupiterError::DatabaseError(format!("Query failed: {}", e)))?
                }
            } else {
                client.query(&query, &[]).await
                    .map_err(|e| JupiterError::DatabaseError(format!("Query failed: {}", e)))?
            };
            
            let mut parsed_rows: Vec<Self> = Vec::new();
            for row in rows {
                parsed_rows.push(Self::from_row(&row)
                    .map_err(|e| JupiterError::DatabaseError(format!("Failed to parse row: {}", e)))?);
            }

            Ok(parsed_rows)
        })
    }
    fn from_row(row: &Row) -> JupiterResult<Self> {
        return Ok(Self {
            id: row.get("id"),
            oid: row.get("oid"),
            temperature: row.get("temperature"),
            humidity: row.get("humidity"),
            percipitation: row.get("percipitation"),
            pm10: row.get("pm10"),
            pm25: row.get("pm25"),
            co2: row.get("co2"),
            tvoc: row.get("tvoc"),
            device_type: row.get("device_type"),
            timestamp: row.get("timestamp"),
        });
    }
}


#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostgresServer {
	pub db_name: String,
    pub username: String,
    pub password: String,
	pub address: String
}
impl PostgresServer {
    pub fn new() -> Result<PostgresServer, ConfigError> {
        let config = DatabaseConfig::homebrew_from_env()?;
        
        Ok(PostgresServer {
            db_name: config.db_name,
            username: config.username,
            password: config.password,
            address: config.address,
        })
    }
    
    pub fn from_config(config: &DatabaseConfig) -> PostgresServer {
        PostgresServer {
            db_name: config.db_name.clone(),
            username: config.username.clone(),
            password: config.password.clone(),
            address: config.address.clone(),
        }

    }
}