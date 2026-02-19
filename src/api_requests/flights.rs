use crate::utils::{Date, IataCode};
use gemini_client_api::gemini::utils::{GeminiSchema, gemini_function, gemini_schema};
use serde::{Deserialize, Serialize};
use serde_json::{Value, to_value};
use std::error::Error;
use std::sync::Arc;
use std::{env, mem};
use tokio::sync::Mutex;

pub type TokenMap = Arc<Mutex<Vec<String>>>;

const RAPID_API_HOST: &str = "google-flights2.p.rapidapi.com";
const BASE_URL: &str = "https://google-flights2.p.rapidapi.com";

#[derive(Deserialize, Serialize)]
#[gemini_schema]
#[allow(non_camel_case_types)]
pub enum TravelClass {
    ECONOMY,
    PREMIUMECONOMY,
    BUSINESS,
    FIRST,
}

pub const TOKEN_PREFIX: &str = "TOKEN_";
pub fn update_token_map(map: &mut Vec<String>, token: String) -> String {
    let placeholder = format!("{TOKEN_PREFIX}{}", map.len());
    map.push(token);
    placeholder
}

pub fn resolve_token(
    map: &Vec<String>,
    placeholder: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let idx: usize = placeholder[TOKEN_PREFIX.len()..]
        .parse()
        .map_err(|_| "Invalid token provided")?;
    map.get(idx).cloned().ok_or("Invalid token provided".into())
}

fn clean_and_replace_tokens(
    val: &mut Value,
    token_map: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for itinerary in val
        .as_array_mut()
        .ok_or("Invalid response. Itinerary not found")?
    {
        for flight in itinerary["flights"]
            .as_array_mut()
            .ok_or("Invalid response. Flights not found")?
        {
            flight
                .as_object_mut()
                .ok_or("Invalid response. flight is not an object")?
                .remove("airline_logo");
        }
        let obj_ref = itinerary
            .as_object_mut()
            .ok_or("Invalid response. Itinerary not found")?;
        obj_ref.remove("airline_logo");
        obj_ref.remove("carbon_emissions");
        if let Some(booking_token) = obj_ref.get_mut("booking_token") {
            let small_token =
                update_token_map(token_map, booking_token.as_str().unwrap().to_string());
            obj_ref.insert("booking_token".into(), small_token.into());
        }
    }
    Ok(())
}

#[gemini_function]
#[allow(unused_variables)]
/// Search for one-way flights between two cities on a specific date using Google Flights.
/// Returns a list of flight itineraries. Each itinerary contains a 'booking_token' (e.g., TOKEN_0)
/// which MUST be passed to 'flight_booking_details' to get live price and booking link.
/// Price will be in INR.
pub async fn flights_between(
    /// starting airport
    origin: IataCode,
    /// destination airport
    destination: IataCode,
    /// The date of departure.
    date: Date,
    /// The class of travel (ECONOMY, PREMIUM_ECONOMY, BUSINESS, or FIRST).
    travel_class: TravelClass,
    /// Number of adult passengers (12+ years old).
    adults: u8,
    ///The number of child passengers (ages 2–11).
    children: u8,
    ///The count of infants traveling without a seat, sitting on an adult's lap (ages < 2).
    infant_on_lap: Option<u8>,
    ///The count of infants (below 2 years old) who require a separate seat.
    infant_in_seat: Option<u8>,
    ///Specifies the type of search strategy to apply when retrieving flight results.
    ///`best`: prioritizes a balanced mix of price, duration, and convenience.
    ///`cheap`: returns the lowest-cost options, possibly with longer layovers or travel time.
    search_type: Option<String>,
) -> Result<Value, Box<dyn Error + Send + Sync>> {
    todo!()
}
pub async fn between(
    origin: IataCode,
    destination: IataCode,
    date: Date,
    travel_class: TravelClass,
    adults: u8,
    token_map: TokenMap,
    children: u8,
    infant_on_lap: Option<u8>,
    infant_in_seat: Option<u8>,
    search_type: Option<String>,
) -> Result<Value, Box<dyn Error + Send + Sync>> {
    let api_key = env::var("RAPIDAPI_KEY")?;
    let client = reqwest::Client::new();

    let url = format!("{BASE_URL}/api/v1/searchFlights");

    let mut query = vec![
        ("departure_id", origin.to_string()),
        ("arrival_id", destination.to_string()),
        ("outbound_date", date.to_yyyy_mm_dd()),
        ("currency", "INR".to_string()),
        ("country_code", "IN".to_string()),
        ("adults", adults.to_string()),
        (
            "travel_class",
            to_value(travel_class)?.as_str().unwrap().to_string(),
        ),
        ("children", children.to_string()),
        ("search_type", search_type.unwrap_or("cheap".into())),
    ];
    if let Some(v) = infant_on_lap {
        query.push(("infant_on_lap", v.to_string()));
    }
    if let Some(v) = infant_in_seat {
        query.push(("infant_in_seat", v.to_string()));
    }

    let resp = client
        .get(&url)
        .header("x-rapidapi-key", api_key)
        .header("x-rapidapi-host", RAPID_API_HOST)
        .query(&query)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await?;
        return Err(format!("Flight Search Error: {} - {}", status, text).into());
    }

    let mut val: Value = resp.json().await?;

    // Extract topFlights, supplementing with otherFlights if fewer than 3
    let top = val
        .pointer_mut("/data/itineraries/topFlights")
        .and_then(|v| v.as_array_mut().map(mem::take))
        .unwrap_or_default();

    let mut flights = top;
    if flights.len() < 3 {
        if let Some(others) = val
            .pointer_mut("/data/itineraries/otherFlights")
            .and_then(|v| v.as_array_mut().map(mem::take))
        {
            flights.extend(others);
        }
    }

    if flights.is_empty() {
        return Err(format!(
            "No flight itineraries found for {origin} to {destination} on {}",
            date.to_yyyy_mm_dd()
        )
        .into());
    }

    let mut flights_val = Value::Array(flights);
    clean_and_replace_tokens(&mut flights_val, &mut *token_map.lock().await)?;
    Ok(flights_val)
}

#[gemini_function]
#[allow(unused_variables)]
/// Get detailed booking options for a specific flight itinerary.
/// Use this after 'flights_between' to see different ways to book the flight (e.g., directly with airline or via OTA).
/// Returns a list of booking options, each with a 'token' (e.g., TOKEN_1) that MUST be passed to 'flight_booking_link' to get the final URL.
/// Price will be in INR.
pub async fn flight_booking_details(
    /// The placeholder token (e.g., TOKEN_0) received from 'flights_between' for a specific itinerary.
    booking_token: String,
) -> Result<(Vec<Value>, Vec<String>), Box<dyn Error + Send + Sync>> {
    todo!()
}
pub async fn booking_details(
    booking_token: String,
    token_map: TokenMap,
) -> Result<Vec<Value>, Box<dyn Error + Send + Sync>> {
    let api_key = env::var("RAPIDAPI_KEY")?;
    let client = reqwest::Client::new();
    let url = format!("{BASE_URL}/api/v1/getBookingDetails");

    let response = client
        .get(&url)
        .header("x-rapidapi-key", api_key)
        .header("x-rapidapi-host", RAPID_API_HOST)
        .query(&[
            ("booking_token", booking_token),
            ("currency", "INR".to_string()),
            ("country_code", "IN".into()),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Booking Link Error: {}", response.status()).into());
    }
    let mut val: Value = response.json().await?;
    let data = val["data"].as_array_mut().ok_or("data not found")?;

    //Updating response with placeholder tokens
    let mut map = token_map.lock().await;
    for flights in data.iter_mut() {
        let flights = flights
            .as_object_mut()
            .ok_or("Invalid response format. Data don't have objects")?;

        if let Some(booking_token) = flights.get_mut("token") {
            let small_token =
                update_token_map(&mut map, booking_token.as_str().unwrap().to_string());
            flights.insert("token".into(), small_token.into());
        }
    }

    Ok(mem::take(data))
}

#[gemini_function]
#[allow(unused_variables)]
/// Get the final booking URL for a specific booking option.
/// Returns object containing the "url" to the checkout page and the token passed in agrument.
pub async fn flight_booking_link(
    /// The placeholder token (e.g., TOKEN_1) received from 'flight_booking_details' for a specific booking option.
    token: String,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let api_key = env::var("RAPIDAPI_KEY")?;
    let client = reqwest::Client::new();

    let url = format!("{BASE_URL}/api/v1/getBookingURL");

    let resp = client
        .get(&url)
        .header("x-rapidapi-key", api_key)
        .header("x-rapidapi-host", RAPID_API_HOST)
        .query(&[("token", token)])
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(format!("Booking Link Error: {}", resp.status()).into());
    }

    let val: Value = resp.json().await?;

    if let Some(link) = val["data"].as_str() {
        Ok(link.to_string())
    } else {
        Err(val.to_string().into())
    }
}

#[tokio::test]
async fn flights_between_test() {
    dbg!(
        between(
            IataCode::new("IXR").unwrap(),
            IataCode::new("GOI").unwrap(),
            Date::new_now(),
            TravelClass::ECONOMY,
            2,
            Arc::new(Mutex::new(vec![])),
            0,
            None,
            None,
            None,
        )
        .await
        .unwrap()
    );
}
