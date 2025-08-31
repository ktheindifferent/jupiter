use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use super::common::{
    Weather, WeatherError, WeatherProvider, Forecast, Alert, Location, 
    DailyForecast, HourlyForecast, AlertSeverity, WeatherFeature, 
    HistoricalData, RateLimiter
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

// Helper function to safely get current timestamp
fn get_current_timestamp() -> Result<i64, WeatherError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .map_err(|e| WeatherError::ConfigurationError(format!("Failed to get system time: {}", e)))
}

pub struct AccuWeatherProvider {
    api_key: String,
    base_url: String,
    rate_limiter: Arc<RateLimiter>,
    client: reqwest::Client,
}

impl AccuWeatherProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "http://dataservice.accuweather.com".to_string(),
            rate_limiter: Arc::new(RateLimiter::new(50, 3600)), // 50 requests per hour for free tier
            client: reqwest::Client::new(),
        }
    }
    
    async fn get_location_key(&self, location: &str) -> Result<String, WeatherError> {
        if !self.rate_limiter.check_rate_limit() {
            return Err(WeatherError::RateLimitExceeded);
        }
        
        let url = if location.chars().all(|c| c.is_digit(10)) {
            format!("{}/locations/v1/postalcodes/search?apikey={}&q={}", 
                self.base_url, self.api_key, location)
        } else {
            format!("{}/locations/v1/cities/search?apikey={}&q={}", 
                self.base_url, self.api_key, location)
        };
        
        let response = self.client.get(&url)
            .send()
            .await?;
            
        if response.status() == 401 {
            return Err(WeatherError::InvalidApiKey);
        }
        
        let locations: Vec<AccuLocation> = response.json().await?;
        
        locations.first()
            .map(|l| l.key.clone())
            .ok_or_else(|| WeatherError::NotFound(format!("Location not found: {}", location)))
    }
    
    async fn get_5day_forecast(&self, location_key: &str) -> Result<Vec<AccuDailyForecast>, WeatherError> {
        if !self.rate_limiter.check_rate_limit() {
            return Err(WeatherError::RateLimitExceeded);
        }
        
        let url = format!("{}/forecasts/v1/daily/5day/{}?apikey={}&metric=true", 
            self.base_url, location_key, self.api_key);
            
        let response = self.client.get(&url)
            .send()
            .await?;
            
        let forecast: AccuForecastResponse = response.json().await?;
        Ok(forecast.daily_forecasts)
    }
    
    async fn get_hourly_forecast(&self, location_key: &str) -> Result<Vec<AccuHourlyForecast>, WeatherError> {
        if !self.rate_limiter.check_rate_limit() {
            return Err(WeatherError::RateLimitExceeded);
        }
        
        let url = format!("{}/forecasts/v1/hourly/12hour/{}?apikey={}&metric=true", 
            self.base_url, location_key, self.api_key);
            
        let response = self.client.get(&url)
            .send()
            .await?;
            
        response.json().await.map_err(|e| e.into())
    }
    
    async fn get_weather_alerts(&self, location_key: &str) -> Result<Vec<AccuAlert>, WeatherError> {
        if !self.rate_limiter.check_rate_limit() {
            return Err(WeatherError::RateLimitExceeded);
        }
        
        let url = format!("{}/alerts/v1/{}?apikey={}", 
            self.base_url, location_key, self.api_key);
            
        let response = self.client.get(&url)
            .send()
            .await?;
            
        if response.status() == 204 {
            return Ok(Vec::new());
        }
        
        response.json().await.map_err(|e| e.into())
    }
    
    async fn get_location_details(&self, location_key: &str) -> Result<AccuLocation, WeatherError> {
        if !self.rate_limiter.check_rate_limit() {
            return Err(WeatherError::RateLimitExceeded);
        }
        
        let url = format!("{}/locations/v1/{}?apikey={}", 
            self.base_url, location_key, self.api_key);
            
        let response = self.client.get(&url)
            .send()
            .await?;
            
        response.json().await.map_err(|e| e.into())
    }
}

#[async_trait]
impl WeatherProvider for AccuWeatherProvider {
    async fn get_current_weather(&self, location: &str) -> Result<Weather, WeatherError> {
        let location_key = self.get_location_key(location).await?;
        
        if !self.rate_limiter.check_rate_limit() {
            return Err(WeatherError::RateLimitExceeded);
        }
        
        let url = format!("{}/currentconditions/v1/{}?apikey={}&details=true", 
            self.base_url, location_key, self.api_key);
            
        let response = self.client.get(&url)
            .send()
            .await?;
            
        let conditions: Vec<AccuCurrentCondition> = response.json().await?;
        let condition = conditions.first()
            .ok_or_else(|| WeatherError::NotFound("No current conditions available".to_string()))?;
        
        let location_details = self.get_location_details(&location_key).await?;
        
        Ok(Weather {
            temperature: condition.temperature.metric.value,
            feels_like: condition.real_feel_temperature.as_ref().map(|t| t.metric.value),
            humidity: condition.relative_humidity,
            pressure: condition.pressure.as_ref().map(|p| p.metric.value),
            wind_speed: condition.wind.as_ref().map(|w| w.speed.metric.value),
            wind_direction: condition.wind.as_ref().map(|w| w.direction.degrees),
            description: condition.weather_text.clone(),
            icon: Some(condition.weather_icon.to_string()),
            precipitation: condition.precipitation_summary.as_ref()
                .and_then(|p| p.precipitation.as_ref())
                .map(|p| p.metric.value),
            visibility: condition.visibility.as_ref().map(|v| v.metric.value),
            uv_index: condition.uv_index.map(|u| u as f64),
            provider: "AccuWeather".to_string(),
            location: Location {
                latitude: location_details.geo_position.latitude,
                longitude: location_details.geo_position.longitude,
                name: location_details.localized_name,
                country: Some(location_details.country.localized_name),
                region: location_details.administrative_area.as_ref().map(|a| a.localized_name.clone()),
                postal_code: location_details.primary_postal_code,
            },
            timestamp: get_current_timestamp()?,
        })
    }
    
    async fn get_forecast(&self, location: &str, days: u8) -> Result<Forecast, WeatherError> {
        let location_key = self.get_location_key(location).await?;
        let location_details = self.get_location_details(&location_key).await?;
        
        let daily_forecasts = self.get_5day_forecast(&location_key).await?;
        let hourly_forecasts = self.get_hourly_forecast(&location_key).await?;
        
        let daily = daily_forecasts.iter()
            .take(days as usize)
            .map(|d| DailyForecast {
                date: d.date.clone(),
                temperature_min: d.temperature.minimum.value,
                temperature_max: d.temperature.maximum.value,
                humidity: None,
                precipitation_probability: d.day.precipitation_probability,
                precipitation_amount: d.day.total_liquid.as_ref().map(|t| t.value),
                wind_speed: d.day.wind.as_ref().map(|w| w.speed.value),
                wind_direction: d.day.wind.as_ref().map(|w| w.direction.degrees),
                description: d.day.icon_phrase.clone(),
                icon: Some(d.day.icon.to_string()),
                sunrise: d.sun.as_ref().map(|s| s.rise.clone()),
                sunset: d.sun.as_ref().map(|s| s.set.clone()),
            })
            .collect();
        
        let hourly = Some(hourly_forecasts.iter()
            .map(|h| HourlyForecast {
                datetime: h.date_time.clone(),
                temperature: h.temperature.value,
                feels_like: h.real_feel_temperature.as_ref().map(|t| t.value),
                humidity: h.relative_humidity,
                precipitation_probability: Some(h.precipitation_probability as f64),
                precipitation_amount: h.total_liquid.as_ref().map(|t| t.value),
                wind_speed: h.wind.as_ref().map(|w| w.speed.value),
                wind_direction: h.wind.as_ref().map(|w| w.direction.degrees),
                description: h.icon_phrase.clone(),
                icon: Some(h.weather_icon.to_string()),
            })
            .collect());
        
        Ok(Forecast {
            location: Location {
                latitude: location_details.geo_position.latitude,
                longitude: location_details.geo_position.longitude,
                name: location_details.localized_name,
                country: Some(location_details.country.localized_name),
                region: location_details.administrative_area.as_ref().map(|a| a.localized_name.clone()),
                postal_code: location_details.primary_postal_code,
            },
            provider: "AccuWeather".to_string(),
            daily,
            hourly,
        })
    }
    
    async fn get_alerts(&self, location: &str) -> Result<Vec<Alert>, WeatherError> {
        let location_key = self.get_location_key(location).await?;
        let accu_alerts = self.get_weather_alerts(&location_key).await?;
        
        Ok(accu_alerts.iter()
            .map(|a| Alert {
                title: a.description.localized.clone(),
                description: a.description.english.clone(),
                severity: match a.severity {
                    s if s <= 3 => AlertSeverity::Minor,
                    s if s <= 6 => AlertSeverity::Moderate,
                    s if s <= 9 => AlertSeverity::Severe,
                    _ => AlertSeverity::Extreme,
                },
                start: a.effective_time_local.clone(),
                end: a.expires_time_local.clone(),
                regions: a.area.iter().map(|area| area.name.clone()).collect(),
            })
            .collect())
    }
    
    fn name(&self) -> &str {
        "AccuWeather"
    }
    
    fn supports_feature(&self, feature: WeatherFeature) -> bool {
        match feature {
            WeatherFeature::CurrentWeather => true,
            WeatherFeature::Forecast => true,
            WeatherFeature::Alerts => true,
            WeatherFeature::HourlyForecast => true,
            WeatherFeature::UvIndex => true,
            WeatherFeature::AirQuality => true,
            WeatherFeature::HistoricalData => false,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuLocation {
    key: String,
    localized_name: String,
    country: AccuCountry,
    administrative_area: Option<AccuAdminArea>,
    geo_position: AccuGeoPosition,
    primary_postal_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuCountry {
    #[serde(rename = "ID")]
    id: String,
    localized_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuAdminArea {
    localized_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuGeoPosition {
    latitude: f64,
    longitude: f64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuCurrentCondition {
    weather_text: String,
    weather_icon: i32,
    temperature: AccuTemperature,
    real_feel_temperature: Option<AccuTemperature>,
    relative_humidity: Option<f64>,
    wind: Option<AccuWind>,
    #[serde(rename = "UVIndex")]
    uv_index: Option<i32>,
    visibility: Option<AccuMeasurement>,
    pressure: Option<AccuMeasurement>,
    precipitation_summary: Option<AccuPrecipitationSummary>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuTemperature {
    metric: AccuUnit,
    imperial: AccuUnit,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuUnit {
    value: f64,
    unit: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuWind {
    direction: AccuWindDirection,
    speed: AccuMeasurement,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuWindDirection {
    degrees: f64,
    localized: String,
    english: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuMeasurement {
    metric: AccuUnit,
    imperial: AccuUnit,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuPrecipitationSummary {
    precipitation: Option<AccuMeasurement>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuForecastResponse {
    daily_forecasts: Vec<AccuDailyForecast>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuDailyForecast {
    date: String,
    temperature: AccuTempRange,
    day: AccuDayNight,
    night: AccuDayNight,
    sun: Option<AccuSun>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuTempRange {
    minimum: AccuUnit,
    maximum: AccuUnit,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuDayNight {
    icon: i32,
    icon_phrase: String,
    precipitation_probability: Option<f64>,
    total_liquid: Option<AccuUnit>,
    wind: Option<AccuWindForecast>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuWindForecast {
    speed: AccuUnit,
    direction: AccuWindDirection,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuSun {
    rise: String,
    set: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuHourlyForecast {
    date_time: String,
    weather_icon: i32,
    icon_phrase: String,
    temperature: AccuUnit,
    real_feel_temperature: Option<AccuUnit>,
    relative_humidity: Option<f64>,
    precipitation_probability: i32,
    total_liquid: Option<AccuUnit>,
    wind: Option<AccuWindForecast>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuAlert {
    description: AccuAlertDescription,
    severity: i32,
    effective_time_local: String,
    expires_time_local: Option<String>,
    area: Vec<AccuAlertArea>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuAlertDescription {
    localized: String,
    english: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct AccuAlertArea {
    name: String,
}