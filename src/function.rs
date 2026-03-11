use std::time::Duration;

use crate::{
    api_requests::{
        flights::{TokenMap, flight_booking_details, flight_booking_link, flights_between},
        hotel::{get_hotel_by_coordinates, get_hotel_details, get_room_availability},
        site_seen::get_about_place,
        trains::{train_seats_available, trains_between},
    },
    constants::{PlanOutputSchema, RETRY_COUNT, TRAVEL_PLANNER_SYS_PROMPT},
    execute_functions::execute_calls,
};
use gemini_client_api::gemini::{
    ask::Gemini,
    error::{GeminiResponseError, Status},
    types::{
        request::{Role, ThinkingConfig, ThinkingLevel, Tool},
        response::GeminiResponseStream,
        sessions::Session,
    },
    utils::GeminiSchema,
};

pub async fn plan_tour(mut session: Session, token_map: &TokenMap) -> GeminiResponseStream {
    let tools = vec![
        flights_between::gemini_schema(),
        flight_booking_link::gemini_schema(),
        flight_booking_details::gemini_schema(),
        trains_between::gemini_schema(),
        train_seats_available::gemini_schema(),
        get_about_place::gemini_schema(),
        get_hotel_by_coordinates::gemini_schema(),
        get_hotel_details::gemini_schema(),
        get_room_availability::gemini_schema(),
    ];
    let mut ai = Gemini::new_with_client(
        std::env::var("GEMINI_API_KEY").unwrap(),
        "gemini-3-flash-preview",
        Some(TRAVEL_PLANNER_SYS_PROMPT.to_string().into()),
        reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .unwrap(),
    )
    .set_thinking_config(ThinkingConfig::new(false, ThinkingLevel::High))
    .set_json_mode(PlanOutputSchema::gemini_schema())
    .set_tools(vec![Tool::FunctionDeclarations(tools)]);
    ai.set_generation_config()["seed"] = 1.into();

    if Role::User == *session.get_last_chat().unwrap().role() {
        println!(
            "User: {}",
            session.get_last_chat().unwrap().get_text_no_think("\n")
        )
    } else {
        execute_calls(&mut session, token_map).await;
    }
    let mut retry_remaining = RETRY_COUNT;
    loop {
        match ai.ask_as_stream(session).await {
            Err((new_session, GeminiResponseError::StatusNotOk(e)))
                if e.error.status == Status::Unavailable =>
            {
                session = new_session;
                retry_remaining -= 1;
                if retry_remaining == 0 {
                    panic!(
                        "Model not available for {RETRY_COUNT} consecutive retries.\n{e}\nCrashing.."
                    )
                } else {
                    eprintln!("Model not available. {retry_remaining} retry remaining\n{e}")
                }
            }
            Err((session, e)) => panic!("ERROR: handle_request failed:\n{e:?}\n{:?}", session),
            Ok(v) => break v,
        }
    }
}
