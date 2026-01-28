use crate::utils::{Currency, Date, IataCode, get_bearer_token};
use gemini_client_api::gemini::utils::{GeminiSchema, gemini_function};
use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};
use std::env;

const BASE_URL: &str = "https://test.api.amadeus.com/v2/shopping/flight-offers";

#[derive(Debug, Serialize, Deserialize)]
pub struct Flight {
    pub id: String,
    pub price: Currency,
    pub itineraries: Vec<Itinerary>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Itinerary {
    pub duration: String,
    pub segments: Vec<Segment>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Segment {
    pub departure: Endpoint,
    pub arrival: Endpoint,
    pub carrier_code: String,
    pub number: String,
    pub duration: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Endpoint {
    pub iata_code: String,
    pub at: String,
}

#[derive(Deserialize)]
struct AmadeusFlightResponse {
    data: Vec<AmadeusFlightOffer>,
}

#[derive(Deserialize)]
struct AmadeusFlightOffer {
    id: String,
    price: AmadeusPrice,
    itineraries: Vec<AmadeusItinerary>,
}

#[derive(Deserialize)]
struct AmadeusPrice {
    currency: String,
    total: String,
}

#[derive(Deserialize)]
struct AmadeusItinerary {
    duration: String,
    segments: Vec<AmadeusSegment>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmadeusSegment {
    departure: AmadeusEndpoint,
    arrival: AmadeusEndpoint,
    carrier_code: String,
    number: String,
    duration: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmadeusEndpoint {
    iata_code: String,
    at: String,
}

#[gemini_function]
///Search for flight offers between two cities on a specific date.
pub async fn flights_between(
    ///IATA origin city code (e.g., 'JFK')
    source: IataCode,
    ///IATA destination city code (e.g., 'LAX')
    destination: IataCode,
    least_departure: Date,
    ///Number of adult passengers
    adult_count: u8,
    ///3-letter currency code (e.g., 'USD')
    currency_code: String,
) -> Result<Vec<Flight>, Box<dyn std::error::Error + Send + Sync>> {
    let client_id = env::var("AMADEUS_API_KEY")?;
    let client_secret = env::var("AMADEUS_API_SECRET")?;

    let token = get_bearer_token(&client_id, &client_secret).await?;

    let client = reqwest::Client::new();
    let resp = client
        .get(BASE_URL)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .query(&[
            ("originLocationCode", source.to_string()),
            ("destinationLocationCode", destination.to_string()),
            ("departureDate", least_departure.to_yyyy_mm_dd()),
            ("adults", adult_count.to_string()),
            ("currencyCode", currency_code.to_string()),
            ("max", "10".to_string()),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let error_text = resp.text().await?;
        return Err(format!("Amadeus API error: {}", error_text).into());
    }

    let response: AmadeusFlightResponse = resp.json().await?;

    let flights = response
        .data
        .into_iter()
        .map(|offer| {
            let currency = Currency::parse_currency(&offer.price.currency, &offer.price.total)
                .unwrap_or(Currency::Usd(0.0));
            Flight {
                id: offer.id,
                price: currency,
                itineraries: offer
                    .itineraries
                    .into_iter()
                    .map(|iti| Itinerary {
                        duration: iti.duration,
                        segments: iti
                            .segments
                            .into_iter()
                            .map(|seg| Segment {
                                departure: Endpoint {
                                    iata_code: seg.departure.iata_code,
                                    at: seg.departure.at,
                                },
                                arrival: Endpoint {
                                    iata_code: seg.arrival.iata_code,
                                    at: seg.arrival.at,
                                },
                                carrier_code: seg.carrier_code,
                                number: seg.number,
                                duration: seg.duration,
                            })
                            .collect(),
                    })
                    .collect(),
            }
        })
        .collect();

    Ok(flights)
}

#[gemini_function]
///Retrieve seat maps and availability for a specific flight offer ID.
pub async fn flight_seats_available(
    ///The unique ID of the flight offer
    flight_offer_id: String,
) -> Result<serde_json::Value, Box<dyn std::error::Error + Send + Sync>> {
    let client_id = env::var("AMADEUS_API_KEY")?;
    let client_secret = env::var("AMADEUS_API_SECRET")?;

    let token = get_bearer_token(&client_id, &client_secret).await?;

    let client = reqwest::Client::new();
    let resp = client
        .get("https://test.api.amadeus.com/v1/shopping/seatmaps")
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .query(&[("flight-offerId", flight_offer_id)])
        .send()
        .await?;

    if !resp.status().is_success() {
        let error_text = resp.text().await?;
        return Err(format!("Amadeus API error: {}", error_text).into());
    }

    let response: serde_json::Value = resp.json().await?;
    Ok(response)
}

#[tokio::test]
async fn flights_between_integration_test() {
    if std::env::var("AMADEUS_API_KEY").is_err() || std::env::var("AMADEUS_API_SECRET").is_err() {
        println!("Skipping integration test: Amadeus credentials not found in env");
        return;
    }

    let source = IataCode::new("JFK".to_string()).unwrap();
    let destination = IataCode::new("LAX".to_string()).unwrap();
    let departure_date = Date::new(2026, 1, 25).unwrap();
    let adult_count = 1;
    let currency = "USD".to_string();

    let result = flights_between(source, destination, departure_date, adult_count, currency).await;

    dbg!(result.unwrap());
}
