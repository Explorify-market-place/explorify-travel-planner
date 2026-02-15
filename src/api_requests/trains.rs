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
    pub train_type: String,
    pub run_days: Vec<String>,

    /// Arrival time at destination. Used to calculate if the user reaches in the morning/night.
    pub to_sta: String,

    /// Departure time from source. Essential for "after work" or "early morning" filters.
    pub from_std: String,

    /// Detailed station names (e.g., "MUMBAI CENTRAL").
    pub to_station_name: String,

    /// The total travel time (e.g., "15:40").
    pub duration: String,

    /// List of available travel classes (e.g., ["3A", "2A", "1A"]).
    pub class_type: Vec<String>,

    /// Indicates if food is available on board.
    pub has_pantry: bool,

    /// Total number of stops.
    pub halt_stn: u32,

    /// A pre-calculated quality score from the API.
    pub score: i32,

    /// Distance in KM.
    pub distance: f64,

    /// Proximity text (e.g., "6 Kms from NDLS").
    pub from_distance_text: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct TrainBetweenResponse {
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
/// Response:
/// Train {
///   train_number,
///   train_name,
///   train_type,
///   run_days,
///   // Arrival time at destination. Used to calculate if the user reaches in the morning/night.
///   to_sta,
///   // Departure time from source. Essential for "after work" or "early morning" filters.
///   from_std,
///   // Detailed station names (e.g., "MUMBAI CENTRAL").
///   to_station_name,
///   // The total travel time (e.g., "15:40").
///   duration,
///   // List of available travel classes (e.g., ["3A", "2A", "1A"]).
///   class_type,
///   // Indicates if food is available on board.
///   has_pantry,
///   // Total number of stops.
///   // A pre-calculated quality score from the API.
///   score,
///   // Distance in KM.
///   distance,
///   // Proximity text (e.g., "6 Kms from NDLS").
///   from_distance_text,
///}[]
pub async fn trains_between(
    ///Source station
    source: Station,
    ///Destination station
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

    let response: TrainBetweenResponse = resp.json().await?;
    Ok(response.data)
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
    pub availability: String,
    pub fare: u32,
    pub quota: String,
}

#[derive(Deserialize)]
struct SeatAvailabilityResponse {
    pub status: bool,
    pub message: String,
    pub data: Vec<AvailabilityDetail>,
}

#[gemini_function]
///Check seat availability and status for a specific train and class.
pub async fn train_seats_available(
    train_number: String,
    from_station: Station,
    to_station: Station,
    date: Date,
    ///Class code (e.g., '2A', '3A', 'SL')
    class: String,
    ///Quota code (e.g., 'GN', 'TQ')
    quota: String,
) -> Result<Vec<AvailabilityDetail>, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!(
        "https://irctc1.p.rapidapi.com/api/v2/checkSeatAvailability?classType={}&quota={}&trainNo={}&date={}&fromStationCode={}&toStationCode={}",
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
    Ok(body.data)
}

#[tokio::test]
async fn trains_between_test() {
    dbg!(
        trains_between(
            Station::new("NDLS".into()).unwrap(),
            Station::new("BCT".into()).unwrap(),
            Date::new_now(),
        )
        .await
        .unwrap()
    );
}

#[tokio::test]
async fn train_seats_available_test() {
    let result = train_seats_available(
        "12952".to_string(),
        Station::new("NDLS".into()).unwrap(),
        Station::new("MMCT".into()).unwrap(),
        Date::new_now(),
        "3A".to_string(),
        "GN".to_string(),
    )
    .await;

    match result {
        Ok(data) => {
            println!("Got availability data: {:#?}", data);
        }
        Err(e) => {
            println!("Error fetching availability: {}", e);
        }
    }
}
