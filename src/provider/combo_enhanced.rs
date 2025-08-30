use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use super::common::{
    Weather, WeatherError, WeatherProvider, Forecast, Alert, Location, 
    DailyForecast, HourlyForecast, AlertSeverity, WeatherFeature, 
    HistoricalData
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use std::collections::HashMap;

pub struct ComboProvider {
    providers: Vec<Box<dyn WeatherProvider>>,
    weights: HashMap<String, f64>,
    cache: Arc<RwLock<WeatherCache>>,
    cache_duration_secs: u64,
    fallback_enabled: bool,
}

impl ComboProvider {
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            weights: HashMap::new(),
            cache: Arc::new(RwLock::new(WeatherCache::new())),
            cache_duration_secs: 300,
            fallback_enabled: true,
        }
    }
    
    pub fn add_provider(mut self, provider: Box<dyn WeatherProvider>, weight: f64) -> Self {
        let name = provider.name().to_string();
        self.providers.push(provider);
        self.weights.insert(name, weight);
        self
    }
    
    pub fn set_cache_duration(mut self, seconds: u64) -> Self {
        self.cache_duration_secs = seconds;
        self
    }
    
    pub fn set_fallback_enabled(mut self, enabled: bool) -> Self {
        self.fallback_enabled = enabled;
        self
    }
    
    async fn get_from_cache(&self, key: &str) -> Option<serde_json::Value> {
        let cache = self.cache.read().await;
        cache.get(key, self.cache_duration_secs)
    }
    
    async fn store_in_cache(&self, key: &str, value: serde_json::Value) {
        let mut cache = self.cache.write().await;
        cache.set(key.to_string(), value);
    }
    
    fn average_weather(&self, weathers: Vec<(String, Weather)>) -> Result<Weather, WeatherError> {
        if weathers.is_empty() {
            return Err(WeatherError::NotFound("No weather data available from any provider".to_string()));
        }
        
        let total_weight: f64 = weathers.iter()
            .map(|(name, _)| self.weights.get(name).unwrap_or(&1.0))
            .sum();
        
        let mut avg_temp = 0.0;
        let mut avg_feels_like = 0.0;
        let mut avg_humidity = 0.0;
        let mut avg_pressure = 0.0;
        let mut avg_wind_speed = 0.0;
        let mut avg_wind_direction = 0.0;
        let mut avg_precipitation = 0.0;
        let mut avg_visibility = 0.0;
        let mut avg_uv = 0.0;
        
        let mut feels_like_count = 0.0;
        let mut humidity_count = 0.0;
        let mut pressure_count = 0.0;
        let mut wind_speed_count = 0.0;
        let mut wind_direction_count = 0.0;
        let mut precipitation_count = 0.0;
        let mut visibility_count = 0.0;
        let mut uv_count = 0.0;
        
        let mut descriptions = Vec::new();
        let mut location = None;
        
        for (name, weather) in &weathers {
            let weight = self.weights.get(name).unwrap_or(&1.0);
            
            avg_temp += weather.temperature * weight;
            
            if let Some(val) = weather.feels_like {
                avg_feels_like += val * weight;
                feels_like_count += weight;
            }
            if let Some(val) = weather.humidity {
                avg_humidity += val * weight;
                humidity_count += weight;
            }
            if let Some(val) = weather.pressure {
                avg_pressure += val * weight;
                pressure_count += weight;
            }
            if let Some(val) = weather.wind_speed {
                avg_wind_speed += val * weight;
                wind_speed_count += weight;
            }
            if let Some(val) = weather.wind_direction {
                avg_wind_direction += val * weight;
                wind_direction_count += weight;
            }
            if let Some(val) = weather.precipitation {
                avg_precipitation += val * weight;
                precipitation_count += weight;
            }
            if let Some(val) = weather.visibility {
                avg_visibility += val * weight;
                visibility_count += weight;
            }
            if let Some(val) = weather.uv_index {
                avg_uv += val * weight;
                uv_count += weight;
            }
            
            descriptions.push(format!("{}: {}", name, weather.description));
            
            if location.is_none() {
                location = Some(weather.location.clone());
            }
        }
        
        Ok(Weather {
            temperature: avg_temp / total_weight,
            feels_like: if feels_like_count > 0.0 { Some(avg_feels_like / feels_like_count) } else { None },
            humidity: if humidity_count > 0.0 { Some(avg_humidity / humidity_count) } else { None },
            pressure: if pressure_count > 0.0 { Some(avg_pressure / pressure_count) } else { None },
            wind_speed: if wind_speed_count > 0.0 { Some(avg_wind_speed / wind_speed_count) } else { None },
            wind_direction: if wind_direction_count > 0.0 { Some(avg_wind_direction / wind_direction_count) } else { None },
            description: format!("Combined: {}", descriptions.join(" | ")),
            icon: None,
            precipitation: if precipitation_count > 0.0 { Some(avg_precipitation / precipitation_count) } else { None },
            visibility: if visibility_count > 0.0 { Some(avg_visibility / visibility_count) } else { None },
            uv_index: if uv_count > 0.0 { Some(avg_uv / uv_count) } else { None },
            provider: "Combo".to_string(),
            location: location.unwrap_or_else(|| Location {
                latitude: 0.0,
                longitude: 0.0,
                name: "Unknown".to_string(),
                country: None,
                region: None,
                postal_code: None,
            }),
            timestamp: SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64,
        })
    }
    
    fn combine_forecasts(&self, forecasts: Vec<(String, Forecast)>) -> Result<Forecast, WeatherError> {
        if forecasts.is_empty() {
            return Err(WeatherError::NotFound("No forecast data available from any provider".to_string()));
        }
        
        let mut daily_map: HashMap<String, Vec<(String, DailyForecast)>> = HashMap::new();
        let mut hourly_map: HashMap<String, Vec<(String, HourlyForecast)>> = HashMap::new();
        let mut location = None;
        
        for (provider_name, forecast) in &forecasts {
            if location.is_none() {
                location = Some(forecast.location.clone());
            }
            
            for daily in &forecast.daily {
                daily_map.entry(daily.date.clone())
                    .or_insert_with(Vec::new)
                    .push((provider_name.clone(), daily.clone()));
            }
            
            if let Some(hourly_data) = &forecast.hourly {
                for hourly in hourly_data {
                    hourly_map.entry(hourly.datetime.clone())
                        .or_insert_with(Vec::new)
                        .push((provider_name.clone(), hourly.clone()));
                }
            }
        }
        
        let mut combined_daily: Vec<DailyForecast> = daily_map.into_iter()
            .map(|(date, provider_forecasts)| {
                let total_weight: f64 = provider_forecasts.iter()
                    .map(|(name, _)| self.weights.get(name).unwrap_or(&1.0))
                    .sum();
                
                let mut avg = DailyForecast {
                    date,
                    temperature_min: 0.0,
                    temperature_max: 0.0,
                    humidity: None,
                    precipitation_probability: None,
                    precipitation_amount: None,
                    wind_speed: None,
                    wind_direction: None,
                    description: String::new(),
                    icon: None,
                    sunrise: None,
                    sunset: None,
                };
                
                let mut humidity_sum = 0.0;
                let mut humidity_count = 0.0;
                let mut precip_prob_sum = 0.0;
                let mut precip_prob_count = 0.0;
                let mut precip_amt_sum = 0.0;
                let mut precip_amt_count = 0.0;
                let mut wind_speed_sum = 0.0;
                let mut wind_speed_count = 0.0;
                let mut wind_dir_sum = 0.0;
                let mut wind_dir_count = 0.0;
                
                for (name, forecast) in &provider_forecasts {
                    let weight = self.weights.get(name).unwrap_or(&1.0);
                    
                    avg.temperature_min += forecast.temperature_min * weight;
                    avg.temperature_max += forecast.temperature_max * weight;
                    
                    if let Some(val) = forecast.humidity {
                        humidity_sum += val * weight;
                        humidity_count += weight;
                    }
                    if let Some(val) = forecast.precipitation_probability {
                        precip_prob_sum += val * weight;
                        precip_prob_count += weight;
                    }
                    if let Some(val) = forecast.precipitation_amount {
                        precip_amt_sum += val * weight;
                        precip_amt_count += weight;
                    }
                    if let Some(val) = forecast.wind_speed {
                        wind_speed_sum += val * weight;
                        wind_speed_count += weight;
                    }
                    if let Some(val) = forecast.wind_direction {
                        wind_dir_sum += val * weight;
                        wind_dir_count += weight;
                    }
                    
                    if avg.sunrise.is_none() {
                        avg.sunrise = forecast.sunrise.clone();
                    }
                    if avg.sunset.is_none() {
                        avg.sunset = forecast.sunset.clone();
                    }
                }
                
                avg.temperature_min /= total_weight;
                avg.temperature_max /= total_weight;
                avg.humidity = if humidity_count > 0.0 { Some(humidity_sum / humidity_count) } else { None };
                avg.precipitation_probability = if precip_prob_count > 0.0 { Some(precip_prob_sum / precip_prob_count) } else { None };
                avg.precipitation_amount = if precip_amt_count > 0.0 { Some(precip_amt_sum / precip_amt_count) } else { None };
                avg.wind_speed = if wind_speed_count > 0.0 { Some(wind_speed_sum / wind_speed_count) } else { None };
                avg.wind_direction = if wind_dir_count > 0.0 { Some(wind_dir_sum / wind_dir_count) } else { None };
                avg.description = "Combined forecast".to_string();
                
                avg
            })
            .collect();
        
        combined_daily.sort_by(|a, b| a.date.cmp(&b.date));
        
        let combined_hourly = if !hourly_map.is_empty() {
            let mut hourly: Vec<HourlyForecast> = hourly_map.into_iter()
                .map(|(datetime, provider_forecasts)| {
                    let total_weight: f64 = provider_forecasts.iter()
                        .map(|(name, _)| self.weights.get(name).unwrap_or(&1.0))
                        .sum();
                    
                    let mut avg = HourlyForecast {
                        datetime,
                        temperature: 0.0,
                        feels_like: None,
                        humidity: None,
                        precipitation_probability: None,
                        precipitation_amount: None,
                        wind_speed: None,
                        wind_direction: None,
                        description: "Combined".to_string(),
                        icon: None,
                    };
                    
                    for (name, forecast) in &provider_forecasts {
                        let weight = self.weights.get(name).unwrap_or(&1.0);
                        avg.temperature += forecast.temperature * weight;
                    }
                    avg.temperature /= total_weight;
                    
                    avg
                })
                .collect();
            
            hourly.sort_by(|a, b| a.datetime.cmp(&b.datetime));
            Some(hourly)
        } else {
            None
        };
        
        Ok(Forecast {
            location: location.unwrap_or_else(|| Location {
                latitude: 0.0,
                longitude: 0.0,
                name: "Unknown".to_string(),
                country: None,
                region: None,
                postal_code: None,
            }),
            provider: "Combo".to_string(),
            daily: combined_daily,
            hourly: combined_hourly,
        })
    }
    
    fn merge_alerts(&self, alerts_list: Vec<(String, Vec<Alert>)>) -> Vec<Alert> {
        let mut all_alerts = Vec::new();
        let mut seen_alerts = std::collections::HashSet::new();
        
        for (provider, alerts) in alerts_list {
            for mut alert in alerts {
                let alert_key = format!("{}-{}", alert.title, alert.start);
                if !seen_alerts.contains(&alert_key) {
                    seen_alerts.insert(alert_key);
                    alert.title = format!("[{}] {}", provider, alert.title);
                    all_alerts.push(alert);
                }
            }
        }
        
        all_alerts.sort_by(|a, b| b.severity.cmp(&a.severity));
        all_alerts
    }
}

#[async_trait]
impl WeatherProvider for ComboProvider {
    async fn get_current_weather(&self, location: &str) -> Result<Weather, WeatherError> {
        let cache_key = format!("current:{}", location);
        
        if let Some(cached) = self.get_from_cache(&cache_key).await {
            if let Ok(weather) = serde_json::from_value::<Weather>(cached) {
                return Ok(weather);
            }
        }
        
        let mut results = Vec::new();
        for provider in &self.providers {
            let provider_name = provider.name().to_string();
            match provider.get_current_weather(location).await {
                Ok(data) => {
                    results.push((provider_name, data));
                    if !self.fallback_enabled {
                        break;
                    }
                }
                Err(e) => {
                    log::error!("Provider {} failed: {:?}", provider_name, e);
                }
            }
        }
        
        let weather = self.average_weather(results)?;
        
        if let Ok(json_value) = serde_json::to_value(&weather) {
            self.store_in_cache(&cache_key, json_value).await;
        }
        
        Ok(weather)
    }
    
    async fn get_forecast(&self, location: &str, days: u8) -> Result<Forecast, WeatherError> {
        let cache_key = format!("forecast:{}:{}", location, days);
        
        if let Some(cached) = self.get_from_cache(&cache_key).await {
            if let Ok(forecast) = serde_json::from_value::<Forecast>(cached) {
                return Ok(forecast);
            }
        }
        
        let mut results = Vec::new();
        for provider in &self.providers {
            if provider.supports_feature(WeatherFeature::Forecast) {
                let provider_name = provider.name().to_string();
                match provider.get_forecast(location, days).await {
                    Ok(data) => {
                        results.push((provider_name, data));
                        if !self.fallback_enabled {
                            break;
                        }
                    }
                    Err(e) => {
                        log::error!("Provider {} failed: {:?}", provider_name, e);
                    }
                }
            }
        }
        
        let forecast = self.combine_forecasts(results)?;
        
        if let Ok(json_value) = serde_json::to_value(&forecast) {
            self.store_in_cache(&cache_key, json_value).await;
        }
        
        Ok(forecast)
    }
    
    async fn get_alerts(&self, location: &str) -> Result<Vec<Alert>, WeatherError> {
        let cache_key = format!("alerts:{}", location);
        
        if let Some(cached) = self.get_from_cache(&cache_key).await {
            if let Ok(alerts) = serde_json::from_value::<Vec<Alert>>(cached) {
                return Ok(alerts);
            }
        }
        
        let mut results = Vec::new();
        for provider in &self.providers {
            if provider.supports_feature(WeatherFeature::Alerts) {
                let provider_name = provider.name().to_string();
                match provider.get_alerts(location).await {
                    Ok(data) => {
                        results.push((provider_name, data));
                    }
                    Err(e) => {
                        log::error!("Provider {} failed: {:?}", provider_name, e);
                    }
                }
            }
        }
        
        let alerts = self.merge_alerts(results);
        
        if let Ok(json_value) = serde_json::to_value(&alerts) {
            self.store_in_cache(&cache_key, json_value).await;
        }
        
        Ok(alerts)
    }
    
    async fn get_historical(&self, location: &str, date: &str) -> Result<HistoricalData, WeatherError> {
        let mut results = Vec::new();
        for provider in &self.providers {
            if provider.supports_feature(WeatherFeature::HistoricalData) {
                let provider_name = provider.name().to_string();
                match provider.get_historical(location, date).await {
                    Ok(data) => {
                        results.push((provider_name, data));
                        if !self.fallback_enabled {
                            break;
                        }
                    }
                    Err(e) => {
                        log::error!("Provider {} failed: {:?}", provider_name, e);
                    }
                }
            }
        }
        
        if results.is_empty() {
            return Err(WeatherError::NotFound("No historical data available".to_string()));
        }
        
        let first = results.first().unwrap();
        Ok(first.1.clone())
    }
    
    fn name(&self) -> &str {
        "Combo"
    }
    
    fn supports_feature(&self, feature: WeatherFeature) -> bool {
        self.providers.iter().any(|p| p.supports_feature(feature))
    }
}


struct WeatherCache {
    data: HashMap<String, CacheEntry>,
}

struct CacheEntry {
    value: serde_json::Value,
    timestamp: u64,
}

impl WeatherCache {
    fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
    
    fn get(&self, key: &str, ttl_secs: u64) -> Option<serde_json::Value> {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        
        self.data.get(key).and_then(|entry| {
            if now - entry.timestamp < ttl_secs {
                Some(entry.value.clone())
            } else {
                None
            }
        })
    }
    
    fn set(&mut self, key: String, value: serde_json::Value) {
        let timestamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        self.data.insert(key, CacheEntry { value, timestamp });
    }
}