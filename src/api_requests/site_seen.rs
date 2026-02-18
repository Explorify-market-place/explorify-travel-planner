use gemini_client_api::gemini::utils::{GeminiSchema, gemini_function, gemini_schema};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[gemini_schema]
/// Fields that can be requested for a place. Each field adds more detail to the response.  
/// Usage are following:
///- `Id`: Unique identifier for the place.
///- `DisplayName`: Name of the place (e.g., "Eiffel Tower").
///- `FormattedAddress`: Full address (e.g., "Champ de Mars, 5 Av. Anatole France, 75007 Paris, France").
///- `Location`: Latitude and longitude coordinates.
///- `Rating`: Average user rating (0-5).
///- `UserRatingCount`: Number of people who have rated this place.
///- `PriceLevel`: Price range (e.g., FREE, INEXPENSIVE, MODERATE, EXPENSIVE, VERY_EXPENSIVE).
///- `Types`: Categories for the place (e.g., "tourist_attraction", "park", "restaurant").
///- `WebsiteUri`: Link to the official website of the place.
///- `RegularOpeningHours`: Operating hours for each day of the week.
///- `EditorialSummary`: Short description or summary of the place.
///- `Photos`: List of photo references for the place.
///- `InternationalPhoneNumber`: Phone number in international format.
///- `Reviews`: Recent customer reviews and comments.
pub enum PlaceField {
    /// Unique identifier for the place.
    Id,
    /// Name of the place (e.g., "Eiffel Tower").
    DisplayName,
    /// Full address (e.g., "Champ de Mars, 5 Av. Anatole France, 75007 Paris, France").
    FormattedAddress,
    /// Latitude and longitude coordinates.
    Location,
    /// Average user rating (0-5).
    Rating,
    /// Number of people who have rated this place.
    UserRatingCount,
    /// Price range (e.g., FREE, INEXPENSIVE, MODERATE, EXPENSIVE, VERY_EXPENSIVE).
    PriceLevel,
    /// Categories for the place (e.g., "tourist_attraction", "park", "restaurant").
    Types,
    /// Link to the official website of the place.
    WebsiteUri,
    /// Operating hours for each day of the week.
    RegularOpeningHours,
    /// Short description or summary of the place.
    EditorialSummary,
    /// List of photo references for the place.
    Photos,
    /// Phone number in international format.
    InternationalPhoneNumber,
    /// Recent customer reviews and comments.
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

#[gemini_function]
/// Get detailed information about landmarks, tourist attractions, hotels, restaurants, or other points of interest.
/// Use this to find places in a city or get more info about a specific known spot.
/// Returns a list of places matching the query.
/// Note: It is safe to show image_url containing placeholder_api_key (found in photos field of
/// response) and will work properly.(due
/// to post processing)
pub async fn get_about_place(
    /// The name of the place to search for (e.g., 'Eiffel Tower', 'Best restaurants in Paris', 'Hotels near Central Park').
    query: String,
    /// Maximum number of results to return (1 to 20).
    max_results: u8,
    /// Specific fields to include in the response (e.g., Reviews, Rating, EditorialSummary).
    /// If empty, defaults to returning ID, Name, and Address.
    fields: Vec<PlaceField>,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let api_key = std::env::var("GOOGLE_MAPS_API_KEY").unwrap();

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

    let resp = client.post(url).headers(headers).json(&body).send().await?;

    if !resp.status().is_success() {
        let error_text = resp.text().await?;
        return Err(format!("Places API error: {}", error_text).into());
    }

    let payload: TextSearchResponse = resp.json().await?;
    let places = payload
        .places
        .unwrap_or_default()
        .into_iter()
        .map(|mut place| {
            if let Some(photos) = place["photos"].as_array_mut() {
                for photo in photos {
                    *photo = json!({
                        "googleMapsUri":photo["googleMapsUri"],
                        "image_url":get_place_image_url(photo["name"].as_str().expect("INVALID RESPONSE. Expected photos.name to be string"))
                    })
                }
            }
            place
        })
        .collect();
    Ok(places)
}
const PLACEHOLDER_API_KEY: &str = "placeholder_api_key";
fn get_place_image_url(photo_name: &str) -> String {
    format!(
        "https://places.googleapis.com/v1/{photo_name}/media?key={PLACEHOLDER_API_KEY}&maxHeightPx=400",
    )
}

#[tokio::test]
async fn get_about_place_test() {
    dbg!(
        get_about_place(
            "top tourist attractions in North Goa".into(),
            1,
            vec![
                PlaceField::DisplayName,
                PlaceField::EditorialSummary,
                PlaceField::Photos,
                PlaceField::Rating
            ]
        )
        .await
        .unwrap()
    );
}
