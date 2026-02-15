use crate::utils::{Date, IataCode};
use futures::future::join_all;
use gemini_client_api::gemini::types::request::{PartType, Role};
use gemini_client_api::gemini::types::sessions::Session;
use gemini_client_api::gemini::utils::{GeminiSchema, gemini_function, gemini_schema};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json, to_value};
use std::error::Error;
use std::sync::Arc;
use std::{env, mem};
use tokio::sync::Mutex;

pub type TokenMap = Arc<Mutex<Vec<String>>>;

const RAPID_API_HOST: &str = "google-flights2.p.rapidapi.com";
const BASE_URL: &str = "https://google-flights2.p.rapidapi.com";

#[derive(Deserialize, Serialize)]
#[gemini_schema]
#[allow(non_camel_case_types)]
pub enum TravelClass {
    ECONOMY,
    PREMIUMECONOMY,
    BUSINESS,
    FIRST,
}

const TOKEN_PREFIX: &str = "TOKEN_";
fn update_token_map(map: &mut Vec<String>, token: String) -> String {
    let placeholder = format!("{TOKEN_PREFIX}{}", map.len());
    map.push(token);
    placeholder
}

fn resolve_token(
    map: &Vec<String>,
    placeholder: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let idx: usize = placeholder[TOKEN_PREFIX.len()..]
        .parse()
        .map_err(|_| "Invalid token provided")?;
    map.get(idx)
        .cloned()
        .ok_or("Invalid token provided".into())
}

fn clean_and_replace_tokens(
    val: &mut Value,
    token_map: &mut Vec<String>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    for itinerary in val
        .as_array_mut()
        .ok_or("Invalid response. Itinerary not found")?
    {
        for flight in itinerary["flights"]
            .as_array_mut()
            .ok_or("Invalid response. Flights not found")?
        {
            flight
                .as_object_mut()
                .ok_or("Invalid response. flight is not an object")?
                .remove("airline_logo");
        }
        let obj_ref = itinerary
            .as_object_mut()
            .ok_or("Invalid response. Itinerary not found")?;
        obj_ref.remove("airline_logo");
        obj_ref.remove("carbon_emissions");
        if let Some(booking_token) = obj_ref.get_mut("booking_token") {
            let small_token =
                update_token_map(token_map, booking_token.as_str().unwrap().to_string());
            obj_ref.insert("booking_token".into(), small_token.into());
        }
    }
    Ok(())
}

#[gemini_function]
/// Search for one-way flights between two cities on a specific date using Google Flights.
/// Returns a list of flight itineraries. Each itinerary contains a 'booking_token' (e.g., TOKEN_0)
/// which MUST be passed to 'flight_booking_details' to get actual booking options.
/// Price will be in INR.
pub async fn flights_between(
    /// starting airport
    origin: IataCode,
    /// destination airport
    destination: IataCode,
    /// The date of departure.
    date: Date,
    /// The class of travel (ECONOMY, PREMIUM_ECONOMY, BUSINESS, or FIRST).
    travel_class: TravelClass,
    /// Number of adult passengers (12+ years old).
    adults: u8,
    ///The number of child passengers (ages 2–11).
    children: u8,
    ///The count of infants traveling without a seat, sitting on an adult's lap (ages < 2).
    infant_on_lap: Option<u8>,
    ///The count of infants (below 2 years old) who require a separate seat.
    infant_in_seat: Option<u8>,
    ///Specifies the type of search strategy to apply when retrieving flight results.
    ///`best`: prioritizes a balanced mix of price, duration, and convenience.
    ///`cheap`: returns the lowest-cost options, possibly with longer layovers or travel time.
    search_type: String,
) -> Result<Value, Box<dyn Error + Send + Sync>> {
    todo!()
}
async fn between(
    origin: IataCode,
    destination: IataCode,
    date: Date,
    travel_class: TravelClass,
    adults: u8,
    token_map: TokenMap,
    children: u8,
    infant_on_lap: Option<u8>,
    infant_in_seat: Option<u8>,
    search_type: String,
) -> Result<Value, Box<dyn Error + Send + Sync>> {
    let api_key = env::var("RAPIDAPI_KEY")?;
    let client = reqwest::Client::new();

    let url = format!("{BASE_URL}/api/v1/searchFlights");

    let mut query = vec![
        ("departure_id", origin.to_string()),
        ("arrival_id", destination.to_string()),
        ("outbound_date", date.to_yyyy_mm_dd()),
        ("currency", "INR".to_string()),
        ("country_code", "IN".to_string()),
        ("adults", adults.to_string()),
        (
            "travel_class",
            to_value(travel_class)?.as_str().unwrap().to_string(),
        ),
        ("children", children.to_string()),
        ("search_type", search_type),
    ];
    if let Some(v) = infant_on_lap {
        query.push(("infant_on_lap", v.to_string()));
    }
    if let Some(v) = infant_in_seat {
        query.push(("infant_in_seat", v.to_string()));
    }

    let resp = client
        .get(&url)
        .header("x-rapidapi-key", api_key)
        .header("x-rapidapi-host", RAPID_API_HOST)
        .query(&query)
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await?;
        return Err(format!("Flight Search Error: {} - {}", status, text).into());
    }

    let mut val: Value = resp.json().await?;

    // Extract topFlights and clean them
    let mut top_flights = val
        .pointer_mut("/data/itineraries/topFlights")
        .ok_or("Field not found: /data/itineraries/topFlights")?;
    match top_flights.as_array().and_then(|e| Some(e.len() != 0)) {
        Some(true) => {}
        _ => {
            top_flights = val
                .pointer_mut("/data/itineraries/otherFlights")
                .ok_or("Field not found: /data/itineraries/otherFlights")?;
        }
    };
    clean_and_replace_tokens(top_flights, &mut *token_map.lock().await)?;
    Ok(mem::take(top_flights))
}

#[gemini_function]
/// Get detailed booking options for a specific flight itinerary.
/// Use this after 'flights_between' to see different ways to book the flight (e.g., directly with airline or via OTA).
/// Returns a list of booking options, each with a 'token' (e.g., TOKEN_1) that MUST be passed to 'flight_booking_link' to get the final URL.
/// Price will be in INR.
pub async fn flight_booking_details(
    /// The placeholder token (e.g., TOKEN_0) received from 'flights_between' for a specific itinerary.
    booking_token: String,
) -> Result<(Vec<Value>, Vec<String>), Box<dyn Error + Send + Sync>> {
    todo!()
}
async fn booking_details(
    booking_token: String,
    token_map: TokenMap,
) -> Result<Vec<Value>, Box<dyn Error + Send + Sync>> {
    let api_key = env::var("RAPIDAPI_KEY")?;
    let client = reqwest::Client::new();
    let url = format!("{BASE_URL}/api/v1/getBookingDetails");

    let response = client
        .get(&url)
        .header("x-rapidapi-key", api_key)
        .header("x-rapidapi-host", RAPID_API_HOST)
        .query(&[
            ("booking_token", booking_token),
            ("currency", "INR".to_string()),
            ("country_code", "IN".into()),
        ])
        .send()
        .await?;

    if !response.status().is_success() {
        return Err(format!("Booking Link Error: {}", response.status()).into());
    }
    let mut val: Value = response.json().await?;
    let data = val["data"].as_array_mut().ok_or("data not found")?;

    //Updating response with placeholder tokens
    let mut map = token_map.lock().await;
    for flights in data.iter_mut() {
        let flights = flights
            .as_object_mut()
            .ok_or("Invalid response format. Data don't have objects")?;

        if let Some(booking_token) = flights.get_mut("token") {
            let small_token =
                update_token_map(&mut map, booking_token.as_str().unwrap().to_string());
            flights.insert("token".into(), small_token.into());
        }
    }

    Ok(mem::take(data))
}

#[gemini_function]
/// Get the final booking URL for a specific booking option.
/// Returns object containing the "url" to the checkout page and the token passed in agrument.
pub async fn flight_booking_link(
    /// The placeholder token (e.g., TOKEN_1) received from 'flight_booking_details' for a specific booking option.
    token: String,
) -> Result<String, Box<dyn Error + Send + Sync>> {
    let api_key = env::var("RAPIDAPI_KEY")?;
    let client = reqwest::Client::new();

    let url = format!("{BASE_URL}/api/v1/getBookingURL");

    let resp = client
        .get(&url)
        .header("x-rapidapi-key", api_key)
        .header("x-rapidapi-host", RAPID_API_HOST)
        .query(&[("token", token)])
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(format!("Booking Link Error: {}", resp.status()).into());
    }

    let val: Value = resp.json().await?;

    if let Some(link) = val["data"].as_str() {
        Ok(link.to_string())
    } else {
        Err(val.to_string().into())
    }
}

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
        std::pin::Pin<Box<dyn std::future::Future<Output = Result<Value, Box<dyn Error + Send + Sync>>> + Send>>,
    )> = Vec::new();

    for part in last_chat.parts() {
        if let PartType::FunctionCall(call) = part.data() {
            let args = call.args().as_ref().unwrap();
            let name = call.name().to_string();
            let tm = token_map.clone();

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
                futures.push((name, Box::pin(async move {
                    let resolved = resolve_token(&*tm.lock().await, &args["booking_token"].as_str().unwrap_or_default())?
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
                })));
            } else if call.name() == "flight_booking_link" {
                let args = args.clone();
                futures.push((name, Box::pin(async move {
                    let resolved = resolve_token(&*tm.lock().await, &args["token"].as_str().unwrap_or_default())?
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
                })));
            }
        }
    }

    // Execute all futures concurrently
    let names: Vec<String> = futures.iter().map(|(n, _)| n.clone()).collect();
    let futs: Vec<_> = futures.into_iter().map(|(_, f)| f).collect();
    let results = join_all(futs).await;

    for (function_name, result) in names.into_iter().zip(results) {
        update_session(function_name, session, result);
    }
}

#[tokio::test]
async fn execute_calls_test() {
    use gemini_client_api::gemini::types::request::FunctionCall;
    use serde_json::json;

    let mut session = Session::new(10);
    let token_map: TokenMap = Arc::new(Mutex::new(Vec::new()));

    // 1. Test flights_between call via execute_calls
    let call = FunctionCall::new(
        "flights_between".to_string(),
        Some(json!({
            "origin": "GOI",
            "destination": "IXR",
            "date": Date::new_now(),
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
    let second_token_placeholder = format!("{TOKEN_PREFIX}{}", token_map.lock().await.len() - 1);

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
