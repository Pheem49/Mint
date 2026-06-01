use reqwest::Client;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherReport {
    pub location: String,
    pub data: String,
    pub temperature_celsius: f64,
    pub apparent_temperature_celsius: f64,
    pub humidity_percent: f64,
    pub wind_speed_kmh: f64,
    pub weather_code: i64,
}

pub async fn weather(city: &str) -> Result<WeatherReport, String> {
    let city = city.trim();
    if city.is_empty() {
        return Err("weather city is required".into());
    }
    let client = Client::new();
    let geocode: Value = client
        .get("https://geocoding-api.open-meteo.com/v1/search")
        .query(&[("name", city), ("count", "1"), ("language", "en")])
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?
        .json()
        .await
        .map_err(request_error)?;
    let place = geocode["results"]
        .as_array()
        .and_then(|results| results.first())
        .ok_or_else(|| format!("weather location was not found: {city}"))?;
    let latitude = number(place, "latitude")?;
    let longitude = number(place, "longitude")?;
    let current: Value = client
        .get("https://api.open-meteo.com/v1/forecast")
        .query(&[
            ("latitude", latitude.to_string()),
            ("longitude", longitude.to_string()),
            (
                "current",
                "temperature_2m,relative_humidity_2m,apparent_temperature,weather_code,wind_speed_10m".into(),
            ),
        ])
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?
        .json()
        .await
        .map_err(request_error)?;
    let current = &current["current"];
    let temperature = number(current, "temperature_2m")?;
    let apparent = number(current, "apparent_temperature")?;
    let humidity = number(current, "relative_humidity_2m")?;
    let wind = number(current, "wind_speed_10m")?;
    let code = current["weather_code"].as_i64().unwrap_or_default();
    let location = [place["name"].as_str(), place["country"].as_str()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(", ");
    Ok(WeatherReport {
        data: format!(
            "{location}: {:.1} C, feels like {:.1} C, humidity {:.0}%, wind {:.1} km/h",
            temperature, apparent, humidity, wind
        ),
        location,
        temperature_celsius: temperature,
        apparent_temperature_celsius: apparent,
        humidity_percent: humidity,
        wind_speed_kmh: wind,
        weather_code: code,
    })
}

fn number(value: &Value, key: &str) -> Result<f64, String> {
    value[key]
        .as_f64()
        .ok_or_else(|| format!("weather response did not include {key}"))
}

fn request_error(error: reqwest::Error) -> String {
    format!("weather request failed: {error}")
}
