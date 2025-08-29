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
use crate::ssl_config::{create_combo_connector, SslConfig};
use crate::input_sanitizer::{InputSanitizer, DatabaseInputValidator, ValidationError};
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres_openssl::MakeTlsConnector;
use crate::config::{DatabaseConfig, ConfigError};

// Ability to combine, average, and cache final values between all configured providers.

// Secure filter parameters for database queries
#[derive(Debug, Clone)]
pub struct FilterParams {
    pub oid: Option<String>,
    // Add more filter fields as needed
}

// Lives in memory, no SQL
#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub accu_config: Option<crate::provider::accuweather::Config>,
    pub homebrew_config: Option<crate::provider::homebrew::Config>,
    pub apikey: String,
    pub cache_timeout: Option<i64>,
    pub pg: PostgresServer,
    pub port: u16,
    pub zip_code: String,
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
            .field("cache_timeout", &self.cache_timeout)
            .field("pg", &self.pg)
            .field("port", &self.port)
            .field("zip_code", &self.zip_code)
            .finish()
    }
}

impl Config {
    pub fn new(accu_config: Option<crate::provider::accuweather::Config>,
               homebrew_config: Option<crate::provider::homebrew::Config>,
               apikey: String,
               cache_timeout: Option<i64>,
               pg: PostgresServer,
               port: u16,
               zip_code: String) -> Self {
        let (shutdown_tx, _) = broadcast::channel(1);
        Config {
            accu_config,
            homebrew_config,
            apikey,
            cache_timeout,
            pg,
            port,
            zip_code,
            server_handle: Some(Arc::new(std::sync::Mutex::new(None))),
            shutdown_flag: Arc::new(AtomicBool::new(false)),
            shutdown_tx: Some(shutdown_tx),
        }
    }

    pub async fn init(&mut self) -> JupiterResult<()> {

        self.build_tables().await?;

        let config = self.clone();
        let shutdown_flag = self.shutdown_flag.clone();
        let _shutdown_rx = self.shutdown_tx.as_ref().unwrap().subscribe();
        let server_port = config.port;
        
        let handle = thread::spawn(move || {
            // Create rate limiter: max 10 attempts per minute per IP
            let rate_limiter = Arc::new(RateLimiter::new(10, 60));
            
            let server = rouille::Server::new(format!("0.0.0.0:{}", server_port).as_str(), move |request| {
    
                // Validate authentication with rate limiting
                if let Err(response) = validate_auth_header(request, &config.apikey, Some(&rate_limiter)) {
                    return response;
                }
    

                match config.homebrew_config.clone(){
                    Some(cfg) => {
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
        
                                let mut obj = crate::provider::homebrew::WeatherReport::new();
                                obj.temperature = input.temperature;
                                obj.humidity = input.humidity;
                                obj.percipitation = input.percipitation;
                                obj.pm10 = input.pm10;
                                obj.pm25 = input.pm25;
                                obj.co2 = input.co2;
                                obj.tvoc = input.tvoc;
                                obj.device_type = input.device_type.to_string();
                                obj.save(cfg.clone());
                                return Response::json(&obj);
                            }
                            if request.method() == "GET" {
                                let objects = match crate::provider::homebrew::WeatherReport::select(cfg.clone(), Some(1), None, Some(format!("timestamp DESC")), None) {
                                    Ok(objs) => objs,
                                    Err(e) => {
                                        log::error!("Failed to select homebrew weather reports: {}", e);
                                        return Response::text("Database error").with_status_code(500);
                                    }
                                };
                                
                                // Check if we have any results before accessing
                                if let Some(first) = objects.first() {
                                    return Response::json(&first.clone());
                                } else {
                                    eprintln!("[combo/homebrew] Warning: No weather data found in homebrew database");
                                    return Response::text("No homebrew weather data available").with_status_code(404);
                                }
                            }
                        }
                    },
                    None => {}
                }


  
                // Return a cached response if one exists within the timeout window
                // Otherwise check configured providers for current weather conditions and cache the results
                if request.method() == "GET" {

                    match config.cache_timeout.clone(){
                        Some(timeout) => {
                            let objects = match CachedWeatherData::select(config.clone(), Some(1), None, Some(format!("timestamp DESC")), None) {
                                Ok(objs) => objs,
                                Err(e) => {
                                    log::error!("Failed to select cached weather data: {}", e);
                                    // Continue without cache
                                    vec![]
                                }
                            };
                            
                            // Use safe array access with .first()
                            if let Some(first) = objects.first() {
                                let current_timestamp = match SystemTime::now().duration_since(UNIX_EPOCH) {
                                    Ok(duration) => duration.as_secs() as i64,
                                    Err(e) => {
                                        log::error!("System time error: {}", e);
                                        0i64
                                    }
                                };
                                let x = current_timestamp - first.timestamp;
                                if x < timeout {
                                    return Response::json(&first.clone());
                                }
                            } else {
                                eprintln!("[combo] Warning: No cached weather data found in database");
                            }
                        },
                        None => {}
                    }

                    let mut resp = CachedWeatherData::new();

                    match config.accu_config.clone(){
                        Some(cfg) => {
                            // Handle Option return from search_by_zip
                            match crate::provider::accuweather::Location::search_by_zip(cfg.clone(), config.zip_code.clone()) {
                                Ok(Some(location)) => {
                                    // Handle Option return from get
                                    match crate::provider::accuweather::CurrentCondition::get(cfg, location.clone()) {
                                        Ok(Some(current)) => {
                                            let j = match serde_json::to_string(&current) {
                                                Ok(json) => json,
                                                Err(e) => {
                                                    log::error!("Failed to serialize AccuWeather data: {}", e);
                                                    String::new()
                                                }
                                            };
                                            resp.accuweather = Some(j);
                                        },
                                        Ok(None) => {
                                            eprintln!("[combo] No current conditions available from AccuWeather");
                                        },
                                        Err(e) => {
                                            eprintln!("[combo] Error fetching current conditions from AccuWeather: {}", e);
                                        }
                                    }
                                },
                                Ok(None) => {
                                    eprintln!("[combo] No location found for zip code: {}", config.zip_code);
                                },
                                Err(e) => {
                                    eprintln!("[combo] Error searching location by zip: {}", e);
                                }
                            }
                        },
                        None => {}
                    }
         

                    match config.homebrew_config.clone(){
                        Some(cfg) => {
                            let objects = match crate::provider::homebrew::WeatherReport::select(cfg.clone(), Some(1), None, Some(format!("timestamp DESC")), None) {
                                Ok(objs) => objs,
                                Err(e) => {
                                    log::error!("Failed to select homebrew data for combo: {}", e);
                                    vec![]
                                }
                            };
                            
                            // Use safe array access to prevent panic on empty results
                            if let Some(first) = objects.first() {
                                let j = match serde_json::to_string(&first.clone()) {
                                    Ok(json) => json,
                                    Err(e) => {
                                        log::error!("Failed to serialize homebrew data: {}", e);
                                        String::new()
                                    }
                                };
                                resp.homebrew = Some(j);
                            } else {
                                eprintln!("[combo] Warning: No homebrew data available for caching");
                            }
                            // If no data, resp.homebrew remains None which is acceptable
                        },
                        None => {}
                    }

                    resp.save(config.clone());

                    // let objects = WeatherReport::select(config.clone(), None, None, None, None).unwrap();
                    return Response::json(&resp);
                }
                
    
    
                let mut response = Response::text("hello world");

                return response;
            }).expect("Failed to create server");
            
            log::info!("Combo server started on port {}", server_port);
            
            // Run server with shutdown support
            while !shutdown_flag.load(Ordering::Relaxed) {
                server.poll_timeout(std::time::Duration::from_millis(100));
            }
            
            log::info!("Combo server shutting down...");
        });
        
        if let Some(handle_mutex) = &self.server_handle {
            let mut handle_guard = handle_mutex.lock().unwrap();
            *handle_guard = Some(handle);
        }
        
        Ok(())
    }

    pub async fn shutdown(&mut self) {
        self.shutdown_with_timeout(std::time::Duration::from_secs(10)).await;
    }

    pub async fn shutdown_with_timeout(&mut self, timeout: std::time::Duration) {
        log::info!("Initiating graceful shutdown of combo server...");
        
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
                let mut handle_guard = handle_mutex_clone.lock().unwrap();
                if let Some(handle) = handle_guard.take() {
                    // Since we can't directly join std::thread in async context,
                    // we'll use a different approach
                    let _ = tokio::task::spawn_blocking(move || {
                        handle.join()
                    }).await;
                }
            }).await;
            
            match join_result {
                Ok(_) => log::info!("Combo server thread joined successfully"),
                Err(_) => {
                    log::warn!("Combo server shutdown timed out after {:?}", timeout);
                    // Force cleanup if needed
                    if let Ok(mut handle_guard) = handle_mutex.lock() {
                        handle_guard.take(); // Drop the handle
                    }
                }
            }
        }
        
        log::info!("Combo server shutdown complete");
    }

    pub async fn build_tables(&self) -> JupiterResult<()> {
    
        // Use centralized SSL configuration
        let connector = create_combo_connector()
            .map_err(|e| {
                log::error!("Failed to create SSL connector: {}", e);
                JupiterError::SslError(format!("Unable to create SSL connector: {}", e))
            })?;
    
        let (client, connection) = tokio_postgres::connect(format!("postgresql://{}:{}@{}/{}?sslmode=prefer", &self.pg.username, &self.pg.password, &self.pg.address, &self.pg.db_name).as_str(), connector).await?;
        
        // The connection object performs the actual communication with the database,
        // so spawn it off to run on its own.
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });
    
        // Build CachedWeatherData Table
        // ---------------------------------------------------------------
        let db = client.batch_execute(CachedWeatherData::sql_build_statement()).await;
        match db {
            Ok(_v) => log::info!("POSTGRES: CREATED CachedWeatherData Table"),
            Err(e) => log::error!("POSTGRES: {:?}", e),
        }
        let db_migrations = CachedWeatherData::migrations();
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
pub struct CachedWeatherData {
    pub id: i32,
    pub oid: String,
    pub accuweather: Option<String>, // JSON string
    pub homebrew: Option<String>, // JSON string
    pub openweathermap: Option<String>, // JSON string
    pub timestamp: i64
}
impl CachedWeatherData {
    pub fn new() -> CachedWeatherData {
        let oid: String = thread_rng().sample_iter(&Alphanumeric).take(15).map(char::from).collect();
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH)
            .unwrap_or_else(|e| {
                log::error!("System time error: {}", e);
                std::time::Duration::from_secs(0)
            })
            .as_secs() as i64;

        CachedWeatherData { 
            id: 0,
            oid: oid,
            accuweather: None,
            homebrew: None,
            openweathermap: None,
            timestamp: timestamp
        }
    }
    pub fn sql_table_name() -> String {
        return format!("cached_weather_data")
    }
    pub fn sql_build_statement() -> &'static str {
        "CREATE TABLE public.cached_weather_data (
            id serial NOT NULL,
            oid varchar NOT NULL UNIQUE,
            accuweather VARCHAR NULL,
            homebrew VARCHAR NULL,
            openweathermap VARCHAR NULL,
            timestamp BIGINT DEFAULT 0,
            CONSTRAINT cached_weather_data_pkey PRIMARY KEY (id));"
    }
    pub fn migrations() -> Vec<&'static str> {
        vec![
            "",
        ]
    }
    pub fn save(&self, config: Config) -> JupiterResult<&Self> {
        // Get a copy of the master key and postgres info
        let postgres = config.pg.clone();

        // Build SQL adapter with proper SSL verification
        let connector = create_combo_connector()
            .map_err(|e| {
                log::error!("Failed to create SSL connector: {}", e);
                JupiterError::SslError(format!("Unable to create SSL connector: {}", e))
            })?;

        // Build postgres client
        let mut client = crate::postgres::Client::connect(format!("postgresql://{}:{}@{}/{}?sslmode=prefer", &postgres.username, &postgres.password, &postgres.address, &postgres.db_name).as_str(), connector)?;

        // Search for OID matches using secure parameterized query
        let rows = Self::select_by_oid(
            config.clone(),
            &self.oid
        )?;

        if rows.len() == 0 {
            client.execute("INSERT INTO cached_weather_data (oid, timestamp) VALUES ($1, $2)",
                &[&self.oid.clone(),
                &self.timestamp]
            )?;
        } 

        if self.accuweather.is_some() {
            client.execute("UPDATE cached_weather_data SET accuweather = $1 WHERE oid = $2;", 
            &[
                &self.accuweather,
                &self.oid
            ])?;
        }

        if self.homebrew.is_some() {
            client.execute("UPDATE cached_weather_data SET homebrew = $1 WHERE oid = $2;", 
            &[
                &self.homebrew,
                &self.oid
            ])?;
        }

        if self.openweathermap.is_some() {
            client.execute("UPDATE cached_weather_data SET openweathermap = $1 WHERE oid = $2;", 
            &[
                &self.openweathermap,
                &self.oid
            ])?;
        }

        return Ok(self);
    }
    // Secure method to select by OID using parameterized query
    pub fn select_by_oid(config: Config, oid: &str) -> JupiterResult<Vec<Self>> {
        // Validate OID input before using in query
        if !InputSanitizer::validate_oid(oid) {
            // For postgres Error, we need to return a proper database error
            // Since we can't directly create a custom Error, we'll let the query fail safely
            // if invalid input gets through (which it won't with parameterized queries)
            log::error!("Invalid OID format detected: {}", oid);
        }
        
        if !InputSanitizer::check_for_sql_keywords(oid) {
            log::error!("Potential SQL injection detected in OID: {}", oid);
        }
        
        let postgres = config.pg.clone();
        
        let connector = create_combo_connector()
            .map_err(|e| {
                log::error!("Failed to create SSL connector: {}", e);
                JupiterError::SslError(format!("Unable to create SSL connector: {}", e))
            })?;
        let mut client = crate::postgres::Client::connect(
            format!("postgresql://{}:{}@{}/{}?sslmode=prefer", 
                &postgres.username, &postgres.password, &postgres.address, &postgres.db_name).as_str(), 
            connector
        )?;
        
        let query = "SELECT * FROM cached_weather_data WHERE oid = $1 ORDER BY id DESC";
        let mut parsed_rows: Vec<Self> = Vec::new();
        for row in client.query(query, &[&oid])? {
            parsed_rows.push(Self::from_row(&row)?);
        }
        
        Ok(parsed_rows)
    }
    
    // Secure select method with parameterized queries
    pub fn select(config: Config, limit: Option<usize>, offset: Option<usize>, order_column: Option<String>, filter_params: Option<FilterParams>) -> JupiterResult<Vec<Self>> {
        let postgres = config.pg.clone();
        
        // Build secure query with parameterized placeholders
        let mut query = String::from("SELECT * FROM cached_weather_data");
        let mut param_count = 0;
        let mut params: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> = Vec::new();
        
        // Add WHERE clause if filter parameters provided
        if let Some(ref filters) = filter_params {
            if let Some(ref oid) = filters.oid {
                param_count += 1;
                query.push_str(&format!(" WHERE oid = ${}", param_count));
                // Note: actual parameter binding happens in the query execution
            }
        }
        
        // Add ORDER BY clause (validate column name against whitelist)
        let valid_order_columns = vec!["id", "timestamp", "oid"];
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
        
        let connector = create_combo_connector()
            .map_err(|e| JupiterError::SslError(format!("Failed to create SSL connector: {}", e)))?;
        let mut client = crate::postgres::Client::connect(
            format!("postgresql://{}:{}@{}/{}?sslmode=prefer", 
                &postgres.username, &postgres.password, &postgres.address, &postgres.db_name).as_str(), 
            connector
        )?;
        
        let mut parsed_rows: Vec<Self> = Vec::new();
        
        // Execute query with appropriate parameters
        let rows = if let Some(ref filters) = filter_params {
            if let Some(ref oid) = filters.oid {
                client.query(&query, &[oid])?
            } else {
                client.query(&query, &[])?
            }
        } else {
            client.query(&query, &[])?
        };
        
        for row in rows {
            parsed_rows.push(Self::from_row(&row)?);
        }
        
        return Ok(parsed_rows);
    }
    fn from_row(row: &Row) -> JupiterResult<Self> {
        return Ok(Self {
            id: row.get("id"),
            oid: row.get("oid"),
            accuweather: row.get("accuweather"),
            homebrew: row.get("homebrew"),
            openweathermap: row.get("openweathermap"),
            timestamp: row.get("timestamp"),
        });
    }
}



// Lives in memory, no SQL
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PostgresServer {
	pub db_name: String,
    pub username: String,
    pub password: String,
	pub address: String
}
impl PostgresServer {
    pub fn new() -> Result<PostgresServer, ConfigError> {
        let config = DatabaseConfig::combo_from_env()?;
        
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