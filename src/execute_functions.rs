use crate::api_requests::{
    flights::{
        TokenMap, between, booking_details, flight_booking_details, flight_booking_link,
        flights_between, resolve_token,
    },
    hotel::{
        get_hotel_by_coordinates, get_hotel_description, get_hotel_details, get_room_availability,
    },
    site_seen::get_about_place,
    trains::{train_seats_available, trains_between},
};
use futures::future::join_all;
use gemini_client_api::gemini::{
    types::{request::Role, sessions::Session},
    utils::execute_function_calls,
};
use serde_json::{Value, json, to_value};
use std::error::Error;

fn update_session(
    name: String,
    session: &mut Session,
    result: Result<Value, Box<dyn Error + Send + Sync>>,
) {
    let response = match result {
        Ok(val) => val,
        Err(e) => serde_json::json!({"Error":e.to_string()}),
    };
    session.add_function_response(name, response).unwrap();
}

pub async fn execute_calls(session: &mut Session, token_map: &TokenMap) {
    let last_chat = if *session.get_last_chat().unwrap().role() == Role::Function {
        session.get_previous_chat(2).unwrap()
    } else {
        session.get_last_chat().unwrap()
    };

    // Collect (name, future) pairs without awaiting
    let mut futures: Vec<(
        String,
        std::pin::Pin<
            Box<
                dyn std::future::Future<Output = Result<Value, Box<dyn Error + Send + Sync>>>
                    + Send,
            >,
        >,
    )> = Vec::new();

    for call in last_chat.get_function_calls() {
        let args = call.args().as_ref().unwrap();
        let name = call.name().to_string();
        let tm = token_map.clone();

        if call.name() == "flights_between" {
            let args = args.clone();
            futures.push((
                name,
                Box::pin(async move {
                    let (
                        origin,
                        destination,
                        date,
                        travel_class,
                        adults,
                        children,
                        infant_on_lap,
                        infant_in_seat,
                        search_type,
                    ) = flights_between::parse_arguments(&args)?;
                    between(
                        origin,
                        destination,
                        date,
                        travel_class,
                        adults,
                        tm,
                        children,
                        infant_on_lap,
                        infant_in_seat,
                        search_type,
                    )
                    .await
                }),
            ));
        } else if call.name() == "flight_booking_details" {
            let args = args.clone();
            futures.push((
                name,
                Box::pin(async move {
                    let (booking_token,) = flight_booking_details::parse_arguments(&args)?;
                    let booking_token = resolve_token(&*tm.lock().await, &booking_token)?;
                    let response = booking_details(booking_token, tm).await?;
                    Ok(to_value(response).unwrap())
                }),
            ));
        } else if call.name() == "flight_booking_link" {
            let args = args.clone();
            futures.push((
                name,
                Box::pin(async move {
                    let resolved = resolve_token(
                        &*tm.lock().await,
                        &args["token"].as_str().unwrap_or_default(),
                    )?
                    .to_string();
                    let (token,) = flight_booking_link::parse_arguments(&args)?;
                    let url = flight_booking_link(resolved).await?;
                    Ok(json!({
                        "url_for": token,
                        "url": url
                    }))
                }),
            ));
        }
    }
    futures.push((
        "".into(),
        Box::pin(async {
            let _ = execute_function_calls!(
                session,
                train_seats_available,
                trains_between,
                get_about_place,
                get_hotel_by_coordinates,
                get_hotel_details,
                get_room_availability,
                get_hotel_description,
            );
            Ok("".into())
        }),
    ));

    // Execute all futures concurrently
    let names: Vec<String> = futures.iter().map(|(n, _)| n.to_string()).collect();
    let futs: Vec<_> = futures.into_iter().map(|(_, f)| f).collect();
    let results = join_all(futs).await;

    for (function_name, result) in names.into_iter().zip(results) {
        if function_name.len() == 0 {
            continue;
        }
        update_session(function_name, session, result);
    }
}

#[tokio::test]
async fn execute_calls_test() {
    use gemini_client_api::gemini::types::request::{FunctionCall, PartType};
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    let mut session = Session::new(10);
    let token_map: TokenMap = Arc::new(Mutex::new(Vec::new()));

    // 1. Test flights_between call via execute_calls
    let call = FunctionCall::new(
        "flights_between".to_string(),
        Some(json!({
            "origin": "GOI",
            "destination": "IXR",
            "date": crate::utils::Date::new_now(),
            "travel_class": "ECONOMY",
            "adults": 1,
            "children":0,
            "search_type":"cheap"
        })),
    );
    session.reply_parts(vec![call.into()]);

    println!("Executing flights_between via execute_calls...");
    execute_calls(&mut session, &token_map).await;

    // Verify session has response
    assert_eq!(session.get_history_length(), 2);
    let last_chat = session.get_last_chat().unwrap();
    assert_eq!(*last_chat.role(), Role::Function);

    // Verify token_map is populated
    assert!(
        !token_map.lock().await.is_empty(),
        "Token map should be populated after flights_between. session\n{session:?}"
    );
    let first_token_placeholder = "TOKEN_0";
    println!("Token map size: {}", token_map.lock().await.len());

    // 2. Test flight_booking_details call via execute_calls
    let call_details = FunctionCall::new(
        "flight_booking_details".to_string(),
        Some(json!({
            "booking_token": first_token_placeholder
        })),
    );
    session.reply_parts(vec![call_details.into()]);

    println!("Executing flight_booking_details via execute_calls...");
    execute_calls(&mut session, &token_map).await;

    // Verify session has response
    assert_eq!(session.get_history_length(), 4, "{session:?}");

    // 3. Test flight_booking_link call via execute_calls
    // After flight_booking_details, we should have more tokens in the map
    let second_token_placeholder = format!(
        "{}{}",
        crate::api_requests::flights::TOKEN_PREFIX,
        token_map.lock().await.len() - 1
    );

    let call_link = FunctionCall::new(
        "flight_booking_link".to_string(),
        Some(json!({
            "token": second_token_placeholder
        })),
    );
    session.reply_parts(vec![call_link.into()]);

    println!("Executing flight_booking_link via execute_calls...");
    execute_calls(&mut session, &token_map).await;

    // Verify session has response
    assert_eq!(session.get_history_length(), 6);
    let last_response = session.get_last_chat().unwrap().parts()[0].data();
    if let PartType::FunctionResponse(resp) = last_response {
        assert_eq!(resp.name(), "flight_booking_link");
        // add_function_response wraps non-object responses in a {"result": ...} object
        assert!(
            resp.response()["url"]
                .as_str()
                .unwrap()
                .starts_with("https://"),
        );
    } else {
        panic!("Expected FunctionResponse");
    }

    println!("execute_calls_test passed successfully!");
}
