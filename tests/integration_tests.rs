use jupiter::provider::common::*;
use jupiter::provider::accuweather_enhanced::AccuWeatherProvider;
use jupiter::provider::openweather::OpenWeatherProvider;
use jupiter::provider::homebrew_enhanced::HomebrewProvider;
use jupiter::provider::combo_enhanced::ComboProvider;
use jupiter::provider::homebrew::{Config as HomebrewConfig, PostgresServer};

#[tokio::test]
async fn test_combo_provider_with_multiple_providers() {
    let combo = ComboProvider::new()
        .set_cache_duration(300)
        .set_fallback_enabled(true);
    
    assert_eq!(combo.name(), "Combo");
    assert!(combo.supports_feature(WeatherFeature::CurrentWeather));
}

#[tokio::test]
async fn test_weather_provider_trait_implementation() {
    let providers: Vec<Box<dyn WeatherProvider>> = vec![
        Box::new(AccuWeatherProvider::new("test_key".to_string())),
        Box::new(OpenWeatherProvider::new("test_key".to_string())),
    ];
    
    for provider in providers {
        assert!(!provider.name().is_empty());
        assert!(provider.supports_feature(WeatherFeature::CurrentWeather));
        assert!(provider.supports_feature(WeatherFeature::Forecast));
    }
}

#[cfg(test)]
mod mock_tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::RwLock;
    
    struct MockWeatherProvider {
        name: String,
        weather_data: Arc<RwLock<Option<Weather>>>,
        forecast_data: Arc<RwLock<Option<Forecast>>>,
        alerts_data: Arc<RwLock<Vec<Alert>>>,
    }
    
    impl MockWeatherProvider {
        fn new(name: String) -> Self {
            Self {
                name,
                weather_data: Arc::new(RwLock::new(None)),
                forecast_data: Arc::new(RwLock::new(None)),
                alerts_data: Arc::new(RwLock::new(Vec::new())),
            }
        }
        
        async fn set_weather(&self, weather: Weather) {
            let mut data = self.weather_data.write().await;
            *data = Some(weather);
        }
        
        async fn set_forecast(&self, forecast: Forecast) {
            let mut data = self.forecast_data.write().await;
            *data = Some(forecast);
        }
        
        async fn set_alerts(&self, alerts: Vec<Alert>) {
            let mut data = self.alerts_data.write().await;
            *data = alerts;
        }
    }
    
    #[async_trait::async_trait]
    impl WeatherProvider for MockWeatherProvider {
        async fn get_current_weather(&self, _location: &str) -> Result<Weather, WeatherError> {
            let data = self.weather_data.read().await;
            data.clone().ok_or_else(|| WeatherError::NotFound("No mock data".to_string()))
        }
        
        async fn get_forecast(&self, _location: &str, _days: u8) -> Result<Forecast, WeatherError> {
            let data = self.forecast_data.read().await;
            data.clone().ok_or_else(|| WeatherError::NotFound("No mock data".to_string()))
        }
        
        async fn get_alerts(&self, _location: &str) -> Result<Vec<Alert>, WeatherError> {
            let data = self.alerts_data.read().await;
            Ok(data.clone())
        }
        
        fn name(&self) -> &str {
            &self.name
        }
        
        fn supports_feature(&self, _feature: WeatherFeature) -> bool {
            true
        }
    }
    
    #[tokio::test]
    async fn test_mock_weather_provider() {
        let provider = MockWeatherProvider::new("Mock".to_string());
        
        let test_weather = Weather {
            temperature: 25.0,
            feels_like: Some(24.0),
            humidity: Some(60.0),
            pressure: Some(1015.0),
            wind_speed: Some(10.0),
            wind_direction: Some(180.0),
            description: "Test weather".to_string(),
            icon: None,
            precipitation: Some(0.0),
            visibility: Some(10000.0),
            uv_index: Some(5.0),
            provider: "Mock".to_string(),
            location: Location {
                latitude: 0.0,
                longitude: 0.0,
                name: "Test Location".to_string(),
                country: None,
                region: None,
                postal_code: None,
            },
            timestamp: 0,
        };
        
        provider.set_weather(test_weather.clone()).await;
        
        let result = provider.get_current_weather("test").await.unwrap();
        assert_eq!(result.temperature, 25.0);
        assert_eq!(result.description, "Test weather");
    }
    
    #[tokio::test]
    async fn test_combo_provider_with_mocks() {
        let mock1 = Box::new(MockWeatherProvider::new("Mock1".to_string()));
        let mock2 = Box::new(MockWeatherProvider::new("Mock2".to_string()));
        
        let weather1 = Weather {
            temperature: 20.0,
            feels_like: None,
            humidity: Some(50.0),
            pressure: None,
            wind_speed: None,
            wind_direction: None,
            description: "Mock1 weather".to_string(),
            icon: None,
            precipitation: None,
            visibility: None,
            uv_index: None,
            provider: "Mock1".to_string(),
            location: Location {
                latitude: 0.0,
                longitude: 0.0,
                name: "Test".to_string(),
                country: None,
                region: None,
                postal_code: None,
            },
            timestamp: 0,
        };
        
        let weather2 = Weather {
            temperature: 22.0,
            feels_like: None,
            humidity: Some(60.0),
            pressure: None,
            wind_speed: None,
            wind_direction: None,
            description: "Mock2 weather".to_string(),
            icon: None,
            precipitation: None,
            visibility: None,
            uv_index: None,
            provider: "Mock2".to_string(),
            location: Location {
                latitude: 0.0,
                longitude: 0.0,
                name: "Test".to_string(),
                country: None,
                region: None,
                postal_code: None,
            },
            timestamp: 0,
        };
        
        mock1.set_weather(weather1).await;
        mock2.set_weather(weather2).await;
        
        let combo = ComboProvider::new()
            .add_provider(mock1, 1.0)
            .add_provider(mock2, 1.0);
        
        let result = combo.get_current_weather("test").await.unwrap();
        assert_eq!(result.temperature, 21.0);
        assert!(result.description.contains("Combined"));
    }
    
    #[tokio::test]
    async fn test_alert_merging() {
        let mock1 = Box::new(MockWeatherProvider::new("Mock1".to_string()));
        let mock2 = Box::new(MockWeatherProvider::new("Mock2".to_string()));
        
        let alerts1 = vec![
            Alert {
                title: "Storm Warning".to_string(),
                description: "Severe storm approaching".to_string(),
                severity: AlertSeverity::Severe,
                start: "2024-01-01T12:00:00".to_string(),
                end: None,
                regions: vec!["Region1".to_string()],
            },
        ];
        
        let alerts2 = vec![
            Alert {
                title: "Heat Advisory".to_string(),
                description: "High temperatures expected".to_string(),
                severity: AlertSeverity::Moderate,
                start: "2024-01-01T14:00:00".to_string(),
                end: None,
                regions: vec!["Region2".to_string()],
            },
        ];
        
        mock1.set_alerts(alerts1).await;
        mock2.set_alerts(alerts2).await;
        
        let combo = ComboProvider::new()
            .add_provider(mock1, 1.0)
            .add_provider(mock2, 1.0);
        
        let result = combo.get_alerts("test").await.unwrap();
        assert_eq!(result.len(), 2);
        assert!(result[0].title.contains("[Mock"));
    }
}