use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use super::common::{
    Weather, WeatherError, WeatherProvider, Forecast, Alert, Location, 
    DailyForecast, HourlyForecast, AlertSeverity, WeatherFeature, 
    HistoricalData, RateLimiter
};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

pub struct OpenWeatherProvider {
    api_key: String,
    base_url: String,
    rate_limiter: Arc<RateLimiter>,
    client: reqwest::Client,
}

impl OpenWeatherProvider {
    pub fn new(api_key: String) -> Self {
        Self {
            api_key,
            base_url: "https://api.openweathermap.org".to_string(),
            rate_limiter: Arc::new(RateLimiter::new(60, 60)), // 60 requests per minute for free tier
            client: reqwest::Client::new(),
        }
    }
    
    async fn geocode_location(&self, location: &str) -> Result<(f64, f64, String), WeatherError> {
        if !self.rate_limiter.check_rate_limit() {
            return Err(WeatherError::RateLimitExceeded);
        }
        
        let url = if location.chars().all(|c| c.is_digit(10)) {
            format!("{}/geo/1.0/zip?zip={}&appid={}", 
                self.base_url, location, self.api_key)
        } else {
            format!("{}/geo/1.0/direct?q={}&limit=1&appid={}", 
                self.base_url, location, self.api_key)
        };
        
        let response = self.client.get(&url)
            .send()
            .await?;
            
        if response.status() == 401 {
            return Err(WeatherError::InvalidApiKey);
        }
        
        let text = response.text().await?;
        
        if location.chars().all(|c| c.is_digit(10)) {
            let geo: OpenWeatherZipGeo = serde_json::from_str(&text)?;
            Ok((geo.lat, geo.lon, geo.name))
        } else {
            let geos: Vec<OpenWeatherGeo> = serde_json::from_str(&text)?;
            let geo = geos.first()
                .ok_or_else(|| WeatherError::NotFound(format!("Location not found: {}", location)))?;
            Ok((geo.lat, geo.lon, geo.name.clone()))
        }
    }
}

#[async_trait]
impl WeatherProvider for OpenWeatherProvider {
    async fn get_current_weather(&self, location: &str) -> Result<Weather, WeatherError> {
        let (lat, lon, name) = self.geocode_location(location).await?;
        
        if !self.rate_limiter.check_rate_limit() {
            return Err(WeatherError::RateLimitExceeded);
        }
        
        let url = format!("{}/data/2.5/weather?lat={}&lon={}&appid={}&units=metric", 
            self.base_url, lat, lon, self.api_key);
            
        let response = self.client.get(&url)
            .send()
            .await?;
            
        let current: OpenWeatherCurrent = response.json().await?;
        
        Ok(Weather {
            temperature: current.main.temp,
            feels_like: Some(current.main.feels_like),
            humidity: Some(current.main.humidity),
            pressure: Some(current.main.pressure),
            wind_speed: Some(current.wind.speed),
            wind_direction: current.wind.deg,
            description: current.weather.first()
                .map(|w| w.description.clone())
                .unwrap_or_default(),
            icon: current.weather.first().map(|w| w.icon.clone()),
            precipitation: current.rain.as_ref().map(|r| r.one_h.unwrap_or(0.0))
                .or_else(|| current.snow.as_ref().map(|s| s.one_h.unwrap_or(0.0))),
            visibility: current.visibility.map(|v| v as f64),
            uv_index: None,
            provider: "OpenWeather".to_string(),
            location: Location {
                latitude: lat,
                longitude: lon,
                name: name.clone(),
                country: Some(current.sys.country.clone()),
                region: None,
                postal_code: None,
            },
            timestamp: current.dt as i64,
        })
    }
    
    async fn get_forecast(&self, location: &str, days: u8) -> Result<Forecast, WeatherError> {
        let (lat, lon, name) = self.geocode_location(location).await?;
        
        if !self.rate_limiter.check_rate_limit() {
            return Err(WeatherError::RateLimitExceeded);
        }
        
        let url = format!("{}/data/3.0/onecall?lat={}&lon={}&exclude=minutely,alerts&appid={}&units=metric", 
            self.base_url, lat, lon, self.api_key);
            
        let response = self.client.get(&url)
            .send()
            .await?;
            
        if response.status() == 403 {
            // Fall back to 5-day forecast API if One Call API is not available
            return self.get_5day_forecast(location, days).await;
        }
        
        let forecast: OpenWeatherOneCall = response.json().await?;
        
        let daily = forecast.daily.iter()
            .take(days as usize)
            .map(|d| DailyForecast {
                date: format_timestamp(d.dt),
                temperature_min: d.temp.min,
                temperature_max: d.temp.max,
                humidity: Some(d.humidity),
                precipitation_probability: Some(d.pop * 100.0),
                precipitation_amount: d.rain.or(d.snow),
                wind_speed: Some(d.wind_speed),
                wind_direction: Some(d.wind_deg),
                description: d.weather.first()
                    .map(|w| w.description.clone())
                    .unwrap_or_default(),
                icon: d.weather.first().map(|w| w.icon.clone()),
                sunrise: Some(format_timestamp(d.sunrise)),
                sunset: Some(format_timestamp(d.sunset)),
            })
            .collect();
        
        let hourly = Some(forecast.hourly.iter()
            .take(48)
            .map(|h| HourlyForecast {
                datetime: format_timestamp(h.dt),
                temperature: h.temp,
                feels_like: Some(h.feels_like),
                humidity: Some(h.humidity),
                precipitation_probability: Some(h.pop * 100.0),
                precipitation_amount: h.rain.as_ref().map(|r| r.one_h.unwrap_or(0.0))
                    .or_else(|| h.snow.as_ref().map(|s| s.one_h.unwrap_or(0.0))),
                wind_speed: Some(h.wind_speed),
                wind_direction: Some(h.wind_deg),
                description: h.weather.first()
                    .map(|w| w.description.clone())
                    .unwrap_or_default(),
                icon: h.weather.first().map(|w| w.icon.clone()),
            })
            .collect());
        
        Ok(Forecast {
            location: Location {
                latitude: lat,
                longitude: lon,
                name,
                country: None,
                region: None,
                postal_code: None,
            },
            provider: "OpenWeather".to_string(),
            daily,
            hourly,
        })
    }
    
    async fn get_5day_forecast(&self, location: &str, days: u8) -> Result<Forecast, WeatherError> {
        let (lat, lon, name) = self.geocode_location(location).await?;
        
        if !self.rate_limiter.check_rate_limit() {
            return Err(WeatherError::RateLimitExceeded);
        }
        
        let url = format!("{}/data/2.5/forecast?lat={}&lon={}&appid={}&units=metric", 
            self.base_url, lat, lon, self.api_key);
            
        let response = self.client.get(&url)
            .send()
            .await?;
            
        let forecast: OpenWeather5Day = response.json().await?;
        
        let mut daily_map = std::collections::HashMap::new();
        
        for item in &forecast.list {
            let date = format_date_only(item.dt);
            let entry = daily_map.entry(date.clone()).or_insert_with(|| DailyData {
                date,
                temps: Vec::new(),
                humidity: Vec::new(),
                pop: Vec::new(),
                rain: Vec::new(),
                wind_speed: Vec::new(),
                wind_deg: Vec::new(),
                descriptions: Vec::new(),
                icons: Vec::new(),
            });
            
            entry.temps.push(item.main.temp);
            entry.humidity.push(item.main.humidity);
            entry.pop.push(item.pop * 100.0);
            if let Some(rain) = &item.rain {
                if let Some(h3) = rain.three_h {
                    entry.rain.push(h3);
                }
            }
            entry.wind_speed.push(item.wind.speed);
            if let Some(deg) = item.wind.deg {
                entry.wind_deg.push(deg);
            }
            if let Some(weather) = item.weather.first() {
                entry.descriptions.push(weather.description.clone());
                entry.icons.push(weather.icon.clone());
            }
        }
        
        let mut daily: Vec<DailyForecast> = daily_map.into_iter()
            .map(|(_, data)| DailyForecast {
                date: data.date,
                temperature_min: data.temps.iter().cloned().fold(f64::INFINITY, f64::min),
                temperature_max: data.temps.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                humidity: Some(data.humidity.iter().sum::<f64>() / data.humidity.len() as f64),
                precipitation_probability: Some(data.pop.iter().cloned().fold(0.0, f64::max)),
                precipitation_amount: if data.rain.is_empty() { None } else {
                    Some(data.rain.iter().sum())
                },
                wind_speed: Some(data.wind_speed.iter().sum::<f64>() / data.wind_speed.len() as f64),
                wind_direction: if data.wind_deg.is_empty() { None } else {
                    Some(data.wind_deg.iter().sum::<f64>() / data.wind_deg.len() as f64)
                },
                description: data.descriptions.first().cloned().unwrap_or_default(),
                icon: data.icons.first().cloned(),
                sunrise: None,
                sunset: None,
            })
            .collect();
        
        daily.sort_by(|a, b| a.date.cmp(&b.date));
        daily.truncate(days as usize);
        
        let hourly = Some(forecast.list.iter()
            .take(40)
            .map(|h| HourlyForecast {
                datetime: format_timestamp(h.dt),
                temperature: h.main.temp,
                feels_like: Some(h.main.feels_like),
                humidity: Some(h.main.humidity),
                precipitation_probability: Some(h.pop * 100.0),
                precipitation_amount: h.rain.as_ref()
                    .and_then(|r| r.three_h)
                    .or_else(|| h.snow.as_ref().and_then(|s| s.three_h)),
                wind_speed: Some(h.wind.speed),
                wind_direction: h.wind.deg,
                description: h.weather.first()
                    .map(|w| w.description.clone())
                    .unwrap_or_default(),
                icon: h.weather.first().map(|w| w.icon.clone()),
            })
            .collect());
        
        Ok(Forecast {
            location: Location {
                latitude: lat,
                longitude: lon,
                name,
                country: None,
                region: None,
                postal_code: None,
            },
            provider: "OpenWeather".to_string(),
            daily,
            hourly,
        })
    }
    
    async fn get_alerts(&self, location: &str) -> Result<Vec<Alert>, WeatherError> {
        let (lat, lon, _) = self.geocode_location(location).await?;
        
        if !self.rate_limiter.check_rate_limit() {
            return Err(WeatherError::RateLimitExceeded);
        }
        
        let url = format!("{}/data/3.0/onecall?lat={}&lon={}&exclude=current,minutely,hourly,daily&appid={}", 
            self.base_url, lat, lon, self.api_key);
            
        let response = self.client.get(&url)
            .send()
            .await?;
            
        if response.status() == 403 {
            return Ok(Vec::new());
        }
        
        let data: serde_json::Value = response.json().await?;
        
        if let Some(alerts) = data.get("alerts").and_then(|a| a.as_array()) {
            Ok(alerts.iter()
                .filter_map(|a| {
                    Some(Alert {
                        title: a.get("event")?.as_str()?.to_string(),
                        description: a.get("description")?.as_str()?.to_string(),
                        severity: AlertSeverity::Moderate,
                        start: format_timestamp(a.get("start")?.as_i64()? as i64),
                        end: a.get("end").and_then(|e| e.as_i64()).map(|e| format_timestamp(e as i64)),
                        regions: a.get("tags")
                            .and_then(|t| t.as_array())
                            .map(|tags| tags.iter()
                                .filter_map(|t| t.as_str().map(String::from))
                                .collect())
                            .unwrap_or_default(),
                    })
                })
                .collect())
        } else {
            Ok(Vec::new())
        }
    }
    
    async fn get_historical(&self, location: &str, date: &str) -> Result<HistoricalData, WeatherError> {
        let (lat, lon, name) = self.geocode_location(location).await?;
        
        let timestamp = parse_date_to_timestamp(date)
            .ok_or_else(|| WeatherError::ParseError("Invalid date format".to_string()))?;
        
        if !self.rate_limiter.check_rate_limit() {
            return Err(WeatherError::RateLimitExceeded);
        }
        
        let url = format!("{}/data/3.0/onecall/timemachine?lat={}&lon={}&dt={}&appid={}&units=metric", 
            self.base_url, lat, lon, timestamp, self.api_key);
            
        let response = self.client.get(&url)
            .send()
            .await?;
            
        if response.status() == 403 {
            return Err(WeatherError::NotFound("Historical data requires subscription".to_string()));
        }
        
        let data: OpenWeatherHistorical = response.json().await?;
        
        let temps: Vec<f64> = data.data.iter().map(|h| h.temp).collect();
        let humidities: Vec<f64> = data.data.iter().map(|h| h.humidity).collect();
        let wind_speeds: Vec<f64> = data.data.iter().map(|h| h.wind_speed).collect();
        
        Ok(HistoricalData {
            location: Location {
                latitude: lat,
                longitude: lon,
                name,
                country: None,
                region: None,
                postal_code: None,
            },
            provider: "OpenWeather".to_string(),
            date: date.to_string(),
            temperature_min: temps.iter().cloned().fold(f64::INFINITY, f64::min),
            temperature_max: temps.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
            temperature_avg: temps.iter().sum::<f64>() / temps.len() as f64,
            humidity_avg: Some(humidities.iter().sum::<f64>() / humidities.len() as f64),
            precipitation_total: None,
            wind_speed_avg: Some(wind_speeds.iter().sum::<f64>() / wind_speeds.len() as f64),
        })
    }
    
    fn name(&self) -> &str {
        "OpenWeather"
    }
    
    fn supports_feature(&self, feature: WeatherFeature) -> bool {
        match feature {
            WeatherFeature::CurrentWeather => true,
            WeatherFeature::Forecast => true,
            WeatherFeature::Alerts => true,
            WeatherFeature::HourlyForecast => true,
            WeatherFeature::UvIndex => true,
            WeatherFeature::AirQuality => true,
            WeatherFeature::HistoricalData => true,
        }
    }
}

#[derive(Debug, Deserialize)]
struct OpenWeatherGeo {
    name: String,
    lat: f64,
    lon: f64,
    country: String,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherZipGeo {
    name: String,
    lat: f64,
    lon: f64,
    country: String,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherCurrent {
    dt: i64,
    main: OpenWeatherMain,
    weather: Vec<OpenWeatherWeatherInfo>,
    wind: OpenWeatherWind,
    rain: Option<OpenWeatherPrecip>,
    snow: Option<OpenWeatherPrecip>,
    visibility: Option<i32>,
    sys: OpenWeatherSys,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherMain {
    temp: f64,
    feels_like: f64,
    pressure: f64,
    humidity: f64,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherWeatherInfo {
    description: String,
    icon: String,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherWind {
    speed: f64,
    deg: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherPrecip {
    #[serde(rename = "1h")]
    one_h: Option<f64>,
    #[serde(rename = "3h")]
    three_h: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherSys {
    country: String,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherOneCall {
    current: Option<OpenWeatherCurrentOneCall>,
    hourly: Vec<OpenWeatherHourly>,
    daily: Vec<OpenWeatherDaily>,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherCurrentOneCall {
    dt: i64,
    temp: f64,
    feels_like: f64,
    pressure: f64,
    humidity: f64,
    uvi: Option<f64>,
    visibility: Option<i32>,
    wind_speed: f64,
    wind_deg: f64,
    weather: Vec<OpenWeatherWeatherInfo>,
    rain: Option<OpenWeatherPrecip>,
    snow: Option<OpenWeatherPrecip>,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherHourly {
    dt: i64,
    temp: f64,
    feels_like: f64,
    pressure: f64,
    humidity: f64,
    uvi: Option<f64>,
    visibility: Option<i32>,
    wind_speed: f64,
    wind_deg: f64,
    pop: f64,
    weather: Vec<OpenWeatherWeatherInfo>,
    rain: Option<OpenWeatherPrecip>,
    snow: Option<OpenWeatherPrecip>,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherDaily {
    dt: i64,
    sunrise: i64,
    sunset: i64,
    temp: OpenWeatherDailyTemp,
    feels_like: Option<OpenWeatherDailyTemp>,
    pressure: f64,
    humidity: f64,
    wind_speed: f64,
    wind_deg: f64,
    weather: Vec<OpenWeatherWeatherInfo>,
    pop: f64,
    rain: Option<f64>,
    snow: Option<f64>,
    uvi: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherDailyTemp {
    min: f64,
    max: f64,
}

#[derive(Debug, Deserialize)]
struct OpenWeather5Day {
    list: Vec<OpenWeather5DayItem>,
}

#[derive(Debug, Deserialize)]
struct OpenWeather5DayItem {
    dt: i64,
    main: OpenWeatherMain,
    weather: Vec<OpenWeatherWeatherInfo>,
    wind: OpenWeatherWind,
    pop: f64,
    rain: Option<OpenWeatherPrecip>,
    snow: Option<OpenWeatherPrecip>,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherHistorical {
    data: Vec<OpenWeatherHistoricalHour>,
}

#[derive(Debug, Deserialize)]
struct OpenWeatherHistoricalHour {
    dt: i64,
    temp: f64,
    humidity: f64,
    wind_speed: f64,
}

struct DailyData {
    date: String,
    temps: Vec<f64>,
    humidity: Vec<f64>,
    pop: Vec<f64>,
    rain: Vec<f64>,
    wind_speed: Vec<f64>,
    wind_deg: Vec<f64>,
    descriptions: Vec<String>,
    icons: Vec<String>,
}

fn format_timestamp(ts: i64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let d = UNIX_EPOCH + Duration::from_secs(ts as u64);
    format!("{:?}", d)
}

fn format_date_only(ts: i64) -> String {
    format_timestamp(ts).split('T').next().unwrap_or_default().to_string()
}

fn parse_date_to_timestamp(date: &str) -> Option<i64> {
    None
}