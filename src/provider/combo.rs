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

use tokio_postgres::{Error, Row};
use crate::ssl_config::{create_combo_connector, SslConfig};
use crate::input_sanitizer::{InputSanitizer, DatabaseInputValidator, ValidationError};
use crate::db_pool::{DatabasePool, DatabaseConfig, init_combo_pool, get_combo_pool};

// Ability to combine, average, and cache final values between all configured providers.

// Secure filter parameters for database queries
#[derive(Debug, Clone)]
pub struct FilterParams {
    pub oid: Option<String>,
    // Add more filter fields as needed
}

// Lives in memory, no SQL
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub accu_config: Option<crate::provider::accuweather::Config>,
    pub homebrew_config: Option<crate::provider::homebrew::Config>,
    pub apikey: String,
    pub cache_timeout: Option<i64>,
    pub pg: PostgresServer,
    pub port: u16,
    pub zip_code: String
}
impl Config {
    pub async fn init(&self){
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
        
        match init_combo_pool(db_config).await {
            Ok(pool) => {
                log::info!("[combo] Database connection pool initialized successfully");
                // Log initial pool status
                let status = pool.status();
                status.log("combo");
            },
            Err(e) => {
                log::error!("[combo] Failed to initialize database connection pool: {}", e);
                panic!("Unable to initialize database connection pool: {}", e);
            }
        }

        self.build_tables().await;

        let config = self.clone();
        thread::spawn(move || {
            // Create rate limiter: max 10 attempts per minute per IP
            let rate_limiter = Arc::new(RateLimiter::new(10, 60));
            
            rouille::start_server(format!("0.0.0.0:{}", config.port).as_str(), move |request| {
    
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
                                let objects = crate::provider::homebrew::WeatherReport::select(cfg.clone(), Some(1), None, Some(format!("timestamp DESC")), None).unwrap();
                                
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
                            let objects = CachedWeatherData::select(config.clone(), Some(1), None, Some(format!("timestamp DESC")), None).unwrap();
                            
                            // Use safe array access with .first()
                            if let Some(first) = objects.first() {
                                let current_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
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
                                            let j = serde_json::to_string(&current).unwrap();
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
                            let objects = crate::provider::homebrew::WeatherReport::select(cfg.clone(), Some(1), None, Some(format!("timestamp DESC")), None).unwrap();
                            
                            // Use safe array access to prevent panic on empty results
                            if let Some(first) = objects.first() {
                                let j = serde_json::to_string(&first.clone()).unwrap();
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
                
                // Add metrics endpoint
                if request.url() == "/metrics" {
                    if request.method() == "GET" {
                        let metrics_json = crate::pool_monitor::handle_metrics_endpoint();
                        return Response::text(metrics_json)
                            .with_additional_header("Content-Type", "application/json");
                    }
                }
    
    
                let mut response = Response::text("hello world");

                return response;
            });
        });
    }

    pub async fn build_tables(&self) -> Result<(), Error>{
        // Get connection from pool
        let pool = get_combo_pool()
            .expect("Database pool not initialized");
        
        let client = pool.get_connection_with_retry(3).await
            .expect("Failed to get database connection");
    
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
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

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
    pub fn save(&self, config: Config) -> Result<&Self, Error>{
        // Use async runtime to get connection from pool
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let client = runtime.block_on(async {
            let pool = get_combo_pool()
                .expect("Database pool not initialized");
            
            pool.get_connection_with_retry(3).await
                .expect("Failed to get database connection")
        });

        // Search for OID matches using secure parameterized query
        let rows = Self::select_by_oid(
            config.clone(),
            &self.oid
        ).unwrap();

        if rows.len() == 0 {
            runtime.block_on(client.execute("INSERT INTO cached_weather_data (oid, timestamp) VALUES ($1, $2)",
                &[&self.oid.clone(),
                &self.timestamp]
            )).unwrap();
        } 

        if self.accuweather.is_some() {
            runtime.block_on(client.execute("UPDATE cached_weather_data SET accuweather = $1 WHERE oid = $2;", 
            &[
                &self.accuweather.clone().unwrap(),
                &self.oid
            ]))?;
        }

        if self.homebrew.is_some() {
            runtime.block_on(client.execute("UPDATE cached_weather_data SET homebrew = $1 WHERE oid = $2;", 
            &[
                &self.homebrew.clone().unwrap(),
                &self.oid
            ]))?;
        }

        if self.openweathermap.is_some() {
            runtime.block_on(client.execute("UPDATE cached_weather_data SET openweathermap = $1 WHERE oid = $2;", 
            &[
                &self.openweathermap.clone().unwrap(),
                &self.oid
            ]))?;
        }

        return Ok(self);
    }
    // Secure method to select by OID using parameterized query
    pub fn select_by_oid(config: Config, oid: &str) -> Result<Vec<Self>, Error> {
        // Validate OID input before using in query
        if !InputSanitizer::validate_oid(oid) {
            log::error!("Invalid OID format detected: {}", oid);
        }
        
        if !InputSanitizer::check_for_sql_keywords(oid) {
            log::error!("Potential SQL injection detected in OID: {}", oid);
        }
        
        // Use async runtime to get connection from pool
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let pool = get_combo_pool()
                .expect("Database pool not initialized");
            
            let client = pool.get_connection_with_retry(3).await
                .expect("Failed to get database connection");
            
            let query = "SELECT * FROM cached_weather_data WHERE oid = $1 ORDER BY id DESC";
            let rows = client.query(query, &[&oid]).await?;
            
            let mut parsed_rows: Vec<Self> = Vec::new();
            for row in rows {
                parsed_rows.push(Self::from_row(&row).unwrap());
            }
            
            Ok(parsed_rows)
        })
    }
    
    // Secure select method with parameterized queries
    pub fn select(config: Config, limit: Option<usize>, offset: Option<usize>, order_column: Option<String>, filter_params: Option<FilterParams>) -> Result<Vec<Self>, Error> {
        // Build secure query with parameterized placeholders
        let mut query = String::from("SELECT * FROM cached_weather_data");
        let mut param_count = 0;
        
        // Add WHERE clause if filter parameters provided
        if let Some(ref filters) = filter_params {
            if let Some(ref oid) = filters.oid {
                param_count += 1;
                query.push_str(&format!(" WHERE oid = ${}", param_count));
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
        
        // Use async runtime to get connection from pool
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let pool = get_combo_pool()
                .expect("Database pool not initialized");
            
            let client = pool.get_connection_with_retry(3).await
                .expect("Failed to get database connection");
            
            // Execute query with appropriate parameters
            let rows = if let Some(ref filters) = filter_params {
                if let Some(ref oid) = filters.oid {
                    client.query(&query, &[oid]).await?
                } else {
                    client.query(&query, &[]).await?
                }
            } else {
                client.query(&query, &[]).await?
            };
            
            let mut parsed_rows: Vec<Self> = Vec::new();
            for row in rows {
                parsed_rows.push(Self::from_row(&row).unwrap());
            }
            
            Ok(parsed_rows)
        })
    }
    fn from_row(row: &Row) -> Result<Self, Error> {
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
    pub fn new() -> PostgresServer {

        let db_name = env::var("COMBO_PG_DBNAME").expect("$COMBO_PG_DBNAME is not set");
        let username = env::var("COMBO_PG_USER").expect("$COMBO_PG_USER is not set");
        let password = env::var("COMBO_PG_PASS").expect("$COMBO_PG_PASS is not set");
        let address = env::var("COMBO_PG_ADDRESS").expect("$COMBO_PG_ADDRESS is not set");


        PostgresServer{
            db_name, 
            username, 
            password, 
            address
        }
    }
}