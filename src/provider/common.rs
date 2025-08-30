use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum WeatherError {
    NetworkError(String),
    ParseError(String),
    NotFound(String),
    RateLimitExceeded,
    InvalidApiKey,
    ConfigurationError(String),
    DatabaseError(String),
}

impl fmt::Display for WeatherError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            WeatherError::NetworkError(msg) => write!(f, "Network error: {}", msg),
            WeatherError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            WeatherError::NotFound(msg) => write!(f, "Not found: {}", msg),
            WeatherError::RateLimitExceeded => write!(f, "Rate limit exceeded"),
            WeatherError::InvalidApiKey => write!(f, "Invalid API key"),
            WeatherError::ConfigurationError(msg) => write!(f, "Configuration error: {}", msg),
            WeatherError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
        }
    }
}

impl Error for WeatherError {}

impl From<reqwest::Error> for WeatherError {
    fn from(err: reqwest::Error) -> Self {
        WeatherError::NetworkError(err.to_string())
    }
}

impl From<serde_json::Error> for WeatherError {
    fn from(err: serde_json::Error) -> Self {
        WeatherError::ParseError(err.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Weather {
    pub temperature: f64,
    pub feels_like: Option<f64>,
    pub humidity: Option<f64>,
    pub pressure: Option<f64>,
    pub wind_speed: Option<f64>,
    pub wind_direction: Option<f64>,
    pub description: String,
    pub icon: Option<String>,
    pub precipitation: Option<f64>,
    pub visibility: Option<f64>,
    pub uv_index: Option<f64>,
    pub provider: String,
    pub location: Location,
    pub timestamp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
    pub name: String,
    pub country: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Forecast {
    pub location: Location,
    pub provider: String,
    pub daily: Vec<DailyForecast>,
    pub hourly: Option<Vec<HourlyForecast>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyForecast {
    pub date: String,
    pub temperature_min: f64,
    pub temperature_max: f64,
    pub humidity: Option<f64>,
    pub precipitation_probability: Option<f64>,
    pub precipitation_amount: Option<f64>,
    pub wind_speed: Option<f64>,
    pub wind_direction: Option<f64>,
    pub description: String,
    pub icon: Option<String>,
    pub sunrise: Option<String>,
    pub sunset: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HourlyForecast {
    pub datetime: String,
    pub temperature: f64,
    pub feels_like: Option<f64>,
    pub humidity: Option<f64>,
    pub precipitation_probability: Option<f64>,
    pub precipitation_amount: Option<f64>,
    pub wind_speed: Option<f64>,
    pub wind_direction: Option<f64>,
    pub description: String,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Alert {
    pub title: String,
    pub description: String,
    pub severity: AlertSeverity,
    pub start: String,
    pub end: Option<String>,
    pub regions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum AlertSeverity {
    Minor,
    Moderate,
    Severe,
    Extreme,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoricalData {
    pub location: Location,
    pub provider: String,
    pub date: String,
    pub temperature_min: f64,
    pub temperature_max: f64,
    pub temperature_avg: f64,
    pub humidity_avg: Option<f64>,
    pub precipitation_total: Option<f64>,
    pub wind_speed_avg: Option<f64>,
}

#[async_trait]
pub trait WeatherProvider: Send + Sync {
    async fn get_current_weather(&self, location: &str) -> Result<Weather, WeatherError>;
    
    async fn get_forecast(&self, location: &str, days: u8) -> Result<Forecast, WeatherError>;
    
    async fn get_alerts(&self, location: &str) -> Result<Vec<Alert>, WeatherError>;
    
    async fn get_historical(&self, location: &str, date: &str) -> Result<HistoricalData, WeatherError> {
        Err(WeatherError::NotFound("Historical data not supported by this provider".to_string()))
    }
    
    fn name(&self) -> &str;
    
    fn supports_feature(&self, feature: WeatherFeature) -> bool;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WeatherFeature {
    CurrentWeather,
    Forecast,
    Alerts,
    HistoricalData,
    HourlyForecast,
    UvIndex,
    AirQuality,
}

pub struct RateLimiter {
    pub max_requests: u32,
    pub window_seconds: u64,
    pub requests: std::sync::Arc<std::sync::Mutex<Vec<std::time::Instant>>>,
}

impl RateLimiter {
    pub fn new(max_requests: u32, window_seconds: u64) -> Self {
        Self {
            max_requests,
            window_seconds,
            requests: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }
    
    pub fn check_rate_limit(&self) -> bool {
        let now = std::time::Instant::now();
        let window = std::time::Duration::from_secs(self.window_seconds);
        
        let mut requests = self.requests.lock().unwrap();
        requests.retain(|&req_time| now.duration_since(req_time) < window);
        
        if requests.len() < self.max_requests as usize {
            requests.push(now);
            true
        } else {
            false
        }
    }
}