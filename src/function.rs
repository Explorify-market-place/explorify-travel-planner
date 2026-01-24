use crate::{
    api_requests::{
        flights::amadeus::flights_between, hotels::amadeus::hotels_in_city,
        site_seen::get_site_seeing,
    },
    utils::{Date, IataCode},
};
use gemini_client_api::gemini::{
    ask::Gemini,
    types::{
        request::{SystemInstruction, ThinkingConfig, Tool},
        sessions::Session,
    },
};
use serde::Deserialize;
use serde_json::{json, to_string_pretty};
use std::env;
use tokio::join;

async fn ask_itata_code(
    source: &str,
    destination: &str,
) -> Result<(IataCode, IataCode), Box<dyn std::error::Error + Send + Sync>> {
    let ai = Gemini::new(
        env::var("GEMINI_API_KEY").unwrap(),
        "gemini-flash-lite-latest",
        Some(SystemInstruction::from_str(
            r#"Your are an travel planner. Your task is to tell:
1. ITATA code of nearest Airport from starting location. 
2. ITATA code of Airport nearest to destination."#,
        )),
    )
    .set_json_mode(json!({
        "type":"object",
        "properties":{
            "source_itata":{"type":"string"},
            "destination_itata":{"type":"string"}
        },
        "required":["source_itata", "destination_itata"]
    }))
    .set_thinking_config(ThinkingConfig::new_disable_thinking());
    let mut session = Session::new(2).set_remember_reply(false);

    #[derive(Deserialize)]
    struct ResponseSchema {
        source_itata: String,
        destination_itata: String,
    }
    let response: ResponseSchema = ai
        .ask(session.ask_string(format!(
            "I am at {source} and want to reach {destination} ASAP"
        )))
        .await?
        .get_json()?;

    Ok((
        IataCode::new(response.source_itata)?,
        IataCode::new(response.destination_itata)?,
    ))
}
pub async fn plan_tour(
    source: &str,
    destination: &str,
    adult_count: u8,
    least_departure: Date,
    currency_code: &str,
    budget: f32,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let (source_itata, destination_itata) = ask_itata_code(source, destination).await?;
    let flights = flights_between(
        source_itata,
        destination_itata.clone(),
        least_departure.clone(),
        adult_count,
        currency_code,
    );
    let site_seeing = get_site_seeing(destination);
    let hotels = hotels_in_city(
        destination_itata,
        least_departure.clone(),
        adult_count,
        currency_code,
        budget,
    );
    let (flights, site_seeing, hotels) = join!(flights, site_seeing, hotels);

    let ai = Gemini::new(
        env::var("GEMINI_API_KEY").unwrap(),
        "gemini-flash-lite-latest",
        Some(SystemInstruction::from_str(
            r#"You are an Itinerary maker. You will be provided all the sites to be visited, available hotels and flights. You are supposed to tell the best flight and hotel according to his preferences and complete travel plan.
            You output must be based on data provided.
            Use markdown to format your output and try to show images of hotels and sites to be seen."#,
        )),
    )
    .set_tools(vec![Tool::google_search(json!({}))])
    .set_thinking_config(ThinkingConfig::new_disable_thinking());
    let mut session = Session::new(2).set_remember_reply(false);
    let response = ai
        .ask(session.ask_string(format!(
            r#"I want to go {destination} from {source}.
        # Flights available are as follows:
        {}
        # Hotels in {destination}:
        {}
        # The sites to be seen:
        {}"#,
            to_string_pretty(&flights?).unwrap(),
            to_string_pretty(&hotels?).unwrap(),
            to_string_pretty(&site_seeing?).unwrap()
        )))
        .await?;
    dbg!(session.get_last_message().unwrap());
    Ok(response.get_text(""))
}
#[tokio::test]
async fn plan_test() {
    // Load environment variables from .env file for tests
    dotenv::dotenv().ok();

    dbg!(
        plan_tour(
            "Ranchi",
            "Delhi",
            2,
            Date::from_yyyy_mm_dd("2025-10-04").unwrap(),
            "INR",
            20000.0
        )
        .await
        .unwrap()
    );
}
