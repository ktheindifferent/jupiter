
// TODO - Full Acuweather API Support
// - Locations API
// - Forecast API

// TODO - Full OpenWeather API Support

// TODO - Homebrew API for homebrew weather monitoring

// TODO - Combo API for averaging results between multiple providers and reducing paid API calls
extern crate postgres;
pub mod provider;
pub mod auth;
pub mod ssl_config;
pub mod input_sanitizer;
pub mod db_pool;
pub mod pool_monitor;
pub mod config;
pub mod error;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod security_tests;
#[cfg(test)]
mod db_pool_tests;


// https://api.openweathermap.org/data/3.0/onecall?lat={lat}&lon={lon}&exclude={part}&appid={API key}
// https://api.openweathermap.org/data/3.0/onecall?lat=33.44&lon=-94.04&exclude=hourly,daily&appid={API key}
// units
// lang

// use serde_derive::Deserialize;
// use serde_derive::Serialize;

// #[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
// #[serde(rename_all = "camelCase")]
// pub struct Error {
//     pub cod: i64,
//     pub message: String,
// }
