use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::env;

use crate::utils::{generate_bearer_token, Currency, Date, IataCode, Time};

const BASE_URL: &str = "https://test.api.amadeus.com/v2/shopping/flight-offers";

#[derive(Serialize)]
pub struct Flight {
    arrival_date: Date,
    departure_date: Date,
    arrival_time: Time,
    departure_time: Time,
    price: Currency,
}

#[derive(Deserialize)]
struct FlightOffersResponse {
    data: Vec<Offer>,
}

#[derive(Deserialize)]
struct Offer {
    price: OfferPrice,
    itineraries: Vec<Itinerary>,
}

#[derive(Deserialize)]
struct OfferPrice {
    currency: String,
    total: String,
}

#[derive(Deserialize)]
struct Itinerary {
    segments: Vec<Segment>,
}

#[derive(Deserialize)]
struct Segment {
    departure: Endpoint,
    arrival: Endpoint,
}

#[derive(Deserialize)]
struct Endpoint {
    at: String,
}

pub async fn flights_between(
    source: IataCode,
    destination: IataCode,
    least_departure: Date,
    adult_count: u8,
    currency_code: &str,
) -> Result<Vec<Flight>, Box<dyn std::error::Error + Send + Sync>> {
    let bearer_token = generate_bearer_token(
        &env::var("AMADEUS_API_KEY").unwrap(),
        &env::var("AMADEUS_API_SECRET").unwrap(),
    )
    .await?;
    let departure = least_departure.to_yyyy_mm_dd();
    let url = format!(
        "{BASE_URL}?originLocationCode={source}&destinationLocationCode={destination}&departureDate={departure}&adults={adult_count}&currencyCode={currency_code}"
    );

    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header(AUTHORIZATION, format!("Bearer {}", bearer_token))
        .header(CONTENT_TYPE, "application/json")
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(format!(
            "Amadeus flights API error: {}\nMessage: {}",
            resp.status(),
            resp.text().await?
        )
        .into());
    }

    let payload: FlightOffersResponse = resp.json().await?;

    let mut flights: Vec<Flight> = Vec::new();
    for offer in payload.data.into_iter() {
        if offer.itineraries.is_empty() {
            continue;
        }
        let first_itin = &offer.itineraries[0];
        if first_itin.segments.is_empty() {
            continue;
        }
        let first_seg = &first_itin.segments[0];
        let last_seg = &first_itin.segments[first_itin.segments.len() - 1];

        let (dep_date, dep_time) = split_iso_datetime(&first_seg.departure.at)?;
        let (arr_date, arr_time) = split_iso_datetime(&last_seg.arrival.at)?;

        let price = Currency::parse_currency(currency_code, &offer.price.total)?;

        flights.push(Flight {
            arrival_date: arr_date,
            departure_date: dep_date,
            arrival_time: arr_time,
            departure_time: dep_time,
            price,
        });
    }

    Ok(flights)
}

fn split_iso_datetime(iso: &str) -> Result<(Date, Time), Box<dyn std::error::Error + Send + Sync>> {
    // Expect formats like "YYYY-MM-DDTHH:MM:SS" or with timezone suffix; take first 19 chars
    let trimmed = if iso.len() >= 19 { &iso[0..19] } else { iso };
    let mut parts = trimmed.split('T');
    let date_str = parts
        .next()
        .ok_or_else(|| "Invalid datetime: missing date".to_string())?;
    let time_str = parts
        .next()
        .ok_or_else(|| "Invalid datetime: missing time".to_string())?;
    let date = Date::from_yyyy_mm_dd(date_str)?;
    let time = Time::from_hh_mm_ss(time_str)?;
    Ok((date, time))
}
