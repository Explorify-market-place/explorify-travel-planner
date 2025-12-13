use std::env;
use gemini_client_api::gemini::{
    ask::Gemini,
    error::GeminiResponseError,
    types::{request::SystemInstruction, sessions::Session},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Deserialize, Serialize, Debug)]
pub struct Sites {
    location: String,
    description: String,
    image_url: Option<String>,
}
async fn ask(location: &str) -> Result<Vec<Sites>, GeminiResponseError> {
    let sites_schema: Value = json!({
      "type": "array",
      "items": {"type": "object",
      "properties": {
          "location": {"type":"string"},
          "description":{"type":"string"}
        }
      }
    });
    let ai = Gemini::new(
        env::var("GEMINI_API_KEY").expect("GEMINI_API_KEY not found in env"),
        "gemini-flash-lite-latest",
        Some(SystemInstruction::from_str(
            "You are a tour guide of a location. Your task is to list all the visitable sites and provide a brief description for each.",
        )),
    ).set_json_mode(sites_schema);
    let mut session = Session::new(2).set_remember_reply(false);
    let sites_response: Vec<Sites> = ai
        .ask(session.ask_string(format!("List all explorable sites in {location}")))
        .await?
        .get_json()
        .expect("Invalid json recieved from Gemini");
    Ok(sites_response)
}

#[derive(Deserialize, Debug)]
struct TextSearchPhoto {
    photo_reference: String,
}

#[derive(Deserialize, Debug)]
struct TextSearchResult {
    photos: Option<Vec<TextSearchPhoto>>,
}

#[derive(Deserialize, Debug)]
struct TextSearchResponse {
    results: Vec<TextSearchResult>,
    status: String,
}

/// Returns a URL to a photo of the given location using Google Places API.
///
/// Returns `Ok(None)` if no photo is available for the query.
pub async fn get_place_image_url(
    location: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let api_key = env::var("GOOGLE_MAPS_API_KEY")?;

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

    // 2) Extract the first available photo_reference
    let photo_reference = payload.results.into_iter().find_map(|r| {
        r.photos
            .and_then(|mut p| p.pop())
            .map(|p| p.photo_reference)
    });

    match photo_reference {
        Some(reference) => {
            // Construct a Google Places Photo endpoint URL. This URL will redirect to the actual image.
            // You can change maxwidth to control the requested image size.
            let photo_url = format!(
                "https://maps.googleapis.com/maps/api/place/photo?maxwidth=1600&photo_reference={}&key={}",
                reference, api_key
            );
            Ok(photo_url)
        }
        None => Err("No image found".into()),
    }
}
pub async fn get_site_seeing(
    location: &str,
) -> Result<Vec<Sites>, Box<dyn std::error::Error + Sync + Send>> {
    let mut retries = 3;
    let mut sites;
    loop {
        match ask(location).await {
            Ok(sites_vec) => {
                sites = sites_vec;
                break;
            }
            Err(GeminiResponseError::StatusNotOk(error)) => {
                if !error.contains("503") {
                    return Err(error.into());
                }
            }
            Err(error) => return Err(error.into()),
        }
        retries -= 1;
        if retries == 0 {
            return Err("s".into());
        }
    }
    for site in sites.as_mut_slice() {
        site.image_url = get_place_image_url(&site.location).await.ok();
    }
    Ok(sites)
}
#[tokio::test]
async fn test_site_seeing() {
    dbg!(get_site_seeing("kashmir").await.unwrap());
}
