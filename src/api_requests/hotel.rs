use crate::utils::Date;
use gemini_client_api::gemini::utils::{GeminiSchema, gemini_function, gemini_schema};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::env;
use std::error::Error;

const RAPID_API_HOST: &str = "booking-com15.p.rapidapi.com";
const BASE_URL: &str = "https://booking-com15.p.rapidapi.com";
const DEFAULT_CURRENCY: &str = "INR";

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
pub struct HotelSearchResponse {
    pub status: bool,
    pub message: Value,
    pub data: HotelSearchData,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct HotelSearchData {
    pub result: Vec<HotelSearchResult>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct HotelDetails {
    pub hotel_id: i64,
    pub hotel_name: String,
    pub arrival_date: Option<String>,
    pub departure_date: Option<String>,
    pub latitude: f64,
    pub longitude: f64,
    pub address: Option<String>,
    pub city: Option<String>,
    pub review_nr: Option<i64>,
    pub price_rounded: Option<String>,
    pub property_highlights: Option<Vec<String>>,
    pub top_facilities: Option<Vec<String>>,
    pub url: String,
}

// Internal-only struct for deserialization
#[derive(Deserialize, Debug)]
struct RawHotelDetails {
    hotel_id: i64,
    hotel_name: String,
    arrival_date: Option<String>,
    departure_date: Option<String>,
    latitude: f64,
    longitude: f64,
    address: Option<String>,
    city: Option<String>,
    review_nr: Option<i64>,
    product_price_breakdown: Option<Value>,
    property_highlight_strip: Option<Vec<Value>>,
    facilities_block: Option<Value>,
    url: String,
}

#[derive(Deserialize, Debug)]
struct RawDetailsResponse {
    status: bool,
    message: Value,
    data: RawHotelDetails,
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
pub struct HotelDescriptionResponse {
    pub status: bool,
    pub message: Value,
    pub data: Vec<HotelDescriptionItem>,
}

#[gemini_function]
/// Search for hotels near the specified coordinates.
/// Use this tool to find a list of available hotels in a specific area.
/// Reponse has temperature_unit: C, units: metric and price in INR
pub async fn get_hotel_by_coordinates(
    /// Latitude of the location to search around (e.g. 18.6429). Can be found with
    /// get_about_place()
    latitude: f64,
    /// Longitude of the location to search around (e.g. 72.8759).Can be found with
    /// get_about_place()
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
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(format!("Hotel Search Error: {}", resp.status()).into());
    }

    let raw: Value = resp.json().await?;
    if raw.get("status").and_then(|s| s.as_bool()) != Some(true) {
        let msg = raw
            .get("message")
            .map(|m| m.to_string())
            .unwrap_or_default();
        return Err(format!("Hotel Search API error: {}", msg).into());
    }

    let resp: HotelSearchResponse = serde_json::from_value(raw)?;
    Ok(resp.data)
}

#[gemini_function]
/// Get detailed information for a specific hotel, including address, facilities, and high-level pricing.
/// Use this tool after finding a hotel ID from a search to get more specifics.
/// "url" field can be provided as booking link
pub async fn get_hotel_details(
    /// The unique hotel ID (e.g. "15109166").
    /// Provided by get_hotel_by_coordinates()
    hotel_id: String,
    /// Date of arrival for the intended stay.
    arrival_date: Date,
    /// Date of departure for the intended stay.
    departure_date: Date,
    adults: u8,
    ///The number of children, including infants, who are under 18. Example: Child 1 Age = 8 months Child 2 Age = 1 year Child 3 Age = 17 years Here is what the request parameter would look like: children_age: 0,1,17
    children_age: Option<String>,
    ///The number of rooms that are required
    room_qty: u8,
) -> Result<HotelDetails, Box<dyn Error + Send + Sync>> {
    let api_key = env::var("RAPIDAPI_KEY")?;
    let client = reqwest::Client::new();
    let url = format!("{BASE_URL}/api/v1/hotels/getHotelDetails");

    let mut query = vec![
        ("hotel_id", hotel_id),
        ("arrival_date", arrival_date.to_yyyy_mm_dd()),
        ("departure_date", departure_date.to_yyyy_mm_dd()),
        ("languagecode", "en-us".to_string()),
        ("currency_code", DEFAULT_CURRENCY.to_string()),
        ("adults", adults.to_string()),
        ("room_qty", room_qty.to_string()),
    ];
    if let Some(v) = children_age {
        query.push(("children_age", v));
    }
    let resp = client
        .get(&url)
        .header("x-rapidapi-key", api_key)
        .header("x-rapidapi-host", RAPID_API_HOST)
        .query(&query)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(format!("Hotel Details Error: {}", resp.status()).into());
    }

    let raw: Value = resp.json().await?;
    if raw.get("status").and_then(|s| s.as_bool()) != Some(true) {
        let msg = raw
            .get("message")
            .map(|m| m.to_string())
            .unwrap_or_default();
        return Err(format!("Hotel Details API error: {}", msg).into());
    }

    let raw: RawDetailsResponse = serde_json::from_value(raw)?;
    let d = raw.data;

    // Extract price from nested product_price_breakdown
    let price_rounded = d
        .product_price_breakdown
        .as_ref()
        .and_then(|p| p.pointer("/all_inclusive_amount_hotel_currency/amount_rounded"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Flatten property highlights to just names
    let property_highlights = d.property_highlight_strip.map(|strips| {
        strips
            .iter()
            .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
            .map(|s| s.to_string())
            .collect()
    });

    // Flatten facilities to just names
    let top_facilities = d.facilities_block.as_ref().and_then(|fb| {
        fb.pointer("/facilities")
            .and_then(|f| f.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.get("name").and_then(|n| n.as_str()))
                    .map(|s| s.to_string())
                    .collect()
            })
    });

    Ok(HotelDetails {
        hotel_id: d.hotel_id,
        hotel_name: d.hotel_name,
        arrival_date: d.arrival_date,
        departure_date: d.departure_date,
        latitude: d.latitude,
        longitude: d.longitude,
        address: d.address,
        city: d.city,
        review_nr: d.review_nr,
        url: d.url,
        price_rounded,
        property_highlights,
        top_facilities,
    })
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

    let raw: Value = resp.json().await?;
    if raw.get("status").and_then(|s| s.as_bool()) != Some(true) {
        let msg = raw
            .get("message")
            .map(|m| m.to_string())
            .unwrap_or_default();
        return Err(format!("Hotel Description API error: {}", msg).into());
    }

    let resp: HotelDescriptionResponse = serde_json::from_value(raw)?;
    Ok(resp.data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::Date;

    #[tokio::test]
    async fn test_get_hotel_by_coordinates() {
        let arrival = Date::new(2026, 3, 15).unwrap();
        let departure = Date::new(2026, 3, 20).unwrap();
        let result = get_hotel_by_coordinates(21.57, 83.20, arrival, departure, 1, 1).await;
        match &result {
            Ok(data) => {
                println!("Hotel Search Results ({} hotels):", data.result.len());
                for h in data.result.iter().take(3) {
                    println!(
                        "  {} - {} (score: {:?}, price: {:?})",
                        h.hotel_id, h.hotel_name, h.review_score, h.min_total_price
                    );
                }
                assert!(
                    !data.result.is_empty(),
                    "Expected at least one hotel result"
                );
            }
            Err(e) => {
                println!("Error: {}", e);
                panic!("get_hotel_by_coordinates failed: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_hotel_details() {
        let hotel_id = "191605".to_string(); // Novotel Mumbai Juhu Beach
        let arrival = Date::new(2026, 3, 15).unwrap();
        let departure = Date::new(2026, 3, 20).unwrap();
        let result = get_hotel_details(hotel_id, arrival, departure, 1, None, 1).await;
        match &result {
            Ok(data) => {
                println!("Hotel Details:");
                println!("  Name: {}", data.hotel_name);
                println!("  City: {:?}", data.city);
                println!("  Price: {:?}", data.price_rounded);
                println!("  Highlights: {:?}", data.property_highlights);
                println!("  Facilities: {:?}", data.top_facilities);
                println!("  Url: {:?}", data.url);
                assert_eq!(data.hotel_name, "Novotel Mumbai Juhu Beach");
            }
            Err(e) => {
                println!("Error: {}", e);
                panic!("get_hotel_details failed: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_room_availability() {
        let hotel_id = "191605".to_string();
        let arrival = Date::new(2026, 3, 15).unwrap();
        let departure = Date::new(2026, 3, 20).unwrap();
        let result = get_room_availability(hotel_id, arrival, departure, 1, 1).await;
        match &result {
            Ok(data) => {
                println!("Room Availability:");
                println!("{:#?}", data);
            }
            Err(e) => {
                println!("Error: {}", e);
                panic!("get_room_availability failed: {}", e);
            }
        }
    }

    #[tokio::test]
    async fn test_get_hotel_description() {
        let hotel_id = "191605".to_string();
        let result = get_hotel_description(hotel_id).await;
        match &result {
            Ok(data) => {
                println!("Hotel Description:");
                println!("{:#?}", data);
                assert!(!data.is_empty(), "Expected at least one description item");
            }
            Err(e) => {
                println!("Error: {}", e);
                panic!("get_hotel_description failed: {}", e);
            }
        }
    }
}
