use gemini_client_api::gemini::{ask::Gemini, types::sessions::Session};

use crate::constants::TRAVEL_PLANNER_SYS_PROMPT;

pub async fn plan_tour(
    session: Session,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let ai = Gemini::new(std::env::var("GEMINI_API_KEY").unwrap(), "gemini-3-flash-preview", Some(TRAVEL_PLANNER_SYS_PROMPT.into()));
    todo!()
}
