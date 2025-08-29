use jupiter::provider::common::{WeatherProvider, WeatherFeature};
use jupiter::provider::accuweather_enhanced::AccuWeatherProvider;
use jupiter::provider::openweather::OpenWeatherProvider;
use jupiter::provider::homebrew_enhanced::HomebrewProvider;
use jupiter::provider::combo_enhanced::ComboProvider;
use jupiter::provider::homebrew::{Config as HomebrewConfig, PostgresServer};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    simple_logger::init().unwrap();
    
    println!("Weather Provider API Examples\n");
    
    example_single_provider().await?;
    example_combo_provider().await?;
    example_homebrew_monitoring().await?;
    example_weather_alerts().await?;
    example_forecast_comparison().await?;
    
    Ok(())
}

async fn example_single_provider() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Single Provider Example ===");
    
    let api_key = env::var("OPENWEATHER_API_KEY").unwrap_or_else(|_| "demo".to_string());
    let provider = OpenWeatherProvider::new(api_key);
    
    match provider.get_current_weather("London").await {
        Ok(weather) => {
            println!("Provider: {}", provider.name());
            println!("Location: {}", weather.location.name);
            println!("Temperature: {:.1}°C", weather.temperature);
            println!("Feels like: {:.1}°C", weather.feels_like.unwrap_or(0.0));
            println!("Humidity: {:.0}%", weather.humidity.unwrap_or(0.0));
            println!("Description: {}", weather.description);
            
            if let Some(wind_speed) = weather.wind_speed {
                println!("Wind: {:.1} m/s", wind_speed);
            }
            
            if let Some(precipitation) = weather.precipitation {
                println!("Precipitation: {:.1} mm", precipitation);
            }
        }
        Err(e) => {
            eprintln!("Error fetching weather: {}", e);
        }
    }
    
    println!();
    Ok(())
}

async fn example_combo_provider() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Combo Provider Example ===");
    
    let accu_key = env::var("ACCUWEATHER_API_KEY").unwrap_or_else(|_| "demo1".to_string());
    let open_key = env::var("OPENWEATHER_API_KEY").unwrap_or_else(|_| "demo2".to_string());
    
    let combo = ComboProvider::new()
        .add_provider(
            Box::new(AccuWeatherProvider::new(accu_key)),
            1.5
        )
        .add_provider(
            Box::new(OpenWeatherProvider::new(open_key)),
            1.0
        )
        .set_cache_duration(300)
        .set_fallback_enabled(true);
    
    println!("Combo provider configured with:");
    println!("- AccuWeather (weight: 1.5)");
    println!("- OpenWeather (weight: 1.0)");
    println!("- Cache duration: 5 minutes");
    println!("- Fallback: enabled");
    
    match combo.get_current_weather("New York").await {
        Ok(weather) => {
            println!("\nAveraged Weather Data:");
            println!("Temperature: {:.1}°C", weather.temperature);
            println!("Description: {}", weather.description);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
    
    println!();
    Ok(())
}

async fn example_homebrew_monitoring() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Homebrew Weather Station Example ===");
    
    if env::var("HOMEBREW_PG_DBNAME").is_err() {
        println!("Skipping: Database not configured");
        println!("Set HOMEBREW_PG_* environment variables to enable");
        println!();
        return Ok(());
    }
    
    let config = HomebrewConfig {
        apikey: "internal_key".to_string(),
        pg: PostgresServer::new(),
        port: 8080,
    };
    
    let mut provider = HomebrewProvider::new(config.clone());
    
    provider.add_location_mapping(
        "home".to_string(),
        37.7749,
        -122.4194,
        "Home Weather Station".to_string(),
        vec!["indoor".to_string(), "outdoor".to_string()]
    );
    
    provider.add_location_mapping(
        "greenhouse".to_string(),
        37.7750,
        -122.4195,
        "Greenhouse Monitor".to_string(),
        vec!["greenhouse".to_string()]
    );
    
    match provider.get_current_weather("home").await {
        Ok(weather) => {
            println!("Home Station Data:");
            println!("Temperature: {:.1}°C", weather.temperature);
            
            if let Some(humidity) = weather.humidity {
                println!("Humidity: {:.0}%", humidity);
            }
            
            println!("Description: {}", weather.description);
        }
        Err(e) => {
            eprintln!("Error: {}", e);
        }
    }
    
    let alerts = provider.get_alerts("home").await?;
    if !alerts.is_empty() {
        println!("\nActive Alerts:");
        for alert in alerts {
            println!("- {}: {}", alert.title, alert.description);
        }
    }
    
    println!();
    Ok(())
}

async fn example_weather_alerts() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Weather Alerts Example ===");
    
    let providers: Vec<Box<dyn WeatherProvider>> = vec![
        Box::new(AccuWeatherProvider::new(
            env::var("ACCUWEATHER_API_KEY").unwrap_or_else(|_| "demo".to_string())
        )),
        Box::new(OpenWeatherProvider::new(
            env::var("OPENWEATHER_API_KEY").unwrap_or_else(|_| "demo".to_string())
        )),
    ];
    
    let location = "Miami";
    
    for provider in providers {
        if !provider.supports_feature(WeatherFeature::Alerts) {
            continue;
        }
        
        println!("Checking alerts from {}", provider.name());
        
        match provider.get_alerts(location).await {
            Ok(alerts) => {
                if alerts.is_empty() {
                    println!("No active alerts");
                } else {
                    for alert in alerts {
                        println!("Alert: {}", alert.title);
                        println!("Severity: {:?}", alert.severity);
                        println!("Start: {}", alert.start);
                        if let Some(end) = alert.end {
                            println!("End: {}", end);
                        }
                        println!("Regions: {}", alert.regions.join(", "));
                        println!();
                    }
                }
            }
            Err(e) => {
                eprintln!("Error fetching alerts: {}", e);
            }
        }
    }
    
    println!();
    Ok(())
}

async fn example_forecast_comparison() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Forecast Comparison Example ===");
    
    let providers: Vec<Box<dyn WeatherProvider>> = vec![
        Box::new(AccuWeatherProvider::new(
            env::var("ACCUWEATHER_API_KEY").unwrap_or_else(|_| "demo1".to_string())
        )),
        Box::new(OpenWeatherProvider::new(
            env::var("OPENWEATHER_API_KEY").unwrap_or_else(|_| "demo2".to_string())
        )),
    ];
    
    let location = "Paris";
    let days = 3;
    
    println!("Comparing {}-day forecasts for {}", days, location);
    println!();
    
    for provider in providers {
        println!("Provider: {}", provider.name());
        
        match provider.get_forecast(location, days).await {
            Ok(forecast) => {
                for day in &forecast.daily {
                    println!("  {}", day.date);
                    println!("    Min: {:.1}°C, Max: {:.1}°C", 
                        day.temperature_min, day.temperature_max);
                    
                    if let Some(humidity) = day.humidity {
                        println!("    Humidity: {:.0}%", humidity);
                    }
                    
                    if let Some(precip_prob) = day.precipitation_probability {
                        println!("    Precipitation chance: {:.0}%", precip_prob);
                    }
                    
                    if let Some(precip_amt) = day.precipitation_amount {
                        println!("    Precipitation amount: {:.1} mm", precip_amt);
                    }
                    
                    println!("    Description: {}", day.description);
                }
                
                if let Some(hourly) = &forecast.hourly {
                    println!("  Hourly forecast available: {} hours", hourly.len());
                }
            }
            Err(e) => {
                eprintln!("  Error: {}", e);
            }
        }
        println!();
    }
    
    Ok(())
}

#[allow(dead_code)]
async fn example_historical_data() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Historical Data Example ===");
    
    let provider = OpenWeatherProvider::new(
        env::var("OPENWEATHER_API_KEY").unwrap_or_else(|_| "demo".to_string())
    );
    
    if !provider.supports_feature(WeatherFeature::HistoricalData) {
        println!("Provider {} does not support historical data", provider.name());
        return Ok(());
    }
    
    let location = "Berlin";
    let date = "2024-01-01";
    
    match provider.get_historical(location, date).await {
        Ok(data) => {
            println!("Historical weather for {} on {}", location, date);
            println!("Temperature:");
            println!("  Min: {:.1}°C", data.temperature_min);
            println!("  Max: {:.1}°C", data.temperature_max);
            println!("  Avg: {:.1}°C", data.temperature_avg);
            
            if let Some(humidity) = data.humidity_avg {
                println!("Average humidity: {:.0}%", humidity);
            }
            
            if let Some(precip) = data.precipitation_total {
                println!("Total precipitation: {:.1} mm", precip);
            }
            
            if let Some(wind) = data.wind_speed_avg {
                println!("Average wind speed: {:.1} m/s", wind);
            }
        }
        Err(e) => {
            eprintln!("Error fetching historical data: {}", e);
        }
    }
    
    println!();
    Ok(())
}

#[allow(dead_code)]
fn demonstrate_error_handling() {
    use jupiter::provider::common::WeatherError;
    
    fn handle_weather_error(error: WeatherError) {
        match error {
            WeatherError::NetworkError(msg) => {
                eprintln!("Network issue: {}", msg);
            }
            WeatherError::InvalidApiKey => {
                eprintln!("Invalid API key. Please check your configuration.");
            }
            WeatherError::RateLimitExceeded => {
                eprintln!("Rate limit exceeded. Please wait before retrying.");
            }
            WeatherError::NotFound(msg) => {
                eprintln!("Not found: {}", msg);
            }
            WeatherError::ParseError(msg) => {
                eprintln!("Failed to parse response: {}", msg);
            }
            WeatherError::ConfigurationError(msg) => {
                eprintln!("Configuration error: {}", msg);
            }
            WeatherError::DatabaseError(msg) => {
                eprintln!("Database error: {}", msg);
            }
        }
    }
}