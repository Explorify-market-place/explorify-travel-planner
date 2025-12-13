use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::env;

use crate::{
    api_requests::site_seen::get_place_image_url,
    utils::{Currency, IataCode, generate_bearer_token},
};

const BASE_URL: &str = "https://test.api.amadeus.com/v3/shopping/hotel-offers";

#[derive(Serialize)]
pub struct Hotel {
    name: String,
    latitude: f64,
    longitude: f64,
    address: String,
    price: Currency,
    image_url: String,
}

#[derive(Deserialize)]
struct HotelOffersResponse {
    data: Vec<HotelItem>,
}

#[derive(Deserialize)]
struct HotelItem {
    hotel: HotelInfo,
    offers: Vec<HotelOffer>,
}

#[derive(Deserialize)]
struct HotelInfo {
    name: String,
    latitude: Option<f64>,
    longitude: Option<f64>,
    address: Option<Address>,
}

#[derive(Deserialize)]
struct Address {
    lines: Option<Vec<String>>,
    cityName: Option<String>,
    countryCode: Option<String>,
}

#[derive(Deserialize)]
struct HotelOffer {
    price: OfferPrice,
}

#[derive(Deserialize)]
struct OfferPrice {
    currency: String,
    total: String,
}

pub async fn hotels_in_city(
    city_code: IataCode,
    currency_code: &str,
    max_total: f32,
) -> Result<Vec<Hotel>, Box<dyn std::error::Error + Send + Sync>> {
    let bearer_token = generate_bearer_token(
        &env::var("AMADEUS_API_KEY").unwrap(),
        &env::var("AMADEUS_API_SECRET").unwrap(),
    )
    .await?;

    // Basic query by city; Amadeus supports more params like dates which can be added later
    let url = format!(
        "{BASE_URL}?cityCode={city_code}&currencyCode={currency_code}"
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
            "Amadeus hotels API error: {}\nMessage: {}",
            resp.status(),
            resp.text().await?
        )
        .into());
    }

    let payload: HotelOffersResponse = resp.json().await?;

    let mut hotels: Vec<Hotel> = Vec::new();
    for item in payload.data.into_iter() {
        if item.offers.is_empty() {
            continue;
        }
        // Pick the first offer for pricing comparison
        let offer = &item.offers[0];
        let numeric_total: f32 = match offer.price.total.parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        if numeric_total > max_total {
            continue;
        }

        let price = Currency::parse_currency(&offer.price.currency, &offer.price.total)?;

        let address_line = item
            .hotel
            .address
            .as_ref()
            .map(|addr| {
                let mut parts: Vec<String> = Vec::new();
                if let Some(lines) = &addr.lines {
                    if !lines.is_empty() {
                        parts.push(lines.join(", "));
                    }
                }
                if let Some(city) = &addr.cityName {
                    parts.push(city.to_string());
                }
                if let Some(cc) = &addr.countryCode {
                    parts.push(cc.to_string());
                }
                parts.join(", ")
            })
            .unwrap_or_else(|| "".to_string());

        hotels.push(Hotel {
            name: item.hotel.name,
            latitude: item.hotel.latitude.unwrap_or(0.0),
            longitude: item.hotel.longitude.unwrap_or(0.0),
            image_url: get_place_image_url(&address_line).await?,
            address: address_line,
            price,
        });
    }

    Ok(hotels)
}
