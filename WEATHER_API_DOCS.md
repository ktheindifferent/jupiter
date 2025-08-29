# Weather Provider API Documentation

## Overview

This library provides a unified interface for accessing multiple weather data providers through a common trait-based API. It supports AccuWeather, OpenWeather, homebrew weather stations, and a combo provider that averages results from multiple sources.

## Features

- **Unified Interface**: All providers implement the `WeatherProvider` trait
- **Multiple Data Sources**: AccuWeather, OpenWeather, and homebrew weather stations
- **Combo Provider**: Intelligently combines data from multiple sources with weighted averaging
- **Caching**: Built-in caching to reduce API calls and improve performance
- **Rate Limiting**: Automatic rate limiting to respect API quotas
- **Fallback Support**: Automatic fallback to alternative providers when one fails
- **Comprehensive Data**: Current weather, forecasts, alerts, and historical data

## Quick Start

### Basic Usage

```rust
use jupiter::provider::common::WeatherProvider;
use jupiter::provider::openweather::OpenWeatherProvider;

#[tokio::main]
async fn main() {
    let provider = OpenWeatherProvider::new("your_api_key".to_string());
    
    match provider.get_current_weather("New York").await {
        Ok(weather) => {
            println!("Temperature: {}°C", weather.temperature);
            println!("Description: {}", weather.description);
        }
        Err(e) => eprintln!("Error: {}", e),
    }
}
```

### Using the Combo Provider

```rust
use jupiter::provider::combo_enhanced::ComboProvider;
use jupiter::provider::accuweather_enhanced::AccuWeatherProvider;
use jupiter::provider::openweather::OpenWeatherProvider;

#[tokio::main]
async fn main() {
    let combo = ComboProvider::new()
        .add_provider(
            Box::new(AccuWeatherProvider::new("accu_key".to_string())),
            1.5  // Higher weight for AccuWeather
        )
        .add_provider(
            Box::new(OpenWeatherProvider::new("open_key".to_string())),
            1.0  // Normal weight for OpenWeather
        )
        .set_cache_duration(300)  // Cache for 5 minutes
        .set_fallback_enabled(true);  // Enable automatic fallback
    
    let weather = combo.get_current_weather("London").await.unwrap();
    println!("Combined temperature: {}°C", weather.temperature);
}
```

## Provider Configuration

### AccuWeather

```rust
use jupiter::provider::accuweather_enhanced::AccuWeatherProvider;

let provider = AccuWeatherProvider::new("your_api_key".to_string());
```

**Environment Variables:**
- `ACCUWEATHER_API_KEY`: Your AccuWeather API key

**Features:**
- Current weather conditions
- 5-day forecast with daily and hourly data
- Weather alerts
- UV index and air quality data
- Rate limited to 50 requests/hour (free tier)

### OpenWeather

```rust
use jupiter::provider::openweather::OpenWeatherProvider;

let provider = OpenWeatherProvider::new("your_api_key".to_string());
```

**Environment Variables:**
- `OPENWEATHER_API_KEY`: Your OpenWeather API key

**Features:**
- Current weather conditions
- 5-day forecast with 3-hour steps
- Historical weather data (requires subscription)
- Weather alerts (One Call API)
- Rate limited to 60 requests/minute (free tier)

### Homebrew Weather Station

```rust
use jupiter::provider::homebrew_enhanced::HomebrewProvider;
use jupiter::provider::homebrew::{Config, PostgresServer};

let config = Config {
    apikey: "internal_api_key".to_string(),
    pg: PostgresServer::new(),
    port: 8080,
};

let mut provider = HomebrewProvider::new(config);

// Configure location mappings
provider.add_location_mapping(
    "home".to_string(),
    40.7128,
    -74.0060,
    "Home Station".to_string(),
    vec!["indoor".to_string(), "outdoor".to_string()]
);
```

**Environment Variables:**
- `HOMEBREW_PG_DBNAME`: PostgreSQL database name
- `HOMEBREW_PG_USER`: PostgreSQL username
- `HOMEBREW_PG_PASS`: PostgreSQL password
- `HOMEBREW_PG_ADDRESS`: PostgreSQL server address

**Features:**
- Support for multiple sensor types (temperature, humidity, PM2.5, PM10, CO2, TVOC)
- Indoor/outdoor sensor separation
- Historical data aggregation
- Air quality alerts
- Custom alert thresholds

### Combo Provider

The Combo provider aggregates data from multiple sources:

```rust
use jupiter::provider::combo_enhanced::ComboProvider;

let combo = ComboProvider::new()
    .add_provider(provider1, weight1)
    .add_provider(provider2, weight2)
    .set_cache_duration(seconds)
    .set_fallback_enabled(bool);
```

**Features:**
- Weighted averaging of weather data
- Intelligent caching to reduce API calls
- Automatic fallback when providers fail
- Alert deduplication and merging
- Support for all features of underlying providers

## API Methods

### WeatherProvider Trait

All providers implement the following methods:

```rust
#[async_trait]
pub trait WeatherProvider: Send + Sync {
    // Get current weather conditions
    async fn get_current_weather(&self, location: &str) -> Result<Weather, WeatherError>;
    
    // Get weather forecast
    async fn get_forecast(&self, location: &str, days: u8) -> Result<Forecast, WeatherError>;
    
    // Get weather alerts for a location
    async fn get_alerts(&self, location: &str) -> Result<Vec<Alert>, WeatherError>;
    
    // Get historical weather data (optional)
    async fn get_historical(&self, location: &str, date: &str) -> Result<HistoricalData, WeatherError>;
    
    // Get provider name
    fn name(&self) -> &str;
    
    // Check if a feature is supported
    fn supports_feature(&self, feature: WeatherFeature) -> bool;
}
```

### Data Structures

#### Weather
```rust
pub struct Weather {
    pub temperature: f64,              // Temperature in Celsius
    pub feels_like: Option<f64>,       // Feels-like temperature
    pub humidity: Option<f64>,         // Humidity percentage
    pub pressure: Option<f64>,         // Atmospheric pressure in hPa
    pub wind_speed: Option<f64>,       // Wind speed in m/s
    pub wind_direction: Option<f64>,   // Wind direction in degrees
    pub description: String,           // Weather description
    pub icon: Option<String>,          // Weather icon code
    pub precipitation: Option<f64>,    // Precipitation in mm
    pub visibility: Option<f64>,       // Visibility in meters
    pub uv_index: Option<f64>,         // UV index
    pub provider: String,              // Data provider name
    pub location: Location,            // Location information
    pub timestamp: i64,                // Unix timestamp
}
```

#### Forecast
```rust
pub struct Forecast {
    pub location: Location,
    pub provider: String,
    pub daily: Vec<DailyForecast>,
    pub hourly: Option<Vec<HourlyForecast>>,
}
```

#### Alert
```rust
pub struct Alert {
    pub title: String,
    pub description: String,
    pub severity: AlertSeverity,
    pub start: String,
    pub end: Option<String>,
    pub regions: Vec<String>,
}
```

## Error Handling

The library defines comprehensive error types:

```rust
pub enum WeatherError {
    NetworkError(String),
    ParseError(String),
    NotFound(String),
    RateLimitExceeded,
    InvalidApiKey,
    ConfigurationError(String),
    DatabaseError(String),
}
```

## Examples

### Getting Weather with Fallback

```rust
async fn get_weather_with_fallback(location: &str) -> Result<Weather, WeatherError> {
    let providers: Vec<Box<dyn WeatherProvider>> = vec![
        Box::new(AccuWeatherProvider::new("key1".to_string())),
        Box::new(OpenWeatherProvider::new("key2".to_string())),
    ];
    
    for provider in providers {
        match provider.get_current_weather(location).await {
            Ok(weather) => return Ok(weather),
            Err(e) => eprintln!("Provider {} failed: {}", provider.name(), e),
        }
    }
    
    Err(WeatherError::NotFound("All providers failed".to_string()))
}
```

### Monitoring Air Quality with Homebrew Sensors

```rust
async fn check_air_quality(provider: &HomebrewProvider) -> Result<Vec<Alert>, WeatherError> {
    let alerts = provider.get_alerts("home").await?;
    
    for alert in &alerts {
        match alert.severity {
            AlertSeverity::Severe | AlertSeverity::Extreme => {
                println!("URGENT: {}", alert.title);
                // Send notification
            }
            _ => println!("Warning: {}", alert.title),
        }
    }
    
    Ok(alerts)
}
```

### Averaging Multiple Forecasts

```rust
async fn get_averaged_forecast(location: &str, days: u8) -> Result<Forecast, WeatherError> {
    let combo = ComboProvider::new()
        .add_provider(Box::new(provider1), 1.0)
        .add_provider(Box::new(provider2), 1.0)
        .add_provider(Box::new(provider3), 0.5);  // Lower weight for less reliable provider
    
    combo.get_forecast(location, days).await
}
```

## Testing

Run unit tests:
```bash
cargo test
```

Run integration tests (requires API keys):
```bash
ACCUWEATHER_API_KEY=xxx OPENWEATHER_API_KEY=yyy cargo test --test integration_tests
```

## Rate Limiting

Each provider includes built-in rate limiting:

- **AccuWeather**: 50 requests/hour (free tier)
- **OpenWeather**: 60 requests/minute (free tier)
- **Homebrew**: Configurable, default 10 requests/minute

The rate limiter automatically tracks requests and returns `WeatherError::RateLimitExceeded` when limits are reached.

## Caching

The Combo provider includes intelligent caching:

- Configurable cache duration (default: 5 minutes)
- Automatic cache invalidation
- Reduces API calls and costs
- Improves response times

## Best Practices

1. **Use the Combo Provider**: For production applications, use the Combo provider with multiple data sources for reliability
2. **Configure Weights**: Assign higher weights to more reliable or accurate providers
3. **Enable Caching**: Use caching to reduce API calls and improve performance
4. **Handle Errors**: Always handle errors gracefully with fallback logic
5. **Monitor Rate Limits**: Track API usage to avoid hitting rate limits
6. **Secure API Keys**: Store API keys in environment variables, never in code

## License

See LICENSE file for details.