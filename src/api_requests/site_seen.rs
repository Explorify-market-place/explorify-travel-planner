use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PlaceField {
    Id,
    DisplayName,
    FormattedAddress,
    Location,
    Rating,
    UserRatingCount,
    PriceLevel,
    Types,
    WebsiteUri,
    RegularOpeningHours,
    EditorialSummary,
    Photos,
    InternationalPhoneNumber,
    Reviews,
}

impl PlaceField {
    pub fn as_str(&self) -> &'static str {
        match self {
            PlaceField::Id => "places.id",
            PlaceField::DisplayName => "places.displayName",
            PlaceField::FormattedAddress => "places.formattedAddress",
            PlaceField::Location => "places.location",
            PlaceField::Rating => "places.rating",
            PlaceField::UserRatingCount => "places.userRatingCount",
            PlaceField::PriceLevel => "places.priceLevel",
            PlaceField::Types => "places.types",
            PlaceField::WebsiteUri => "places.websiteUri",
            PlaceField::RegularOpeningHours => "places.regularOpeningHours",
            PlaceField::EditorialSummary => "places.editorialSummary",
            PlaceField::Photos => "places.photos",
            PlaceField::InternationalPhoneNumber => "places.internationalPhoneNumber",
            PlaceField::Reviews => "places.reviews",
        }
    }
}

#[derive(Deserialize, Debug)]
struct TextSearchResponse {
    places: Option<Vec<Value>>,
}

pub async fn get_about_place(
    query: &str,
    max_results: u8,
    fields: Vec<PlaceField>,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let api_key = std::env::var("GOOGLE_MAPS_API_KEY")?;

    let client = reqwest::Client::new();
    let url = "https://places.googleapis.com/v1/places:searchText";

    let field_mask = if fields.is_empty() {
        "places.id,places.displayName,places.formattedAddress".to_string()
    } else {
        fields
            .iter()
            .map(|f| f.as_str())
            .collect::<Vec<_>>()
            .join(",")
    };

    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert("Content-Type", "application/json".parse()?);
    headers.insert("X-Goog-Api-Key", api_key.parse()?);
    headers.insert("X-Goog-FieldMask", field_mask.parse()?);

    let body = serde_json::json!({
        "textQuery": query,
        "maxResultCount": max_results,
    });

    let resp = client
        .post(url)
        .headers(headers)
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        let error_text = resp.text().await?;
        return Err(format!("Places API error: {}", error_text).into());
    }

    let payload: TextSearchResponse = resp.json().await?;
    Ok(payload.places.unwrap_or_default())
}
#[tokio::test]
async fn get_about_place_test() {
    dbg!(get_about_place(
        "Kashmir, manali",
        1,
        vec![PlaceField::DisplayName, PlaceField::FormattedAddress]
    )
    .await
    .unwrap());
}
