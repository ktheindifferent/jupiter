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

use tokio_postgres::{Error, Row};
use openssl::ssl::{SslConnector, SslMethod, SslVerifyMode};
use postgres_openssl::MakeTlsConnector;

// Ability to combine, average, and cache final values between all configured providers.

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

        self.build_tables().await;

        let config = self.clone();
        thread::spawn(move || {
            rouille::start_server(format!("0.0.0.0:{}", config.port).as_str(), move |request| {
    
    
                let auth_header = request.header("Authorization");
    
                if auth_header.is_none(){
                    return Response::empty_404();
                } else {
                    if auth_header.unwrap().to_string() != config.apikey{
                        return Response::empty_404();
                    }
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
                                return Response::json(&objects[0].clone());
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
                            if objects.len() > 0 {
                                let current_timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64;
                                let x = current_timestamp - objects[0].timestamp;
                                if x < timeout {
                                    return Response::json(&objects[0].clone());
                                }
                            }
                        },
                        None => {}
                    }

                    let mut resp = CachedWeatherData::new();

                    match config.accu_config.clone(){
                        Some(cfg) => {
                            let location = crate::provider::accuweather::Location::search_by_zip(cfg.clone(), config.zip_code.clone()).unwrap();
                            let current = crate::provider::accuweather::CurrentCondition::get(cfg, location.clone()).unwrap();
                            let j = serde_json::to_string(&current).unwrap();
                            resp.accuweather = Some(j);
                        },
                        None => {}
                    }
         

                    match config.homebrew_config.clone(){
                        Some(cfg) => {
                            let objects = crate::provider::homebrew::WeatherReport::select(cfg.clone(), Some(1), None, Some(format!("timestamp DESC")), None).unwrap();
                            
                            let j = serde_json::to_string(&objects[0].clone()).unwrap();
                            resp.homebrew = Some(j);
                        },
                        None => {}
                    }

                    resp.save(config.clone());

                    // let objects = WeatherReport::select(config.clone(), None, None, None, None).unwrap();
                    return Response::json(&resp);
                }
                
    
    
                let mut response = Response::text("hello world");

                return response;
            });
        });
    }

    pub async fn build_tables(&self) -> Result<(), Error>{
    
        let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
        builder.set_verify(SslVerifyMode::NONE);
        let connector = MakeTlsConnector::new(builder.build());
    
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
        // Get a copy of the master key and postgres info
        let postgres = config.pg.clone();

        // Build SQL adapter that skips verification for self signed certificates
        let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
        builder.set_verify(SslVerifyMode::NONE);

        // Build connector with the adapter from above
        let connector = MakeTlsConnector::new(builder.build());

        // Build postgres client
        let mut client = crate::postgres::Client::connect(format!("postgresql://{}:{}@{}/{}?sslmode=prefer", &postgres.username, &postgres.password, &postgres.address, &postgres.db_name).as_str(), connector)?;

        // Search for OID matches
        let rows = Self::select(
            config.clone(), 
            None, 
            None, 
            None, 
            Some(format!("oid = '{}'", 
                &self.oid, 
            ))
        ).unwrap();

        if rows.len() == 0 {
            client.execute("INSERT INTO cached_weather_data (oid, timestamp) VALUES ($1, $2)",
                &[&self.oid.clone(),
                &self.timestamp]
            ).unwrap();
        } 

        if self.accuweather.is_some() {
            client.execute("UPDATE cached_weather_data SET accuweather = $1 WHERE oid = $2;", 
            &[
                &self.accuweather.clone().unwrap(),
                &self.oid
            ])?;
        }

        if self.homebrew.is_some() {
            client.execute("UPDATE cached_weather_data SET homebrew = $1 WHERE oid = $2;", 
            &[
                &self.homebrew.clone().unwrap(),
                &self.oid
            ])?;
        }

        if self.openweathermap.is_some() {
            client.execute("UPDATE cached_weather_data SET openweathermap = $1 WHERE oid = $2;", 
            &[
                &self.openweathermap.clone().unwrap(),
                &self.oid
            ])?;
        }

        return Ok(self);
    }
    pub fn select(config: Config, limit: Option<usize>, offset: Option<usize>, order: Option<String>, query: Option<String>) -> Result<Vec<Self>, Error>{
        
    
        // Get a copy of the master key and postgres info
        let postgres = config.pg.clone();
            
        let mut execquery = "SELECT * FROM cached_weather_data".to_string();

        match query {
            Some(query_val) => {
                execquery = format!("{} {} {}", execquery.clone(), "WHERE", query_val);
            },
            None => {
                
            }
        }
        match order {
            Some(order_val) => {
                execquery = format!("{} {} {}", execquery.clone(), "ORDER BY", order_val);
            },
            None => {
                execquery = format!("{} {} {}", execquery.clone(), "ORDER BY", "id DESC");
            }
        }
        match limit {
            Some(limit_val) => {
                execquery = format!("{} {} {}", execquery.clone(), "LIMIT", limit_val);
            },
            None => {}
        }
        match offset {
            Some(offset_val) => {
                execquery = format!("{} {} {}", execquery.clone(), "OFFSET", offset_val);
            },
            None => {}
        }

        let mut builder = SslConnector::builder(SslMethod::tls()).unwrap();
        builder.set_verify(SslVerifyMode::NONE);
        let connector = MakeTlsConnector::new(builder.build());
        let mut client = crate::postgres::Client::connect(format!("postgresql://{}:{}@{}/{}?sslmode=prefer", &postgres.username, &postgres.password, &postgres.address, &postgres.db_name).as_str(), connector)?;

        let mut parsed_rows: Vec<Self> = Vec::new();
        for row in client.query(execquery.as_str(), &[])? {
            parsed_rows.push(Self::from_row(&row).unwrap());
        }

        return Ok(parsed_rows);
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