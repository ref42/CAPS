use serde::Deserialize;

#[derive(Clone, Debug)]
pub struct WeatherNow {
    pub icon: String,
    pub label: String,
    pub title: String,
}

impl Default for WeatherNow {
    fn default() -> Self {
        Self {
            icon: include_str!("../../src/assets/weather/unknown.svg").to_string(),
            label: "--°".to_string(),
            title: "Weather unavailable".to_string(),
        }
    }
}

#[derive(Deserialize)]
struct LocationResponse {
    success: bool,
    latitude: Option<f64>,
    longitude: Option<f64>,
    city: Option<String>,
}

#[derive(Deserialize)]
struct ForecastResponse {
    current_weather: Option<CurrentWeather>,
}

#[derive(Deserialize)]
struct CurrentWeather {
    temperature: f64,
    weathercode: i32,
}

pub async fn local_weather() -> WeatherNow {
    fetch_local_weather().await.unwrap_or_default()
}

async fn fetch_local_weather() -> Option<WeatherNow> {
    let location = fetch_location().await?;

    let latitude = location.latitude?;
    let longitude = location.longitude?;
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={latitude:.4}&longitude={longitude:.4}&current_weather=true&temperature_unit=celsius"
    );
    let forecast = reqwest::get(url)
        .await
        .ok()?
        .json::<ForecastResponse>()
        .await
        .ok()?;
    let current = forecast.current_weather?;
    let icon = weather_icon(current.weathercode).to_string();
    let temp = current.temperature.round() as i32;
    let city = location.city.unwrap_or_else(|| "Local".to_string());

    Some(WeatherNow {
        icon,
        label: format!("{temp}°"),
        title: format!("{city} weather"),
    })
}

async fn fetch_location() -> Option<LocationResponse> {
    if let Some(location) = reqwest::get("https://ipwho.is/")
        .await
        .ok()?
        .json::<LocationResponse>()
        .await
        .ok()
        .filter(|location| location.success)
    {
        return Some(location);
    }
    if let Some(location) = reqwest::get("https://ipapi.co/json/")
        .await
        .ok()?
        .json::<IpApiResponse>()
        .await
        .ok()
    {
        return Some(LocationResponse {
            success: true,
            latitude: location.latitude,
            longitude: location.longitude,
            city: location.city,
        });
    }
    None
}

#[derive(Deserialize)]
struct IpApiResponse {
    latitude: Option<f64>,
    longitude: Option<f64>,
    city: Option<String>,
}

fn weather_icon(code: i32) -> &'static str {
    match code {
        0 => include_str!("../../src/assets/weather/clear.svg"),
        1 | 2 => include_str!("../../src/assets/weather/partly-cloudy.svg"),
        3 => include_str!("../../src/assets/weather/cloudy.svg"),
        45 | 48 => include_str!("../../src/assets/weather/fog.svg"),
        51 | 53 | 55 | 56 | 57 | 61 | 63 | 65 | 66 | 67 | 80 | 81 | 82 => {
            include_str!("../../src/assets/weather/rain.svg")
        }
        71 | 73 | 75 | 77 | 85 | 86 => include_str!("../../src/assets/weather/snow.svg"),
        95 | 96 | 99 => include_str!("../../src/assets/weather/thunderstorm.svg"),
        _ => include_str!("../../src/assets/weather/unknown.svg"),
    }
}
