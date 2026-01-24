use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize, Debug)]
struct TextSearchResponse {
    results: Vec<Value>,
    status: String,
}
pub async fn get_about_place(
    location: &str,
) -> Result<Vec<Value>, Box<dyn std::error::Error + Send + Sync>> {
    let api_key = std::env::var("GOOGLE_MAPS_API_KEY")?;

    // 1) Text search for the place to obtain a photo_reference
    let client = reqwest::Client::new();
    let text_search_url = "https://maps.googleapis.com/maps/api/place/textsearch/json";
    let resp = client
        .get(text_search_url)
        .query(&[("query", location), ("key", api_key.as_str())])
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(resp.status().as_str().into());
    }

    let payload: TextSearchResponse = resp.json().await?;
    if payload.status != "OK" {
        return Err(payload.status.into());
    }
    Ok(payload.results)
}
