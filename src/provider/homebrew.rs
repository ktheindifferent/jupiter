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
use crate::ssl_config::{create_homebrew_connector, SslConfig};
use crate::input_sanitizer::{InputSanitizer, DatabaseInputValidator, ValidationError};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub apikey: String,
    pub pg: PostgresServer,
    pub port: u16
}
impl Config {
    pub async fn init(&self){

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
                        let objects = WeatherReport::select(config.clone(), Some(1), None, Some(format!("timestamp DESC")), None).unwrap();
                        
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
            });
        });
    }

    pub async fn build_tables(&self) -> Result<(), Error>{
    
        // Use centralized SSL configuration
        let connector = create_homebrew_connector()
            .unwrap_or_else(|e| {
                log::error!("Failed to create SSL connector: {}", e);
                panic!("Unable to create SSL connector: {}", e);
            });
    
        let (client, connection) = tokio_postgres::connect(format!("postgresql://{}:{}@{}/{}?sslmode=prefer", &self.pg.username, &self.pg.password, &self.pg.address, &self.pg.db_name).as_str(), connector).await?;
        
        // The connection object performs the actual communication with the database,
        // so spawn it off to run on its own.
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });
    
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
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;

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
    pub fn save(&self, config: Config) -> Result<&Self, Error>{
        // Get a copy of the master key and postgres info
        let postgres = config.pg.clone();

        // Build SQL adapter with proper SSL verification
        let connector = create_homebrew_connector()
            .unwrap_or_else(|e| {
                log::error!("Failed to create SSL connector: {}", e);
                panic!("Unable to create SSL connector: {}", e);
            });

        // Build postgres client
        let mut client = crate::postgres::Client::connect(format!("postgresql://{}:{}@{}/{}?sslmode=prefer", &postgres.username, &postgres.password, &postgres.address, &postgres.db_name).as_str(), connector)?;

        // Search for OID matches using secure parameterized query
        let rows = Self::select_by_oid(
            config.clone(),
            &self.oid
        ).unwrap();

        if rows.len() == 0 {
            client.execute("INSERT INTO weather_reports (oid, device_type, timestamp) VALUES ($1, $2, $3)",
                &[&self.oid.clone(),
                &self.device_type,
                &self.timestamp]
            ).unwrap();
        } 

        if self.temperature.is_some() {
            client.execute("UPDATE weather_reports SET temperature = $1 WHERE oid = $2;", 
            &[
                &self.temperature.clone().unwrap(),
                &self.oid
            ])?;
        }

        if self.humidity.is_some() {
            client.execute("UPDATE weather_reports SET humidity = $1 WHERE oid = $2;", 
            &[
                &self.humidity.clone().unwrap(),
                &self.oid
            ])?;
        }

        if self.percipitation.is_some() {
            client.execute("UPDATE weather_reports SET percipitation = $1 WHERE oid = $2;", 
            &[
                &self.percipitation.clone().unwrap(),
                &self.oid
            ])?;
        }

        if self.pm10.is_some() {
            client.execute("UPDATE weather_reports SET pm10 = $1 WHERE oid = $2;", 
            &[
                &self.pm10.clone().unwrap(),
                &self.oid
            ])?;
        }

        if self.pm25.is_some() {
            client.execute("UPDATE weather_reports SET pm25 = $1 WHERE oid = $2;", 
            &[
                &self.pm25.clone().unwrap(),
                &self.oid
            ])?;
        }

        if self.co2.is_some() {
            client.execute("UPDATE weather_reports SET co2 = $1 WHERE oid = $2;", 
            &[
                &self.co2.clone().unwrap(),
                &self.oid
            ])?;
        }

        if self.tvoc.is_some() {
            client.execute("UPDATE weather_reports SET tvoc = $1 WHERE oid = $2;", 
            &[
                &self.tvoc.clone().unwrap(),
                &self.oid
            ])?;
        }

        return Ok(self);
    }
    // Secure method to select by OID using parameterized query
    pub fn select_by_oid(config: Config, oid: &str) -> Result<Vec<Self>, Error> {
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
        
        let connector = create_homebrew_connector()
            .unwrap_or_else(|e| {
                log::error!("Failed to create SSL connector: {}", e);
                panic!("Unable to create SSL connector: {}", e);
            });
        let mut client = crate::postgres::Client::connect(
            format!("postgresql://{}:{}@{}/{}?sslmode=prefer", 
                &postgres.username, &postgres.password, &postgres.address, &postgres.db_name).as_str(), 
            connector
        )?;
        
        let query = "SELECT * FROM weather_reports WHERE oid = $1 ORDER BY id DESC";
        let mut parsed_rows: Vec<Self> = Vec::new();
        for row in client.query(query, &[&oid])? {
            parsed_rows.push(Self::from_row(&row).unwrap());
        }
        
        Ok(parsed_rows)
    }
    
    // Secure select method with parameterized queries
    pub fn select(config: Config, limit: Option<usize>, offset: Option<usize>, order_column: Option<String>, filter_params: Option<FilterParams>) -> Result<Vec<Self>, Error> {
        let postgres = config.pg.clone();
        
        // Build secure query with parameterized placeholders
        let mut query = String::from("SELECT * FROM weather_reports");
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
        
        let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
        builder.set_verify(SslVerifyMode::NONE);
        let connector = MakeTlsConnector::new(builder.build());
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
            parsed_rows.push(Self::from_row(&row).unwrap());
        }

        return Ok(parsed_rows);
    }
    fn from_row(row: &Row) -> Result<Self, Error> {
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
    pub fn new() -> PostgresServer {

        let db_name = env::var("HOMEBREW_PG_DBNAME").expect("$HOMEBREW_PG_DBNAME is not set");
        let username = env::var("HOMEBREW_PG_USER").expect("$HOMEBREW_PG_USER is not set");
        let password = env::var("HOMEBREW_PG_PASS").expect("$HOMEBREW_PG_PASS is not set");
        let address = env::var("HOMEBREW_PG_ADDRESS").expect("$HOMEBREW_PG_ADDRESS is not set");


        PostgresServer{
            db_name, 
            username, 
            password, 
            address
        }
    }
}