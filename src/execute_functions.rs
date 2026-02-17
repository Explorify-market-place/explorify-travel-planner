use crate::api_requests::{
    flights::{
        TokenMap, between, booking_details, flight_booking_details, flight_booking_link,
        flights_between, resolve_token,
    },
    hotel::{
        get_hotel_by_coordinates, get_hotel_description, get_hotel_details, get_room_availability,
    },
    site_seen::{get_about_place, get_place_image_url},
    trains::{train_seats_available, trains_between},
};
use futures::future::join_all;
use gemini_client_api::gemini::{
    types::{
        request::{PartType, Role},
        sessions::Session,
    },
    utils::execute_function_calls,
};
use serde_json::{Value, json, to_value};
use std::{
    error::Error,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::sync::Mutex;

static PROXY_COUNTER: AtomicU64 = AtomicU64::new(0);

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

pub async fn execute_calls(session: &mut Session, token_map: &TokenMap) -> Vec<(String, String)> {
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
    let proxy_url_map: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));

    for part in last_chat.parts() {
        if let PartType::FunctionCall(call) = part.data() {
            let args = call.args().as_ref().unwrap();
            let name = call.name().to_string();
            let tm = token_map.clone();
            let proxy_url_map = proxy_url_map.clone();

            if call.name() == "flights_between" {
                let args = args.clone();
                futures.push((name, Box::pin(async move {
                    flights_between::execute_with_closure(
                        &args,
                        async |origin, destination, date, travel_class, adults, children, infant_on_lap, infant_in_seat, search_type|
                            -> Result<Value, Box<dyn Error + Send + Sync>> {
                            between(origin, destination, date, travel_class, adults, tm, children, infant_on_lap, infant_in_seat, search_type).await
                        },
                    )
                    .expect("Wrong agrument format from gemini")
                    .await
                })));
            } else if call.name() == "flight_booking_details" {
                let args = args.clone();
                futures.push((
                    name,
                    Box::pin(async move {
                        let resolved = resolve_token(
                            &*tm.lock().await,
                            &args["booking_token"].as_str().unwrap_or_default(),
                        )?
                        .to_string();
                        flight_booking_details::execute_with_closure(
                            &args,
                            async |_booking_token| -> Result<Value, Box<dyn Error + Send + Sync>> {
                                let response = booking_details(resolved, tm).await?;
                                Ok(to_value(response).unwrap())
                            },
                        )
                        .expect("Wrong agrument format from gemini")
                        .await
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
                        flight_booking_link::execute_with_closure(
                            &args,
                            async |token| -> Result<Value, Box<dyn Error + Send + Sync>> {
                                let url = flight_booking_link(resolved).await?;
                                Ok(json!({
                                    "url_for": token,
                                    "url": url
                                }))
                            },
                        )
                        .expect("Wrong agrument format from gemini")
                        .await
                    }),
                ));
            } else if call.name() == "get_place_image_url" {
                let args = args.clone();
                futures.push((
                    name,
                    Box::pin(async move {
                        get_place_image_url::execute_with_closure(
                            &args,
                            async |name| -> Result<Value, Box<dyn Error + Send + Sync>> {
                                let base64 = get_place_image_url(name).await?;
                                let proxy_url = format!(
                                    "https://PROXY_{}",
                                    PROXY_COUNTER.fetch_add(1, Ordering::Relaxed)
                                );
                                let response = json!({"url":proxy_url});

                                proxy_url_map.lock().await.push((proxy_url, base64));
                                Ok(response)
                            },
                        )
                        .expect("Wrong agrument format from gemini")
                        .await
                    }),
                ));
            }
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
    Arc::into_inner(proxy_url_map).unwrap().into_inner()
}

#[tokio::test]
async fn execute_calls_test() {
    use gemini_client_api::gemini::types::request::FunctionCall;
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
    // The details response (from details.json) has tokens that get replaced by placeholders
    // Let's assume there's at least one new token added.
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
