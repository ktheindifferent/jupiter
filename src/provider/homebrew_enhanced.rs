use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use super::common::{
    Weather, WeatherError, WeatherProvider, Forecast, Alert, Location, 
    DailyForecast, HourlyForecast, AlertSeverity, WeatherFeature, 
    HistoricalData, RateLimiter
};
use std::sync::Arc;
use crate::provider::homebrew::{Config, WeatherReport, PostgresServer};
use crate::utils::time::safe_timestamp_with_fallback;
use std::collections::HashMap;

// Helper function to safely get current timestamp
fn get_current_timestamp() -> Result<i64, WeatherError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .map_err(|e| WeatherError::ConfigurationError(format!("Failed to get system time: {}", e)))
}

pub struct HomebrewProvider {
    config: Config,
    location_mappings: HashMap<String, LocationInfo>,
}

#[derive(Clone)]
struct LocationInfo {
    latitude: f64,
    longitude: f64,
    name: String,
    device_types: Vec<String>,
}

impl HomebrewProvider {
    pub fn new(config: Config) -> Self {
        let mut location_mappings = HashMap::new();
        
        location_mappings.insert("default".to_string(), LocationInfo {
            latitude: 0.0,
            longitude: 0.0,
            name: "Default Location".to_string(),
            device_types: vec!["indoor".to_string(), "outdoor".to_string()],
        });
        
        Self {
            config,
            location_mappings,
        }
    }
    
    pub fn add_location_mapping(&mut self, key: String, lat: f64, lon: f64, name: String, device_types: Vec<String>) {
        self.location_mappings.insert(key, LocationInfo {
            latitude: lat,
            longitude: lon,
            name,
            device_types,
        });
    }
    
    fn get_location_info(&self, location: &str) -> Result<LocationInfo, WeatherError> {
        self.location_mappings
            .get(location)
            .or_else(|| self.location_mappings.get("default"))
            .cloned()
            .ok_or_else(|| WeatherError::NotFound(format!("Location not configured: {}", location)))
    }
    
    async fn get_latest_reports(&self, device_type: Option<&str>, limit: usize) -> Result<Vec<WeatherReport>, WeatherError> {
        let filter = device_type.map(|dt| crate::provider::homebrew::FilterParams {
            oid: None,
        });
        
        WeatherReport::select(self.config.clone(), Some(limit), None, Some("timestamp".to_string()), filter)
            .map_err(|e| WeatherError::DatabaseError(e.to_string()))
    }
    
    async fn get_aggregated_data(&self, device_types: &[String]) -> Result<AggregatedData, WeatherError> {
        let mut all_reports = Vec::new();
        
        for device_type in device_types {
            let reports = self.get_latest_reports(Some(device_type), 10).await?;
            all_reports.extend(reports);
        }
        
        if all_reports.is_empty() {
            return Err(WeatherError::NotFound("No data available".to_string()));
        }
        
        let now = get_current_timestamp()?;
        let recent_reports: Vec<_> = all_reports.iter()
            .filter(|r| {
                now - r.timestamp < 3600
            })
            .collect();
        
        if recent_reports.is_empty() {
            return Err(WeatherError::NotFound("No recent data available".to_string()));
        }
        
        let temperatures: Vec<f64> = recent_reports.iter()
            .filter_map(|r| r.temperature)
            .collect();
        
        let humidities: Vec<f64> = recent_reports.iter()
            .filter_map(|r| r.humidity)
            .collect();
        
        let precipitations: Vec<f64> = recent_reports.iter()
            .filter_map(|r| r.percipitation)
            .collect();
        
        let pm25s: Vec<f64> = recent_reports.iter()
            .filter_map(|r| r.pm25)
            .collect();
        
        let pm10s: Vec<f64> = recent_reports.iter()
            .filter_map(|r| r.pm10)
            .collect();
        
        let co2s: Vec<f64> = recent_reports.iter()
            .filter_map(|r| r.co2)
            .collect();
        
        let tvocs: Vec<f64> = recent_reports.iter()
            .filter_map(|r| r.tvoc)
            .collect();
        
        Ok(AggregatedData {
            temperature: if temperatures.is_empty() { None } else {
                Some(temperatures.iter().sum::<f64>() / temperatures.len() as f64)
            },
            humidity: if humidities.is_empty() { None } else {
                Some(humidities.iter().sum::<f64>() / humidities.len() as f64)
            },
            precipitation: if precipitations.is_empty() { None } else {
                Some(precipitations.iter().sum::<f64>())
            },
            pm25: if pm25s.is_empty() { None } else {
                Some(pm25s.iter().sum::<f64>() / pm25s.len() as f64)
            },
            pm10: if pm10s.is_empty() { None } else {
                Some(pm10s.iter().sum::<f64>() / pm10s.len() as f64)
            },
            co2: if co2s.is_empty() { None } else {
                Some(co2s.iter().sum::<f64>() / co2s.len() as f64)
            },
            tvoc: if tvocs.is_empty() { None } else {
                Some(tvocs.iter().sum::<f64>() / tvocs.len() as f64)
            },
            count: recent_reports.len(),
        })
    }
    
    async fn get_historical_aggregated(&self, device_types: &[String], days: u8) -> Result<Vec<DailyAggregatedData>, WeatherError> {
        let mut daily_data = HashMap::new();
        let now = safe_timestamp_with_fallback();
        let start_time = now - (days as i64 * 86400);
        
        for device_type in device_types {
            let reports = self.get_latest_reports(Some(device_type), 1000).await?;
            
            for report in reports {
                if report.timestamp >= start_time {
                    let day = report.timestamp / 86400;
                    let entry = daily_data.entry(day).or_insert_with(|| DailyAggregatedData {
                        date: format_timestamp(day * 86400),
                        temperatures: Vec::new(),
                        humidities: Vec::new(),
                        precipitations: Vec::new(),
                        pm25s: Vec::new(),
                        pm10s: Vec::new(),
                        co2s: Vec::new(),
                        tvocs: Vec::new(),
                    });
                    
                    if let Some(temp) = report.temperature {
                        entry.temperatures.push(temp);
                    }
                    if let Some(hum) = report.humidity {
                        entry.humidities.push(hum);
                    }
                    if let Some(precip) = report.percipitation {
                        entry.precipitations.push(precip);
                    }
                    if let Some(pm25) = report.pm25 {
                        entry.pm25s.push(pm25);
                    }
                    if let Some(pm10) = report.pm10 {
                        entry.pm10s.push(pm10);
                    }
                    if let Some(co2) = report.co2 {
                        entry.co2s.push(co2);
                    }
                    if let Some(tvoc) = report.tvoc {
                        entry.tvocs.push(tvoc);
                    }
                }
            }
        }
        
        let mut result: Vec<_> = daily_data.into_iter()
            .map(|(_, data)| data)
            .collect();
        
        result.sort_by(|a, b| a.date.cmp(&b.date));
        
        Ok(result)
    }
}

#[async_trait]
impl WeatherProvider for HomebrewProvider {
    async fn get_current_weather(&self, location: &str) -> Result<Weather, WeatherError> {
        let location_info = self.get_location_info(location)?;
        let aggregated = self.get_aggregated_data(&location_info.device_types).await?;
        
        let description = format!(
            "Homebrew Weather Station - {} sensors reporting",
            aggregated.count
        );
        
        let mut extra_info = Vec::new();
        if let Some(pm25) = aggregated.pm25 {
            extra_info.push(format!("PM2.5: {:.1} µg/m³", pm25));
        }
        if let Some(pm10) = aggregated.pm10 {
            extra_info.push(format!("PM10: {:.1} µg/m³", pm10));
        }
        if let Some(co2) = aggregated.co2 {
            extra_info.push(format!("CO2: {:.0} ppm", co2));
        }
        if let Some(tvoc) = aggregated.tvoc {
            extra_info.push(format!("TVOC: {:.0} ppb", tvoc));
        }
        
        let full_description = if extra_info.is_empty() {
            description
        } else {
            format!("{} | {}", description, extra_info.join(", "))
        };
        
        Ok(Weather {
            temperature: aggregated.temperature.unwrap_or(0.0),
            feels_like: None,
            humidity: aggregated.humidity,
            pressure: None,
            wind_speed: None,
            wind_direction: None,
            description: full_description,
            icon: None,
            precipitation: aggregated.precipitation,
            visibility: None,
            uv_index: None,
            provider: "Homebrew".to_string(),
            location: Location {
                latitude: location_info.latitude,
                longitude: location_info.longitude,
                name: location_info.name,
                country: None,
                region: None,
                postal_code: None,
            },
            timestamp: safe_timestamp_with_fallback(),
        })
    }
    
    async fn get_forecast(&self, location: &str, days: u8) -> Result<Forecast, WeatherError> {
        let location_info = self.get_location_info(location)?;
        let historical = self.get_historical_aggregated(&location_info.device_types, days).await?;
        
        let daily = historical.iter()
            .map(|day| {
                let temp_min = day.temperatures.iter().cloned().fold(f64::INFINITY, f64::min);
                let temp_max = day.temperatures.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                let humidity_avg = if day.humidities.is_empty() { None } else {
                    Some(day.humidities.iter().sum::<f64>() / day.humidities.len() as f64)
                };
                let precipitation_total = if day.precipitations.is_empty() { None } else {
                    Some(day.precipitations.iter().sum())
                };
                
                DailyForecast {
                    date: day.date.clone(),
                    temperature_min: if temp_min.is_finite() { temp_min } else { 0.0 },
                    temperature_max: if temp_max.is_finite() { temp_max } else { 0.0 },
                    humidity: humidity_avg,
                    precipitation_probability: None,
                    precipitation_amount: precipitation_total,
                    wind_speed: None,
                    wind_direction: None,
                    description: "Homebrew historical data".to_string(),
                    icon: None,
                    sunrise: None,
                    sunset: None,
                }
            })
            .collect();
        
        Ok(Forecast {
            location: Location {
                latitude: location_info.latitude,
                longitude: location_info.longitude,
                name: location_info.name,
                country: None,
                region: None,
                postal_code: None,
            },
            provider: "Homebrew".to_string(),
            daily,
            hourly: None,
        })
    }
    
    async fn get_alerts(&self, _location: &str) -> Result<Vec<Alert>, WeatherError> {
        let outdoor_data = self.get_aggregated_data(&vec!["outdoor".to_string()]).await.ok();
        let indoor_data = self.get_aggregated_data(&vec!["indoor".to_string()]).await.ok();
        
        let mut alerts = Vec::new();
        
        if let Some(data) = &outdoor_data {
            if let Some(pm25) = data.pm25 {
                if pm25 > 35.0 {
                    alerts.push(Alert {
                        title: "Poor Air Quality (PM2.5)".to_string(),
                        description: format!("PM2.5 levels are elevated at {:.1} µg/m³", pm25),
                        severity: if pm25 > 55.0 { AlertSeverity::Severe } else { AlertSeverity::Moderate },
                        start: format_timestamp(safe_timestamp_with_fallback()),
                        end: None,
                        regions: vec!["Outdoor".to_string()],
                    });
                }
            }
        }
        
        if let Some(data) = &indoor_data {
            if let Some(co2) = data.co2 {
                if co2 > 1000.0 {
                    alerts.push(Alert {
                        title: "High CO2 Levels".to_string(),
                        description: format!("Indoor CO2 levels are elevated at {:.0} ppm", co2),
                        severity: if co2 > 2000.0 { AlertSeverity::Severe } else { AlertSeverity::Moderate },
                        start: format_timestamp(safe_timestamp_with_fallback()),
                        end: None,
                        regions: vec!["Indoor".to_string()],
                    });
                }
            }
            
            if let Some(tvoc) = data.tvoc {
                if tvoc > 500.0 {
                    alerts.push(Alert {
                        title: "High TVOC Levels".to_string(),
                        description: format!("Indoor TVOC levels are elevated at {:.0} ppb", tvoc),
                        severity: if tvoc > 1000.0 { AlertSeverity::Severe } else { AlertSeverity::Moderate },
                        start: format_timestamp(safe_timestamp_with_fallback()),
                        end: None,
                        regions: vec!["Indoor".to_string()],
                    });
                }
            }
        }
        
        Ok(alerts)
    }
    
    async fn get_historical(&self, location: &str, date: &str) -> Result<HistoricalData, WeatherError> {
        let location_info = self.get_location_info(location)?;
        let timestamp = parse_date_to_timestamp(date)
            .ok_or_else(|| WeatherError::ParseError("Invalid date format".to_string()))?;
        
        let start_time = timestamp;
        let end_time = timestamp + 86400;
        
        let mut temperatures = Vec::new();
        let mut humidities = Vec::new();
        let mut precipitations = Vec::new();
        
        for device_type in &location_info.device_types {
            let reports = self.get_latest_reports(Some(device_type), 1000).await?;
            
            for report in reports {
                if report.timestamp >= start_time && report.timestamp < end_time {
                    if let Some(temp) = report.temperature {
                        temperatures.push(temp);
                    }
                    if let Some(hum) = report.humidity {
                        humidities.push(hum);
                    }
                    if let Some(precip) = report.percipitation {
                        precipitations.push(precip);
                    }
                }
            }
        }
        
        if temperatures.is_empty() {
            return Err(WeatherError::NotFound(format!("No data available for date: {}", date)));
        }
        
        Ok(HistoricalData {
            location: Location {
                latitude: location_info.latitude,
                longitude: location_info.longitude,
                name: location_info.name,
                country: None,
                region: None,
                postal_code: None,
            },
            provider: "Homebrew".to_string(),
            date: date.to_string(),
            temperature_min: temperatures.iter().cloned().fold(f64::INFINITY, f64::min),
            temperature_max: temperatures.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            temperature_avg: temperatures.iter().sum::<f64>() / temperatures.len() as f64,
            humidity_avg: if humidities.is_empty() { None } else {
                Some(humidities.iter().sum::<f64>() / humidities.len() as f64)
            },
            precipitation_total: if precipitations.is_empty() { None } else {
                Some(precipitations.iter().sum())
            },
            wind_speed_avg: None,
        })
    }
    
    fn name(&self) -> &str {
        "Homebrew"
    }
    
    fn supports_feature(&self, feature: WeatherFeature) -> bool {
        match feature {
            WeatherFeature::CurrentWeather => true,
            WeatherFeature::Forecast => false,
            WeatherFeature::Alerts => true,
            WeatherFeature::HistoricalData => true,
            WeatherFeature::HourlyForecast => false,
            WeatherFeature::UvIndex => false,
            WeatherFeature::AirQuality => true,
        }
    }
}

struct AggregatedData {
    temperature: Option<f64>,
    humidity: Option<f64>,
    precipitation: Option<f64>,
    pm25: Option<f64>,
    pm10: Option<f64>,
    co2: Option<f64>,
    tvoc: Option<f64>,
    count: usize,
}

struct DailyAggregatedData {
    date: String,
    temperatures: Vec<f64>,
    humidities: Vec<f64>,
    precipitations: Vec<f64>,
    pm25s: Vec<f64>,
    pm10s: Vec<f64>,
    co2s: Vec<f64>,
    tvocs: Vec<f64>,
}

fn format_timestamp(ts: i64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let d = UNIX_EPOCH + Duration::from_secs(ts as u64);
    format!("{:?}", d)
}

fn parse_date_to_timestamp(date: &str) -> Option<i64> {
    None
}

pub async fn create_weather_report(
    config: Config,
    temperature: Option<f64>,
    humidity: Option<f64>,
    percipitation: Option<f64>,
    pm10: Option<f64>,
    pm25: Option<f64>,
    co2: Option<f64>,
    tvoc: Option<f64>,
    device_type: String,
) -> Result<WeatherReport, WeatherError> {
    let mut report = WeatherReport::new();
    report.temperature = temperature;
    report.humidity = humidity;
    report.percipitation = percipitation;
    report.pm10 = pm10;
    report.pm25 = pm25;
    report.co2 = co2;
    report.tvoc = tvoc;
    report.device_type = device_type;
    
    report.save(config)
        .map_err(|e| WeatherError::DatabaseError(e.to_string()))?;
    Ok(report)
}

pub async fn get_latest_weather_report(config: Config) -> Result<Option<WeatherReport>, WeatherError> {
    WeatherReport::select(config, Some(1), None, Some("timestamp".to_string()), None)
        .map(|reports| reports.into_iter().next())
        .map_err(|e| WeatherError::DatabaseError(e.to_string()))
}

pub async fn get_weather_reports_by_device(
    config: Config,
    device_type: String,
    limit: usize,
) -> Result<Vec<WeatherReport>, WeatherError> {
    let filter = crate::provider::homebrew::FilterParams {
        oid: None,
    };
    
    WeatherReport::select(config, Some(limit), None, Some("timestamp".to_string()), Some(filter))
        .map_err(|e| WeatherError::DatabaseError(e.to_string()))
}