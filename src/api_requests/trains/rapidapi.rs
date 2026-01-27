use crate::utils::Date;
use gemini_client_api::gemini::utils::{GeminiSchema, gemini_function};
use reqwest::header::{HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::fmt::Display;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Station(String);
impl GeminiSchema for Station {
    fn gemini_schema() -> serde_json::Value {
        json!({"type": "STRING"})
    }
}

impl Station {
    pub fn new(code: String) -> Result<Self, String> {
        if code
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit())
        {
            Ok(Self(code))
        } else {
            Err(format!("Invalid Station code: {code}"))
        }
    }
}

impl Display for Station {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Train {
    pub train_number: String,
    pub train_name: String,
    pub from_sta: String,
    pub to_sta: String,
    pub run_days: Vec<String>,
    pub train_type: String,
}

#[derive(Deserialize)]
struct TrainBetweenResponse {
    data: Vec<Train>,
}

fn get_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "X-RapidAPI-Key",
        HeaderValue::from_str(&env::var("RAPIDAPI_KEY").expect("RAPIDAPI_KEY not found")).unwrap(),
    );
    headers.insert(
        "X-RapidAPI-Host",
        HeaderValue::from_static("irctc1.p.rapidapi.com"),
    );
    headers
}
#[gemini_function]
/// Search for trains running between two stations on a specific date.
pub async fn trains_between(
    ///Source station code (e.g., 'NDLS')
    source: Station,
    ///Destination station code (e.g., 'BCT')
    destination: Station,
    date: Date,
) -> Result<Vec<Train>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "https://irctc1.p.rapidapi.com/api/v3/trainBetweenStations?fromStationCode={}&toStationCode={}&dateOfJourney={}",
        source,
        destination,
        date.to_yyyy_mm_dd()
    );

    let client = reqwest::Client::new();
    let resp = client.get(url).headers(get_headers()).send().await?;

    if !resp.status().is_success() {
        return Err(format!("RapidAPI error: {}", resp.status()).into());
    }

    let body: TrainBetweenResponse = resp.json().await?;
    let trains = body
        .data
        .into_iter()
        .map(|d| Train {
            train_number: d.train_number,
            train_name: d.train_name,
            from_sta: d.from_sta,
            to_sta: d.to_sta,
            run_days: d.run_days,
            train_type: d.train_type,
        })
        .collect();

    Ok(trains)
}

#[derive(Serialize, Deserialize, Debug)]
pub struct TrainDetails {
    pub train_number: String,
    pub train_name: String,
    pub station_list: Vec<StationArrival>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct StationArrival {
    pub station_code: String,
    pub station_name: String,
    pub arrival_time: String,
    pub departure_time: String,
    pub halt_time: String,
}

#[derive(Deserialize)]
struct TrainDetailsResponse {
    data: TrainDetailsData,
}

#[derive(Deserialize)]
struct TrainDetailsData {
    train_number: String,
    train_name: String,
    station_list: Vec<StationArrival>,
}

#[gemini_function]
///Get full details of a train including its route and station stops.
pub async fn train_details(
    ///Train number (e.g., '12002')
    train_number: String,
) -> Result<TrainDetails, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "https://irctc1.p.rapidapi.com/api/v1/getTrainDetails?trainNo={}",
        train_number
    );

    let client = reqwest::Client::new();
    let resp = client.get(url).headers(get_headers()).send().await?;

    if !resp.status().is_success() {
        return Err(format!("RapidAPI error: {}", resp.status()).into());
    }

    let body: TrainDetailsResponse = resp.json().await?;
    Ok(TrainDetails {
        train_number: body.data.train_number,
        train_name: body.data.train_name,
        station_list: body.data.station_list,
    })
}

#[derive(Serialize, Deserialize, Debug)]
pub struct SeatAvailability {
    pub train_number: String,
    pub class: String,
    pub quota: String,
    pub availability: Vec<AvailabilityDetail>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AvailabilityDetail {
    pub date: String,
    pub status: String,
}

#[derive(Deserialize)]
struct SeatAvailabilityResponse {
    data: Vec<AvailabilityDetail>,
}

pub async fn train_seats_available(
    train_number: &str,
    from_station: Station,
    to_station: Station,
    date: Date,
    class: &str,
    quota: &str,
) -> Result<SeatAvailability, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "https://irctc1.p.rapidapi.com/api/v1/checkSeatAvailability?classCode={}&quotaCode={}&trainNo={}&dateOfJourney={}&fromStationCode={}&toStationCode={}",
        class,
        quota,
        train_number,
        date.to_yyyy_mm_dd(),
        from_station,
        to_station
    );

    let client = reqwest::Client::new();
    let resp = client.get(url).headers(get_headers()).send().await?;

    if !resp.status().is_success() {
        return Err(format!("RapidAPI error: {}", resp.status()).into());
    }

    let body: SeatAvailabilityResponse = resp.json().await?;
    Ok(SeatAvailability {
        train_number: train_number.to_string(),
        class: class.to_string(),
        quota: quota.to_string(),
        availability: body.data,
    })
}

#[tokio::test]
async fn trains_between_structure_test() {
    dbg!(
        trains_between(
            Station::new("NDLS".into()).unwrap(),
            Station::new("BCT".into()).unwrap(),
            Date::new(2026, 1, 23).unwrap(),
        )
        .await
        .unwrap()
    );
}
