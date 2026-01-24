use reqwest::header::AUTHORIZATION;
use serde::Deserialize;
use serde_json::Value;
use std::{env, ops::Range};

use crate::utils::{Date, IataCode, get_bearer_token};

const BASE_URL: &str = "https://test.api.amadeus.com/v3/shopping/hotel-offers";
const HOTEL_LIST_URL: &str =
    "https://test.api.amadeus.com/v1/reference-data/locations/hotels/by-city";
#[derive(Deserialize)]
struct AmadeusHotelListResponse {
    data: Vec<AmadeusHotelReference>,
}

#[derive(Deserialize)]
struct AmadeusHotelReference {
    #[serde(rename = "hotelId")]
    hotel_id: String,
}

pub async fn hotels_in_city(
    city_code: IataCode,
    check_in_date: Date,
    adults: u8,
    currency_code: &str,
    rating: Range<u8>,
) -> Result<Value, Box<dyn std::error::Error + Send + Sync>> {
    let client_id = env::var("AMADEUS_API_KEY")?;
    let client_secret = env::var("AMADEUS_API_SECRET")?;

    let token = get_bearer_token(&client_id, &client_secret).await?;
    let client = reqwest::Client::new();

    // 1. Get hotels by city
    let resp = client
        .get(HOTEL_LIST_URL)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .query(&[
            ("cityCode", city_code.to_string()),
            ("ratings", rating.map(|v| v.to_string()).collect::<Vec<String>>().join(",")),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let error_text = resp.text().await?;
        return Err(format!("Amadeus Hotel List error: {} - {}", status, error_text).into());
    }

    let list_response: AmadeusHotelListResponse = resp.json().await?;
    let hotel_ids: Vec<String> = list_response
        .data
        .iter()
        .take(10)
        .map(|h| h.hotel_id.clone())
        .collect();

    if hotel_ids.is_empty() {
        return Err("No hotels found in given city_code".into());
    }

    // 2. Get offers for those hotels
    let resp = client
        .get(BASE_URL)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .query(&[
            ("hotelIds", hotel_ids.join(",")),
            ("checkInDate", check_in_date.to_yyyy_mm_dd()),
            ("adults", adults.to_string()),
            ("currency", currency_code.to_string()),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        if resp.status() == 404 {
            return Err("No hotels found in given city_code".into());
        }
        let status = resp.status();
        let error_text = resp.text().await?;
        return Err(format!("Amadeus Hotel Offers error: {} - {}", status, error_text).into());
    }

    Ok(resp.json().await?)
}

#[tokio::test]
async fn hotels_in_city_test() {
    let city_code = IataCode::new("DEL".to_string()).unwrap();
    let check_in_date = Date::new(2026, 1, 26).unwrap();
    let currency = "INR";

    let result = hotels_in_city(city_code, check_in_date, 2, currency, 3..5).await;

    match result {
        Ok(hotels) => {
            dbg!(&hotels);
        }
        Err(e) => {
            panic!("Error fetching hotels: {}", e);
        }
    }
}
