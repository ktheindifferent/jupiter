#[cfg(test)]
mod tests {
    use super::super::common::*;
    use super::super::accuweather_enhanced::AccuWeatherProvider;
    use super::super::openweather::OpenWeatherProvider;
    use super::super::homebrew_enhanced::HomebrewProvider;
    use super::super::combo_enhanced::ComboProvider;
    use super::super::homebrew::{Config as HomebrewConfig, PostgresServer};
    
    fn create_test_location() -> Location {
        Location {
            latitude: 40.7128,
            longitude: -74.0060,
            name: "New York".to_string(),
            country: Some("US".to_string()),
            region: Some("NY".to_string()),
            postal_code: Some("10001".to_string()),
        }
    }
    
    #[test]
    fn test_weather_error_display() {
        let err = WeatherError::NetworkError("Connection failed".to_string());
        assert_eq!(err.to_string(), "Network error: Connection failed");
        
        let err = WeatherError::InvalidApiKey;
        assert_eq!(err.to_string(), "Invalid API key");
        
        let err = WeatherError::RateLimitExceeded;
        assert_eq!(err.to_string(), "Rate limit exceeded");
    }
    
    #[test]
    fn test_alert_severity_ordering() {
        use super::super::combo_enhanced::AlertSeverity;
        
        assert!(AlertSeverity::Extreme.cmp(&AlertSeverity::Severe) == std::cmp::Ordering::Greater);
        assert!(AlertSeverity::Severe.cmp(&AlertSeverity::Moderate) == std::cmp::Ordering::Greater);
        assert!(AlertSeverity::Moderate.cmp(&AlertSeverity::Minor) == std::cmp::Ordering::Greater);
        assert!(AlertSeverity::Minor.cmp(&AlertSeverity::Minor) == std::cmp::Ordering::Equal);
    }
    
    #[test]
    fn test_rate_limiter() {
        let limiter = RateLimiter::new(2, 1);
        
        assert!(limiter.check_rate_limit());
        assert!(limiter.check_rate_limit());
        assert!(!limiter.check_rate_limit());
        
        std::thread::sleep(std::time::Duration::from_secs(2));
        assert!(limiter.check_rate_limit());
    }
    
    #[test]
    fn test_weather_struct_creation() {
        let weather = Weather {
            temperature: 20.5,
            feels_like: Some(19.0),
            humidity: Some(65.0),
            pressure: Some(1013.25),
            wind_speed: Some(5.5),
            wind_direction: Some(180.0),
            description: "Partly cloudy".to_string(),
            icon: Some("02d".to_string()),
            precipitation: Some(0.0),
            visibility: Some(10000.0),
            uv_index: Some(3.0),
            provider: "Test".to_string(),
            location: create_test_location(),
            timestamp: 1234567890,
        };
        
        assert_eq!(weather.temperature, 20.5);
        assert_eq!(weather.provider, "Test");
        assert_eq!(weather.location.name, "New York");
    }
    
    #[test]
    fn test_forecast_struct_creation() {
        let daily = vec![
            DailyForecast {
                date: "2024-01-01".to_string(),
                temperature_min: 10.0,
                temperature_max: 20.0,
                humidity: Some(70.0),
                precipitation_probability: Some(30.0),
                precipitation_amount: Some(2.5),
                wind_speed: Some(10.0),
                wind_direction: Some(270.0),
                description: "Rain".to_string(),
                icon: Some("10d".to_string()),
                sunrise: Some("06:30".to_string()),
                sunset: Some("18:45".to_string()),
            },
        ];
        
        let forecast = Forecast {
            location: create_test_location(),
            provider: "Test".to_string(),
            daily,
            hourly: None,
        };
        
        assert_eq!(forecast.daily.len(), 1);
        assert_eq!(forecast.daily[0].temperature_max, 20.0);
    }
    
    #[test]
    fn test_alert_struct_creation() {
        let alert = Alert {
            title: "Severe Thunderstorm Warning".to_string(),
            description: "Severe thunderstorms expected".to_string(),
            severity: AlertSeverity::Severe,
            start: "2024-01-01T12:00:00".to_string(),
            end: Some("2024-01-01T18:00:00".to_string()),
            regions: vec!["New York".to_string(), "Brooklyn".to_string()],
        };
        
        assert_eq!(alert.title, "Severe Thunderstorm Warning");
        assert_eq!(alert.regions.len(), 2);
    }
    
    #[test]
    fn test_weather_feature_support() {
        let accuweather = AccuWeatherProvider::new("test_key".to_string());
        assert!(accuweather.supports_feature(WeatherFeature::CurrentWeather));
        assert!(accuweather.supports_feature(WeatherFeature::Forecast));
        assert!(accuweather.supports_feature(WeatherFeature::Alerts));
        assert!(!accuweather.supports_feature(WeatherFeature::HistoricalData));
        
        let openweather = OpenWeatherProvider::new("test_key".to_string());
        assert!(openweather.supports_feature(WeatherFeature::CurrentWeather));
        assert!(openweather.supports_feature(WeatherFeature::HistoricalData));
    }
    
    #[test]
    fn test_provider_names() {
        let accuweather = AccuWeatherProvider::new("test_key".to_string());
        assert_eq!(accuweather.name(), "AccuWeather");
        
        let openweather = OpenWeatherProvider::new("test_key".to_string());
        assert_eq!(openweather.name(), "OpenWeather");
        
        let combo = ComboProvider::new();
        assert_eq!(combo.name(), "Combo");
    }
    
    #[tokio::test]
    async fn test_combo_provider_builder() {
        let combo = ComboProvider::new()
            .set_cache_duration(600)
            .set_fallback_enabled(false);
        
        assert_eq!(combo.name(), "Combo");
    }
    
    #[test]
    fn test_historical_data_struct() {
        let historical = HistoricalData {
            location: create_test_location(),
            provider: "Test".to_string(),
            date: "2024-01-01".to_string(),
            temperature_min: 5.0,
            temperature_max: 15.0,
            temperature_avg: 10.0,
            humidity_avg: Some(75.0),
            precipitation_total: Some(5.0),
            wind_speed_avg: Some(8.0),
        };
        
        assert_eq!(historical.temperature_avg, 10.0);
        assert_eq!(historical.date, "2024-01-01");
    }
}

#[cfg(test)]
mod integration_tests {
    use super::super::common::*;
    
    #[tokio::test]
    #[ignore]
    async fn test_accuweather_integration() {
        let api_key = std::env::var("ACCUWEATHER_API_KEY").unwrap_or_else(|_| "test_key".to_string());
        let provider = super::super::accuweather_enhanced::AccuWeatherProvider::new(api_key);
        
        match provider.get_current_weather("10001").await {
            Ok(weather) => {
                assert!(weather.temperature != 0.0);
                assert_eq!(weather.provider, "AccuWeather");
            }
            Err(WeatherError::InvalidApiKey) => {
                println!("Skipping test: Invalid API key");
            }
            Err(e) => {
                panic!("Unexpected error: {:?}", e);
            }
        }
    }
    
    #[tokio::test]
    #[ignore]
    async fn test_openweather_integration() {
        let api_key = std::env::var("OPENWEATHER_API_KEY").unwrap_or_else(|_| "test_key".to_string());
        let provider = super::super::openweather::OpenWeatherProvider::new(api_key);
        
        match provider.get_current_weather("New York").await {
            Ok(weather) => {
                assert!(weather.temperature != 0.0);
                assert_eq!(weather.provider, "OpenWeather");
            }
            Err(WeatherError::InvalidApiKey) => {
                println!("Skipping test: Invalid API key");
            }
            Err(e) => {
                panic!("Unexpected error: {:?}", e);
            }
        }
    }
}