use crate::utils::Date;
use gemini_client_api::gemini::utils::{GeminiSchema, gemini_function, gemini_schema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::error::Error;

const RAPID_API_HOST: &str = "booking-com15.p.rapidapi.com";
const BASE_URL: &str = "https://booking-com15.p.rapidapi.com";
const DEFAULT_CURRENCY: &str = "INR";
const DEFAULT_LOCATION: &str = "IN";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct BookingCheckInOut {
    pub from: Option<String>,
    pub until: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct HotelSearchResult {
    pub hotel_id: i64,
    pub hotel_name: String,
    pub review_score: Option<f64>,
    pub review_score_word: Option<String>,
    pub min_total_price: Option<f64>,
    pub currencycode: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub city: Option<String>,
    pub address: Option<String>,
    pub checkin: Option<BookingCheckInOut>,
    pub checkout: Option<BookingCheckInOut>,
    pub is_free_cancellable: Option<i32>,
    pub unit_configuration_label: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct HotelSearchResponse {
    pub status: bool,
    pub message: String,
    pub data: HotelSearchData,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct HotelSearchData {
    pub result: Vec<HotelSearchResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct HotelPrice {
    pub value: f64,
    pub currency: String,
    pub amount_rounded: String,
    pub amount_unrounded: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct Facility {
    pub name: String,
    pub icon: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct FacilitiesBlock {
    pub name: String,
    pub facilities: Vec<Facility>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct PropertyHighlight {
    pub name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct HotelDetails {
    pub hotel_id: i64,
    pub hotel_name: String,
    pub description: Option<String>,
    pub arrival_date: Option<String>,
    pub departure_date: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub address: Option<String>,
    pub city: Option<String>,
    pub review_nr: Option<i64>,
    pub all_inclusive_amount_hotel_currency: Option<HotelPrice>,
    pub property_highlight_strip: Option<Vec<PropertyHighlight>>,
    pub facilities_block: Option<FacilitiesBlock>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct HotelDetailsResponse {
    pub status: bool,
    pub message: String,
    pub data: HotelDetails,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct AvailabilityEntry {
    pub date: String,
    pub price: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct AvailabilityData {
    pub av_dates: Vec<AvailabilityEntry>,
    pub lengths_of_stay: Vec<AvailabilityEntry>,
    pub currency: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct HotelDescriptionItem {
    pub hotel_id: String,
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct HotelDescriptionResponse {
    pub status: bool,
    pub message: String,
    pub data: Vec<HotelDescriptionItem>,
}

#[gemini_function]
/// Search for hotels near the specified coordinates.
/// Use this tool to find a list of available hotels in a specific area.
pub async fn get_hotel_by_coordinates(
    /// Latitude of the location to search around (e.g. 18.6429).
    latitude: f64,
    /// Longitude of the location to search around (e.g. 72.8759).
    longitude: f64,
    /// Date of arrival at the hotel.
    arrival_date: Date,
    /// Date of departure from the hotel.
    departure_date: Date,
    /// Number of adults for the stay.
    adults: u8,
    /// Number of rooms required.
    room_qty: u8,
) -> Result<HotelSearchData, Box<dyn Error + Send + Sync>> {
    let api_key = env::var("RAPIDAPI_KEY")?;
    let client = reqwest::Client::new();
    let url = format!("{BASE_URL}/api/v1/hotels/searchHotelsByCoordinates");

    let resp = client
        .get(&url)
        .header("x-rapidapi-key", api_key)
        .header("x-rapidapi-host", RAPID_API_HOST)
        .query(&[
            ("latitude", latitude.to_string()),
            ("longitude", longitude.to_string()),
            ("arrival_date", arrival_date.to_yyyy_mm_dd()),
            ("departure_date", departure_date.to_yyyy_mm_dd()),
            ("adults", adults.to_string()),
            ("room_qty", room_qty.to_string()),
            ("units", "metric".to_string()),
            ("page_number", "1".to_string()),
            ("temperature_unit", "c".to_string()),
            ("languagecode", "en-us".to_string()),
            ("currency_code", DEFAULT_CURRENCY.to_string()),
            ("location", DEFAULT_LOCATION.to_string()),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(format!("Hotel Search Error: {}", resp.status()).into());
    }
    let resp: HotelSearchResponse = resp.json().await?;

    Ok(resp.data)
}

#[gemini_function]
/// Get detailed information for a specific hotel, including address, facilities, and high-level pricing.
/// Use this tool after finding a hotel ID from a search to get more specifics.
pub async fn get_hotel_details(
    /// The unique hotel ID (e.g. "15109166").
    /// Provided by get_hotel_by_coordinates()
    hotel_id: String,
    /// Date of arrival for the intended stay.
    arrival_date: Date,
    /// Date of departure for the intended stay.
    departure_date: Date,
) -> Result<HotelDetails, Box<dyn Error + Send + Sync>> {
    let api_key = env::var("RAPIDAPI_KEY")?;
    let client = reqwest::Client::new();
    let url = format!("{BASE_URL}/api/v1/hotels/getHotelDetails");

    let resp = client
        .get(&url)
        .header("x-rapidapi-key", api_key)
        .header("x-rapidapi-host", RAPID_API_HOST)
        .query(&[
            ("hotel_id", hotel_id),
            ("arrival_date", arrival_date.to_yyyy_mm_dd()),
            ("departure_date", departure_date.to_yyyy_mm_dd()),
            ("languagecode", "en-us".to_string()),
            ("currency_code", DEFAULT_CURRENCY.to_string()),
            ("location", DEFAULT_LOCATION.to_string()),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(format!("Hotel Details Error: {}", resp.status()).into());
    }

    let resp: HotelDetailsResponse = resp.json().await?;
    Ok(resp.data)
}

#[gemini_function]
/// Check room availability and pricing for a specific hotel over a date range.
/// This tool returns available dates and stay durations.
pub async fn get_room_availability(
    /// The unique hotel ID.
    /// Provided by get_hotel_by_coordinates()
    hotel_id: String,
    /// Starting date for the availability check.
    arrival_date: Date,
    /// Ending date for the availability check.
    departure_date: Date,
    /// Number of adults.
    adults: u8,
    /// Number of rooms.
    room_qty: u8,
) -> Result<AvailabilityData, Box<dyn Error + Send + Sync>> {
    let api_key = env::var("RAPIDAPI_KEY")?;
    let client = reqwest::Client::new();
    let url = format!("{BASE_URL}/api/v1/hotels/getAvailability");

    let resp = client
        .get(&url)
        .header("x-rapidapi-key", api_key)
        .header("x-rapidapi-host", RAPID_API_HOST)
        .query(&[
            ("hotel_id", hotel_id),
            ("arrival_date", arrival_date.to_yyyy_mm_dd()),
            ("departure_date", departure_date.to_yyyy_mm_dd()),
            ("adults", adults.to_string()),
            ("room_qty", room_qty.to_string()),
            ("currency_code", DEFAULT_CURRENCY.to_string()),
            ("location", DEFAULT_LOCATION.to_string()),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(format!("Hotel Availability Error: {}", resp.status()).into());
    }

    let raw: Value = resp.json().await?;

    // Transform dynamic date keys into structured entries
    let mut av_dates = Vec::new();
    if let Some(dates) = raw.pointer("/data/avDates").and_then(|v| v.as_array()) {
        for item in dates {
            if let Some(obj) = item.as_object() {
                for (date, price) in obj {
                    if let Some(p) = price.as_i64() {
                        av_dates.push(AvailabilityEntry {
                            date: date.clone(),
                            price: p as i32,
                        });
                    }
                }
            }
        }
    }

    let mut lengths_of_stay = Vec::new();
    if let Some(stays) = raw
        .pointer("/data/lengthsOfStay")
        .and_then(|v| v.as_array())
    {
        for item in stays {
            if let Some(obj) = item.as_object() {
                for (date, stay) in obj {
                    if let Some(s) = stay.as_i64() {
                        lengths_of_stay.push(AvailabilityEntry {
                            date: date.clone(),
                            price: s as i32, // Reusing price field for stay duration for simplicity or rename struct
                        });
                    }
                }
            }
        }
    }

    Ok(AvailabilityData {
        av_dates,
        lengths_of_stay,
        currency: raw
            .pointer("/data/currency")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
    })
}

#[gemini_function]
/// Get a descriptive text summary and general information for a specific hotel.
pub async fn get_hotel_description(
    /// The unique hotel ID.
    /// Provided by get_hotel_by_coordinates()
    hotel_id: String,
) -> Result<Vec<HotelDescriptionItem>, Box<dyn Error + Send + Sync>> {
    let api_key = env::var("RAPIDAPI_KEY")?;
    let client = reqwest::Client::new();
    let url = format!("{BASE_URL}/api/v1/hotels/getDescriptionAndInfo");

    let resp = client
        .get(&url)
        .header("x-rapidapi-key", api_key)
        .header("x-rapidapi-host", RAPID_API_HOST)
        .query(&[
            ("hotel_id", hotel_id),
            ("languagecode", "en-us".to_string()),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(format!("Hotel Description Error: {}", resp.status()).into());
    }

    let resp: HotelDescriptionResponse = resp.json().await?;
    Ok(resp.data)
}
