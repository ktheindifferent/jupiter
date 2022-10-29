extern crate jupiter;

use jupiter::provider::accuweather::Location;
use jupiter::provider::accuweather::Forecast;
use jupiter::provider::accuweather::CurrentCondition;
use jupiter::provider::accuweather;
use jupiter::provider::homebrew;
use jupiter::provider::combo;
use std::env;

// store application version as a const
const VERSION: Option<&'static str> = option_env!("CARGO_PKG_VERSION");


#[tokio::main]
async fn main() {
    println!("Hello, world!");

    let accu_key = env::var("ACCUWEATHERKEY").expect("$ACCUWEATHERKEY is not set");
    let zip_code = env::var("ZIP_CODE").expect("$ZIP_CODE is not set");

    // Acuweather example
    let accuweather_config = accuweather::Config{
        apikey: String::from(accu_key.clone()),
        language: None,
        details: None,
        metric: None
    };
    // let location = Location::search_by_zip(accuweather_config.clone(), String::from("24171")).unwrap();
    // let forecast = Forecast::get_daily(accuweather_config.clone(), location.clone());
    // let current = CurrentCondition::get(accuweather_config.clone(), location.clone());
    // println!("{:?}", forecast);
    // println!("{:?}", current);


    // Homebrew Weather Server Example
    // curl -X GET "http://localhost:8080/" -H "Authorization: xxx"
    // curl -X POST "http://localhost:8080/api/weather_reports" -H "Authorization: xxx" -d "device_type=outdoor&temperature=32&humidity=50&pm25=2&pm10=3&percipitation=4&tvoc=10&co2=400"
    let pg = homebrew::PostgresServer::new();
    let homebrew_config = homebrew::Config{
        apikey: String::from(accu_key.clone()),
        port: 9090,
        pg: pg
    };
    // homebrew_config.clone().init().await;


    // Combo example
    let pg = combo::PostgresServer::new();
    let config = combo::Config{
        apikey: String::from(accu_key.clone()),
        port: 9091,
        pg: pg,
        cache_timeout: Some(3600),
        accu_config: Some(accuweather_config),
        homebrew_config: Some(homebrew_config),
        zip_code: String::from(zip_code)
    };
    config.init().await;

    loop{}
}
