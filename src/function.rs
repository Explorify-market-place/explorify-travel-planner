use crate::{
    api_requests::{
        flights::{execute_calls, flights_between, get_booking_link},
        hotels::hotels_in_city,
        site_seen::get_about_place,
        trains::{train_seats_available, trains_between},
    },
    constants::TRAVEL_PLANNER_SYS_PROMPT,
};
use gemini_client_api::gemini::{
    ask::Gemini,
    error::GeminiResponseError,
    types::{
        request::{Role, Tool},
        response::GeminiResponseStream,
        sessions::Session,
    },
    utils::{GeminiSchema, execute_function_calls},
};

async fn plan_tour(
    mut session: Session,
    token_map: &mut Vec<String>,
) -> Result<GeminiResponseStream, (Session, GeminiResponseError)> {
    let tools = vec![
        hotels_in_city::gemini_schema(),
        flights_between::gemini_schema(),
        get_booking_link::gemini_schema(),
        trains_between::gemini_schema(),
        train_seats_available::gemini_schema(),
        get_about_place::gemini_schema(),
    ];
    let ai = Gemini::new(
        std::env::var("GEMINI_API_KEY").unwrap(),
        "gemini-3-flash-preview",
        Some(TRAVEL_PLANNER_SYS_PROMPT.to_string().into()),
    )
    .set_tools(vec![Tool::FunctionDeclarations(tools)]);

    let results = execute_function_calls!(
        session,
        hotels_in_city,
        get_booking_link,
        train_seats_available,
        trains_between,
        get_about_place,
    );
    println!("Function call response: {results:?}");
    execute_calls(&mut session, token_map).await;

    if let Some(chat) = session.get_last_chat() {
        if *chat.role() == Role::Function {
            println!(
                "FunctionResponse:\n{}",
                serde_json::to_string(chat.parts()).unwrap()
            )
        }
    }
    ai.ask_as_stream(session).await
}

pub async fn handle_request(
    session: Session,
    token_map: &mut Vec<String>,
) -> Result<GeminiResponseStream, (Session, GeminiResponseError)> {
    plan_tour(session, token_map).await
}
