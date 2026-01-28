use std::vec;
use gemini_client_api::gemini::{
    ask::Gemini,
    types::{request::Tool, sessions::Session}, utils::GeminiSchema,
};

use crate::{api_requests::{flights::amadeus::{flight_seats_available, flights_between}, hotels::amadeus::hotels_in_city, site_seen::get_about_place, trains::rapidapi::{train_seats_available, trains_between}}, constants::TRAVEL_PLANNER_SYS_PROMPT};

pub async fn plan_tour(
    session: Session,
) -> Result<Session, Box<dyn std::error::Error + Send + Sync>> {
    let ai = Gemini::new(
        std::env::var("GEMINI_API_KEY").unwrap(),
        "gemini-3-flash-preview",
        Some(TRAVEL_PLANNER_SYS_PROMPT.into()),
    ).set_tools(vec![Tool::FunctionDeclarations(vec![hotels_in_city::gemini_schema(), flights_between::gemini_schema(), flight_seats_available::gemini_schema(), trains_between::gemini_schema(), train_seats_available::gemini_schema(), get_about_place::gemini_schema()])]);
    todo!()
}
