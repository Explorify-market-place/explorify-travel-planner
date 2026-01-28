use crate::{
    api_requests::{
        flights::amadeus::{flight_seats_available, flights_between},
        hotels::amadeus::hotels_in_city,
        site_seen::get_about_place,
        trains::rapidapi::{train_seats_available, trains_between},
    },
    constants::TRAVEL_PLANNER_SYS_PROMPT,
};
use gemini_client_api::gemini::{
    ask::Gemini,
    error::GeminiResponseError,
    types::{request::Tool, response::GeminiResponseStream, sessions::Session},
    utils::{GeminiSchema, execute_function_calls},
};

async fn plan_tour(
    mut session: Session,
) -> Result<GeminiResponseStream, (Session, GeminiResponseError)> {
    let tools = vec![
        hotels_in_city::gemini_schema(),
        flights_between::gemini_schema(),
        flight_seats_available::gemini_schema(),
        trains_between::gemini_schema(),
        train_seats_available::gemini_schema(),
        get_about_place::gemini_schema(),
    ];
    let ai = Gemini::new(
        std::env::var("GEMINI_API_KEY").unwrap(),
        "gemini-3-flash-preview",
        Some(TRAVEL_PLANNER_SYS_PROMPT.to_string().into()),
    )
    .set_tools(vec![Tool::FunctionDeclarations(tools.clone())]);
    let results = execute_function_calls!(
        session,
        hotels_in_city,
        flights_between,
        flight_seats_available,
        train_seats_available,
        trains_between,
        get_about_place
    );
    for i in 0..tools.len() {
        if let Some(Err(e)) = &results[i] {
            session
                .add_function_response(
                    flights_between::name(&tools[i]).unwrap(),
                    serde_json::json!({"Error":e}),
                )
                .unwrap();
        }
    }
    ai.ask_as_stream(session).await
}

pub async fn handle_request(
    session: Session,
) -> Result<GeminiResponseStream, (Session, GeminiResponseError)> {
    plan_tour(session).await
}
