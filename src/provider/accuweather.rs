use serde_json::json;

use serde::{Serialize, Deserialize};
use std::convert::TryInto;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub apikey: String,
    pub language: Option<String>,
    pub details: Option<bool>,
    pub metric: Option<bool>,
}
impl Config {
    pub fn to_params(&self) -> String{
        let mut params = format!("?apikey={}", self.apikey);

        match &self.language {
            Some(x) => {
                params = format!("{}&language={}", params, x);
            },
            None => {}
        }

        match &self.details {
            Some(x) => {
                params = format!("{}&details={}", params, x);
            },
            None => {}
        }

        match &self.metric {
            Some(x) => {
                params = format!("{}&metric={}", params, x);
            },
            None => {}
        }
        return params;
    }
}


pub type Locations = Vec<Location>;


// http://dataservice.accuweather.com/locations/v1/postalcodes/search
#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Location {
    #[serde(rename = "Version")]
    pub version: f64,
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "Type")]
    pub type_field: String,
    #[serde(rename = "Rank")]
    pub rank: f64,
    #[serde(rename = "LocalizedName")]
    pub localized_name: String,
    #[serde(rename = "EnglishName")]
    pub english_name: String,
    #[serde(rename = "PrimaryPostalCode")]
    pub primary_postal_code: String,
    // #[serde(rename = "Region")]
    // pub region: Region,
    // #[serde(rename = "Country")]
    // pub country: Country,
    // #[serde(rename = "AdministrativeArea")]
    // pub administrative_area: AdministrativeArea,
    // #[serde(rename = "TimeZone")]
    // pub time_zone: TimeZone,
    // #[serde(rename = "GeoPosition")]
    // pub geo_position: GeoPosition,
    // #[serde(rename = "IsAlias")]
    // pub is_alias: bool,
    // #[serde(rename = "ParentCity")]
    // pub parent_city: Option<ParentCity>,
    // #[serde(rename = "SupplementalAdminAreas")]
    // pub supplemental_admin_areas: Vec<SupplementalAdminArea>,
    #[serde(rename = "DataSets")]
    pub data_sets: Vec<String>,
    #[serde(rename = "Details")]
    pub details: Option<Details>,
}
impl Location {

    // http://dataservice.accuweather.com/locations/v1/postalcodes/search
    // apikey: string
    // q: string
    // language: string
    // details: bool
    pub fn search_by_zip(config: Config, q: String) -> Result<Location, reqwest::Error> {
        let url = format!("http://dataservice.accuweather.com/locations/v1/postalcodes/search{}&q={}", config.to_params(), q);

        let request = reqwest::blocking::Client::new().get(url).send();
        match request {
            Ok(req) => {
                let json = req.json::<Locations>()?;
                return Ok(json[0].clone());
            },
            Err(err) => {
                return Err(err);
            }
        }

    }
}


#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Forecast {
    #[serde(rename = "Headline")]
    pub headline: Headline,
    #[serde(rename = "DailyForecasts")]
    pub daily_forecasts: Vec<DailyForecast>,
}
impl Forecast {

    // http://dataservice.accuweather.com/forecasts/v1/daily/1day/{location_id}
    // apikey: string
    // language: string
    // details: bool
    // metric: bool
    pub fn get_daily(config: Config, location: Location) -> Result<Forecast, reqwest::Error> {
        let mut url = format!("http://dataservice.accuweather.com/forecasts/v1/daily/1day/{}{}", location.key, config.to_params());

        let request = reqwest::blocking::Client::new().get(url).send();
        match request {
            Ok(req) => {
                let json = req.json::<Forecast>()?;
                return Ok(json);
            },
            Err(err) => {
                return Err(err);
            }
        }

    }
}

pub type CurrentConditions = Vec<CurrentCondition>;

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentCondition {
    #[serde(rename = "LocalObservationDateTime")]
    pub local_observation_date_time: String,
    #[serde(rename = "EpochTime")]
    pub epoch_time: i64,
    #[serde(rename = "WeatherText")]
    pub weather_text: String,
    #[serde(rename = "WeatherIcon")]
    pub weather_icon: i64,
    #[serde(rename = "HasPrecipitation")]
    pub has_precipitation: bool,
    #[serde(rename = "PrecipitationType")]
    pub precipitation_type: Option<String>,
    #[serde(rename = "IsDayTime")]
    pub is_day_time: bool,
    #[serde(rename = "Temperature")]
    pub temperature: Temperature2,
    #[serde(rename = "MobileLink")]
    pub mobile_link: String,
    #[serde(rename = "Link")]
    pub link: String,
}
impl CurrentCondition {

    // http://dataservice.accuweather.com/currentconditions/v1/{location_id}
    // apikey: string
    // language: string
    // details: bool
    pub fn get(config: Config, location: Location) -> Result<CurrentCondition, reqwest::Error> {
        let mut url = format!("http://dataservice.accuweather.com/currentconditions/v1/{}{}", location.key, config.to_params());

        let request = reqwest::blocking::Client::new().get(url).send();
        match request {
            Ok(req) => {
                let json = req.json::<CurrentConditions>()?;
                return Ok(json[0].clone());
            },
            Err(err) => {
                return Err(err);
            }
        }

    }
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Headline {
    #[serde(rename = "EffectiveDate")]
    pub effective_date: String,
    #[serde(rename = "EffectiveEpochDate")]
    pub effective_epoch_date: f64,
    #[serde(rename = "Severity")]
    pub severity: f64,
    #[serde(rename = "Text")]
    pub text: String,
    #[serde(rename = "Category")]
    pub category: String,
    #[serde(rename = "EndDate")]
    pub end_date: String,
    #[serde(rename = "EndEpochDate")]
    pub end_epoch_date: f64,
    #[serde(rename = "MobileLink")]
    pub mobile_link: String,
    #[serde(rename = "Link")]
    pub link: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyForecast {
    #[serde(rename = "Date")]
    pub date: String,
    #[serde(rename = "EpochDate")]
    pub epoch_date: f64,
    #[serde(rename = "Sun")]
    pub sun: Option<Sun>,
    #[serde(rename = "Moon")]
    pub moon: Option<Moon>,
    #[serde(rename = "Temperature")]
    pub temperature: Temperature,
    #[serde(rename = "RealFeelTemperature")]
    pub real_feel_temperature: Option<RealFeelTemperature>,
    #[serde(rename = "RealFeelTemperatureShade")]
    pub real_feel_temperature_shade: Option<RealFeelTemperatureShade>,
    #[serde(rename = "HoursOfSun")]
    pub hours_of_sun: Option<f64>,
    #[serde(rename = "DegreeDaySummary")]
    pub degree_day_summary: Option<DegreeDaySummary>,
    #[serde(rename = "AirAndPollen")]
    pub air_and_pollen: Option<Vec<AirAndPollen>>,
    // #[serde(rename = "Day")]
    // pub day: Day,
    // #[serde(rename = "Night")]
    // pub night: Night,
    // #[serde(rename = "Sources")]
    // pub sources: Vec<String>,
    #[serde(rename = "MobileLink")]
    pub mobile_link: String,
    #[serde(rename = "Link")]
    pub link: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sun {
    #[serde(rename = "Rise")]
    pub rise: String,
    #[serde(rename = "EpochRise")]
    pub epoch_rise: f64,
    #[serde(rename = "Set")]
    pub set: String,
    #[serde(rename = "EpochSet")]
    pub epoch_set: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Moon {
    #[serde(rename = "Rise")]
    pub rise: String,
    #[serde(rename = "EpochRise")]
    pub epoch_rise: f64,
    #[serde(rename = "Set")]
    pub set: String,
    #[serde(rename = "EpochSet")]
    pub epoch_set: f64,
    #[serde(rename = "Phase")]
    pub phase: String,
    #[serde(rename = "Age")]
    pub age: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Temperature {
    #[serde(rename = "Minimum")]
    pub minimum: Minimum,
    #[serde(rename = "Maximum")]
    pub maximum: Maximum,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Temperature2 {
    #[serde(rename = "Metric")]
    pub metric: Metric,
    #[serde(rename = "Imperial")]
    pub imperial: Imperial,
}



#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Minimum {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Maximum {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealFeelTemperature {
    #[serde(rename = "Minimum")]
    pub minimum: Minimum2,
    #[serde(rename = "Maximum")]
    pub maximum: Maximum2,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Minimum2 {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
    #[serde(rename = "Phrase")]
    pub phrase: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Maximum2 {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
    #[serde(rename = "Phrase")]
    pub phrase: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RealFeelTemperatureShade {
    #[serde(rename = "Minimum")]
    pub minimum: Minimum3,
    #[serde(rename = "Maximum")]
    pub maximum: Maximum3,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Minimum3 {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
    #[serde(rename = "Phrase")]
    pub phrase: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Maximum3 {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
    #[serde(rename = "Phrase")]
    pub phrase: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DegreeDaySummary {
    #[serde(rename = "Heating")]
    pub heating: Heating,
    #[serde(rename = "Cooling")]
    pub cooling: Cooling,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Heating {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cooling {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AirAndPollen {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Category")]
    pub category: String,
    #[serde(rename = "CategoryValue")]
    pub category_value: f64,
    #[serde(rename = "Type")]
    pub type_field: Option<String>,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Day {
    #[serde(rename = "Icon")]
    pub icon: f64,
    #[serde(rename = "IconPhrase")]
    pub icon_phrase: String,
    #[serde(rename = "HasPrecipitation")]
    pub has_precipitation: bool,
    #[serde(rename = "ShortPhrase")]
    pub short_phrase: Option<String>,
    #[serde(rename = "LongPhrase")]
    pub long_phrase: Option<String>,
    #[serde(rename = "PrecipitationProbability")]
    pub precipitation_probability: Option<f64>,
    #[serde(rename = "ThunderstormProbability")]
    pub thunderstorm_probability: Option<f64>,
    #[serde(rename = "RainProbability")]
    pub rain_probability: Option<f64>,
    #[serde(rename = "SnowProbability")]
    pub snow_probability: Option<f64>,
    #[serde(rename = "IceProbability")]
    pub ice_probability: Option<f64>,
    #[serde(rename = "Wind")]
    pub wind: Wind,
    #[serde(rename = "WindGust")]
    pub wind_gust: WindGust,
    #[serde(rename = "TotalLiquid")]
    pub total_liquid: TotalLiquid,
    #[serde(rename = "Rain")]
    pub rain: Rain,
    #[serde(rename = "Snow")]
    pub snow: Snow,
    #[serde(rename = "Ice")]
    pub ice: Ice,
    #[serde(rename = "HoursOfPrecipitation")]
    pub hours_of_precipitation: f64,
    #[serde(rename = "HoursOfRain")]
    pub hours_of_rain: f64,
    #[serde(rename = "HoursOfSnow")]
    pub hours_of_snow: f64,
    #[serde(rename = "HoursOfIce")]
    pub hours_of_ice: f64,
    #[serde(rename = "CloudCover")]
    pub cloud_cover: f64,
    #[serde(rename = "Evapotranspiration")]
    pub evapotranspiration: Evapotranspiration,
    #[serde(rename = "SolarIrradiance")]
    pub solar_irradiance: SolarIrradiance,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wind {
    #[serde(rename = "Speed")]
    pub speed: Speed,
    #[serde(rename = "Direction")]
    pub direction: Direction,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Speed {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Direction {
    #[serde(rename = "Degrees")]
    pub degrees: f64,
    #[serde(rename = "Localized")]
    pub localized: String,
    #[serde(rename = "English")]
    pub english: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindGust {
    #[serde(rename = "Speed")]
    pub speed: Speed2,
    #[serde(rename = "Direction")]
    pub direction: Direction2,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Speed2 {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Direction2 {
    #[serde(rename = "Degrees")]
    pub degrees: f64,
    #[serde(rename = "Localized")]
    pub localized: String,
    #[serde(rename = "English")]
    pub english: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotalLiquid {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rain {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snow {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ice {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Evapotranspiration {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SolarIrradiance {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Night {
    #[serde(rename = "Icon")]
    pub icon: f64,
    #[serde(rename = "IconPhrase")]
    pub icon_phrase: String,
    #[serde(rename = "HasPrecipitation")]
    pub has_precipitation: bool,
    #[serde(rename = "ShortPhrase")]
    pub short_phrase: Option<String>,
    #[serde(rename = "LongPhrase")]
    pub long_phrase: Option<String>,
    #[serde(rename = "PrecipitationProbability")]
    pub precipitation_probability: Option<f64>,
    #[serde(rename = "ThunderstormProbability")]
    pub thunderstorm_probability: Option<f64>,
    #[serde(rename = "RainProbability")]
    pub rain_probability: Option<f64>,
    #[serde(rename = "SnowProbability")]
    pub snow_probability: Option<f64>,
    #[serde(rename = "IceProbability")]
    pub ice_probability: Option<f64>,
    #[serde(rename = "Wind")]
    pub wind: Wind2,
    #[serde(rename = "WindGust")]
    pub wind_gust: WindGust2,
    #[serde(rename = "TotalLiquid")]
    pub total_liquid: TotalLiquid2,
    #[serde(rename = "Rain")]
    pub rain: Rain2,
    #[serde(rename = "Snow")]
    pub snow: Snow2,
    #[serde(rename = "Ice")]
    pub ice: Ice2,
    #[serde(rename = "HoursOfPrecipitation")]
    pub hours_of_precipitation: f64,
    #[serde(rename = "HoursOfRain")]
    pub hours_of_rain: f64,
    #[serde(rename = "HoursOfSnow")]
    pub hours_of_snow: f64,
    #[serde(rename = "HoursOfIce")]
    pub hours_of_ice: f64,
    #[serde(rename = "CloudCover")]
    pub cloud_cover: f64,
    #[serde(rename = "Evapotranspiration")]
    pub evapotranspiration: Evapotranspiration,
    #[serde(rename = "SolarIrradiance")]
    pub solar_irradiance: SolarIrradiance,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Wind2 {
    #[serde(rename = "Speed")]
    pub speed: Speed3,
    #[serde(rename = "Direction")]
    pub direction: Direction3,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Speed3 {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Direction3 {
    #[serde(rename = "Degrees")]
    pub degrees: f64,
    #[serde(rename = "Localized")]
    pub localized: String,
    #[serde(rename = "English")]
    pub english: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindGust2 {
    #[serde(rename = "Speed")]
    pub speed: Speed4,
    #[serde(rename = "Direction")]
    pub direction: Direction4,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Speed4 {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Direction4 {
    #[serde(rename = "Degrees")]
    pub degrees: f64,
    #[serde(rename = "Localized")]
    pub localized: String,
    #[serde(rename = "English")]
    pub english: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TotalLiquid2 {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Rain2 {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snow2 {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Ice2 {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}



#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Region {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "LocalizedName")]
    pub localized_name: String,
    #[serde(rename = "EnglishName")]
    pub english_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Country {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "LocalizedName")]
    pub localized_name: String,
    #[serde(rename = "EnglishName")]
    pub english_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdministrativeArea {
    #[serde(rename = "ID")]
    pub id: String,
    #[serde(rename = "LocalizedName")]
    pub localized_name: String,
    #[serde(rename = "EnglishName")]
    pub english_name: String,
    #[serde(rename = "Level")]
    pub level: f64,
    #[serde(rename = "LocalizedType")]
    pub localized_type: String,
    #[serde(rename = "EnglishType")]
    pub english_type: String,
    #[serde(rename = "CountryID")]
    pub country_id: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeZone {
    #[serde(rename = "Code")]
    pub code: String,
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "GmtOffset")]
    pub gmt_offset: f64,
    #[serde(rename = "IsDaylightSaving")]
    pub is_daylight_saving: bool,
    #[serde(rename = "NextOffsetChange")]
    pub next_offset_change: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeoPosition {
    #[serde(rename = "Latitude")]
    pub latitude: f64,
    #[serde(rename = "Longitude")]
    pub longitude: f64,
    #[serde(rename = "Elevation")]
    pub elevation: Elevation,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Elevation {
    #[serde(rename = "Metric")]
    pub metric: Metric,
    #[serde(rename = "Imperial")]
    pub imperial: Imperial,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metric {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Imperial {
    #[serde(rename = "Value")]
    pub value: f64,
    #[serde(rename = "Unit")]
    pub unit: String,
    #[serde(rename = "UnitType")]
    pub unit_type: f64,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParentCity {
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "LocalizedName")]
    pub localized_name: String,
    #[serde(rename = "EnglishName")]
    pub english_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupplementalAdminArea {
    #[serde(rename = "Level")]
    pub level: f64,
    #[serde(rename = "LocalizedName")]
    pub localized_name: String,
    #[serde(rename = "EnglishName")]
    pub english_name: String,
}

#[derive(Default, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    #[serde(rename = "Key")]
    pub key: String,
    #[serde(rename = "StationCode")]
    pub station_code: String,
    #[serde(rename = "StationGmtOffset")]
    pub station_gmt_offset: f64,
    #[serde(rename = "BandMap")]
    pub band_map: String,
    #[serde(rename = "Climo")]
    pub climo: String,
    #[serde(rename = "LocalRadar")]
    pub local_radar: String,
    #[serde(rename = "MediaRegion")]
    pub media_region: Option<String>,
    #[serde(rename = "Metar")]
    pub metar: String,
    #[serde(rename = "NXMetro")]
    pub nxmetro: String,
    #[serde(rename = "NXState")]
    pub nxstate: String,
    // #[serde(rename = "Population")]
    // pub population: Value,
    #[serde(rename = "PrimaryWarningCountyCode")]
    pub primary_warning_county_code: String,
    #[serde(rename = "PrimaryWarningZoneCode")]
    pub primary_warning_zone_code: String,
    #[serde(rename = "Satellite")]
    pub satellite: String,
    #[serde(rename = "Synoptic")]
    pub synoptic: String,
    #[serde(rename = "MarineStation")]
    pub marine_station: String,
    // #[serde(rename = "MarineStationGMTOffset")]
    // pub marine_station_gmtoffset: Value,
    #[serde(rename = "VideoCode")]
    pub video_code: String,
    #[serde(rename = "LocationStem")]
    pub location_stem: String,
    // #[serde(rename = "DMA")]
    // pub dma: Option<Dma>,
    // #[serde(rename = "PartnerID")]
    // pub partner_id: Value,
    // #[serde(rename = "Sources")]
    // pub sources: Vec<Source>,
    #[serde(rename = "CanonicalPostalCode")]
    pub canonical_postal_code: String,
    #[serde(rename = "CanonicalLocationKey")]
    pub canonical_location_key: String,
}