use reqwest::header::AUTHORIZATION;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{env, error::Error};

use crate::{
    api_requests::site_seen::get_place_image_url,
    utils::{Currency, Date, IataCode, generate_bearer_token},
};

const BASE_URL: &str = "https://test.api.amadeus.com/v3/shopping/hotel-offers";
const HOTEL_LIST_URL: &str =
    "https://test.api.amadeus.com/v1/reference-data/locations/hotels/by-city";

#[derive(Debug, Serialize, Deserialize)]
pub struct Hotel {
    pub name: String,
    pub hotel_id: String,
    pub price: Option<Currency>,
    pub image_url: Option<String>,
    pub description: Option<String>,
    pub address: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Deserialize)]
struct AmadeusHotelListResponse {
    data: Vec<AmadeusHotelReference>,
}

#[derive(Deserialize)]
struct AmadeusHotelReference {
    #[serde(rename = "hotelId")]
    hotel_id: String,
    name: String,
    address: Option<AmadeusAddress>,
    latitude: Option<f64>,
    longitude: Option<f64>,
}

#[derive(Deserialize)]
struct AmadeusAddress {
    #[serde(rename = "lines")]
    lines: Option<Vec<String>>,
    #[serde(rename = "cityName")]
    city_name: Option<String>,
}

#[derive(Deserialize)]
struct AmadeusHotelOffersResponse {
    data: Vec<AmadeusHotelOffer>,
}

#[derive(Deserialize)]
struct AmadeusHotelOffer {
    hotel: AmadeusHotelReference,
    offers: Vec<AmadeusOffer>,
}

#[derive(Deserialize)]
struct AmadeusOffer {
    price: AmadeusPrice,
}

#[derive(Deserialize)]
struct AmadeusPrice {
    currency: String,
    total: String,
}

pub async fn hotels_in_city(
    city_code: IataCode,
    check_in_date: Date,
    adults: u8,
    currency_code: &str,
    budget: f32,
) -> Result<Vec<Hotel>, Box<dyn std::error::Error + Send + Sync>> {
    let client_id = env::var("AMADEUS_API_KEY")?;
    let client_secret = env::var("AMADEUS_API_SECRET")?;

    let token = generate_bearer_token(&client_id, &client_secret).await?;
    let client = reqwest::Client::new();

    // 1. Get hotels by city
    let resp = client
        .get(HOTEL_LIST_URL)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .query(&[("cityCode", city_code.to_string())])
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
        return Ok(vec![]);
    }

    // 2. Get offers for those hotels
    let resp = client
        .get(BASE_URL)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .query(&[
            ("hotelIds", hotel_ids.join(",")),
            ("checkInDate", check_in_date.to_amadeus_dd_mm_yy()),
            ("adults", adults.to_string()),
            ("currency", currency_code.to_string()),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        if resp.status() == 404 {
            return Ok(vec![]);
        }
        let status = resp.status();
        let error_text = resp.text().await?;
        return Err(format!("Amadeus Hotel Offers error: {} - {}", status, error_text).into());
    }

    let offers_response: AmadeusHotelOffersResponse = resp.json().await?;

    let mut hotels = Vec::new();
    for offer in offers_response.data {
        let price_val = offer.offers.get(0).map(|o| &o.price);
        let currency = if let Some(p) = price_val {
            Currency::parse_currency(&p.currency, &p.total).ok()
        } else {
            None
        };

        // Filter by budget if price is available
        if let Some(ref c) = currency {
            let amount = match c {
                Currency::Inr(a) => *a,
                Currency::Usd(a) => *a,
                Currency::Eur(a) => *a,
            };
            if amount > budget {
                continue;
            }
        }

        let image_url =
            get_place_image_url(&format!("{} hotel in {}", &offer.hotel.name, city_code))
                .await
                .ok();

        let hotel = Hotel {
            name: offer.hotel.name,
            hotel_id: offer.hotel.hotel_id,
            price: currency,
            image_url,
            description: None,
            address: offer
                .hotel
                .address
                .map(|a| a.lines.unwrap_or_default().join(", ")),
            latitude: offer.hotel.latitude,
            longitude: offer.hotel.longitude,
        };

        hotels.push(hotel);
    }

    Ok(hotels)
}

pub async fn hotel_details(
    hotel_id: &str,
    check_in_date: Date,
) -> Result<Value, Box<dyn Error + Send + Sync>> {
    let client_id = env::var("AMADEUS_API_KEY")?;
    let client_secret = env::var("AMADEUS_API_SECRET")?;

    let token = generate_bearer_token(&client_id, &client_secret).await?;
    let client = reqwest::Client::new();

    let resp = client
        .get(BASE_URL)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .query(&[
            ("hotelIds", hotel_id.to_string()),
            ("checkInDate", check_in_date.to_amadeus_dd_mm_yy()),
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let error_text = resp.text().await?;
        return Err(format!("Amadeus Hotel Details error: {} - {}", status, error_text).into());
    }

    let json: Value = resp.json().await?;
    Ok(json)
}

#[tokio::test]
async fn hotels_in_city_test() {
    dotenv::dotenv().ok();
    if std::env::var("AMADEUS_API_KEY").is_err() || std::env::var("AMADEUS_API_SECRET").is_err() {
        println!("Skipping integration test: Amadeus credentials not found in env");
        return;
    }

    let city_code = IataCode::new("DEL".to_string()).unwrap();
    let check_in_date = Date::new(2025, 10, 4).unwrap();
    let budget = 10000.0;
    let currency = "INR";

    let result = hotels_in_city(city_code, check_in_date, 2, currency, budget).await;

    match result {
        Ok(hotels) => {
            dbg!(&hotels);
            assert!(!hotels.is_empty());
        }
        Err(e) => {
            panic!("Error fetching hotels: {}", e);
        }
    }
}
