use crate::utils::{Date, IataCode};
use gemini_client_api::gemini::types::request::{PartType, Role};
use gemini_client_api::gemini::types::sessions::Session;
use gemini_client_api::gemini::utils::{GeminiSchema, gemini_function, gemini_schema};
use serde::{Deserialize, Serialize};
use serde_json::{Value, to_value};
use std::error::Error;
use std::{env, mem};

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

fn resolve_token<'a>(
    map: &'a Vec<String>,
    placeholder: &str,
) -> Result<&'a String, Box<dyn std::error::Error + Send + Sync>> {
    let idx: usize = placeholder[TOKEN_PREFIX.len()..]
        .parse()
        .map_err(|_| "Invalid token provided")?;
    map.get(idx).ok_or("Invalid token provided".into())
}

fn clean_and_replace_tokens(
    val: &mut Value,
) -> Result<Vec<String>, Box<dyn std::error::Error + Send + Sync>> {
    let mut token_map = Vec::new();
    for itinerary in val
        .as_array_mut()
        .ok_or("Invalid response. Itinerary not found")?
    {
        let obj_ref = itinerary
            .as_object_mut()
            .ok_or("Invalid response. Itinerary not found")?;
        obj_ref.remove("airline_logo");
        obj_ref.remove("carbon_emissions");
        if let Some(booking_token) = obj_ref.get_mut("booking_token") {
            let small_token = update_token_map(&mut token_map, booking_token.as_str().unwrap().to_string());
            obj_ref.insert("booking_token".into(), small_token.into());
        }
    }
    Ok(token_map)
}

#[gemini_function]
///returns flight between two station at a given time.
pub async fn flights_between(
    origin: IataCode,
    destination: IataCode,
    date: Date,
    travel_class: TravelClass,
    adults: u8,
) -> Result<(Value, Vec<String>), Box<dyn Error + Send + Sync>> {
    #[cfg(test)]
    {
        let mut val: Value = serde_json::from_str(include_str!("../../google-flights.json"))?;
        let top_flights = val
            .pointer_mut("")
            .ok_or("Path not found: /")?;
        let token_map = clean_and_replace_tokens(top_flights)?;
        return Ok((mem::take(top_flights), token_map));
    }

    let api_key = env::var("RAPIDAPI_KEY")?;
    let client = reqwest::Client::new();

    let url = format!("{BASE_URL}/api/v1/searchFlights");

    let resp = client
        .get(&url)
        .header("x-rapidapi-key", api_key)
        .header("x-rapidapi-host", RAPID_API_HOST)
        .query(&[
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
        ])
        .send()
        .await?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await?;
        return Err(format!("Flight Search Error: {} - {}", status, text).into());
    }

    let mut val: Value = resp.json().await?;

    // Extract topFlights and clean them
    let top_flights = val
        .pointer_mut("/data/itineraries/topFlights")
        .ok_or("Path not found: /data/itineraries/topFlights")?;

    let token_map = clean_and_replace_tokens(top_flights)?;

    Ok((mem::take(top_flights), token_map))
}

#[gemini_function]
///Get flight booking details and booking link(You will recieve a token which should be passed to
///get_booking_link()) from different platforms
pub async fn flight_booking_details(
    ///Provided by flights_between eg. TOKEN_0
    booking_token: String,
) -> Result<(Vec<Value>, Vec<String>), Box<dyn Error + Send + Sync>> {
    #[cfg(test)]
    {
        let mut val: Value = serde_json::from_str(include_str!("../../details.json"))?;
        let data = val["data"].as_array_mut().ok_or("data not found")?;
        let mut token_map = Vec::new();
        for flights in data.iter_mut() {
            let flights = flights
                .as_object_mut()
                .ok_or("Invalid response format. Data don't have objects")?;
            if let Some(booking_token) = flights.get_mut("token") {
                let small_token = update_token_map(&mut token_map, booking_token.as_str().unwrap().to_string());
                flights.insert("token".into(), small_token.into());
            }
        }
        return Ok((mem::take(data), token_map));
    }

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
    let mut token_map = Vec::new();
    for flights in data.iter_mut() {
        let flights = flights
            .as_object_mut()
            .ok_or("Invalid response format. Data don't have objects")?;

        if let Some(booking_token) = flights.get_mut("token") {
            let small_token = update_token_map(&mut token_map, booking_token.as_str().unwrap().to_string());
            flights.insert("token".into(), small_token.into());
        }
    }

    Ok((mem::take(data), token_map))
}

#[gemini_function]
///returns flight booking link for a given booking_token
pub async fn flight_booking_link(
    ///Provided by get_booking_details eg. TOKEN_0
    token: String) -> Result<String, Box<dyn Error + Send + Sync>> {
    #[cfg(test)]
    {
        return Ok("https://www.google.com/flights?mock_link".to_string());
    }

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
        Ok(val) => {
            println!("Function call {name} response:\n{val}");
            val
        }
        Err(e) => {
            println!("Function call {name} failed:\n{e}");
            serde_json::json!({"Error":e.to_string()})
        }
    };
    session.add_function_response(name, response).unwrap();
}

pub async fn execute_calls(session: &mut Session, token_map: &mut Vec<String>) {
    let mut results = Vec::new();
    let last_chat = if *session.get_last_chat().unwrap().role() == Role::Function {
        session.get_previous_chat(2).unwrap()
    } else {
        session.get_last_chat().unwrap()
    };
    for part in last_chat.parts() {
        if let PartType::FunctionCall(call) = part.data() {
            let args = call.args().as_ref().unwrap();
            if call.name() == "flights_between" {
                results.push((call.name().to_string(),flights_between::execute_with_closure(
                    args,
                    async |origin,
                           destination,
                           date,
                           travel_class,
                           adults|
                           -> Result<Value, Box<dyn Error + Send + Sync>> {
                        let (response, new_tokens) =
                            flights_between(origin, destination, date, travel_class, adults)
                                .await?;
                        token_map.extend(new_tokens);
                        Ok(response)
                    },
                )
                .expect("Wrong agrument format from gemini")
                .await)
                );
            } else if call.name() == "get_booking_details" {
                results.push((
                    call.name().to_string(),
                    flight_booking_details::execute_with_closure(
                        args,
                        async |booking_token| -> Result<Value, Box<dyn Error + Send + Sync>> {
                            let (response, new_tokens) = flight_booking_details(
                                resolve_token(token_map, &booking_token)?.to_string(),
                            )
                            .await?;
                            token_map.extend(new_tokens);
                            Ok(to_value(response).unwrap())
                        },
                    )
                    .expect("Wrong agrument format from gemini")
                    .await,
                ));
            } else if call.name() == "get_booking_link" {
                results.push((
                    call.name().to_string(),
                    flight_booking_link::execute_with_closure(
                        args,
                        async |token| -> Result<Value, Box<dyn Error + Send + Sync>> {
                            Ok(
                                flight_booking_link(resolve_token(token_map, &token)?.to_string())
                                    .await?
                                    .into(),
                            )
                        },
                    )
                    .expect("Wrong agrument format from gemini")
                    .await,
                ));
            }
        }
    }
    for (function_name, result) in results {
        update_session(function_name, session, result);
    }
}


#[tokio::test]
async fn execute_calls_test() {
    use gemini_client_api::gemini::types::request::FunctionCall;
    use serde_json::json;

    let mut session = Session::new(10);
    let mut token_map = Vec::new();

    // 1. Test flights_between call via execute_calls
    let call = FunctionCall::new(
        "flights_between".to_string(),
        Some(json!({
            "origin": "LAX",
            "destination": "JFK",
            "date": {"year": 2026, "month": 2, "day": 12},
            "travel_class": "ECONOMY",
            "adults": 1
        })),
    );
    session.reply_parts(vec![call.into()]);

    println!("Executing flights_between via execute_calls...");
    execute_calls(&mut session, &mut token_map).await;

    // Verify session has response
    assert_eq!(session.get_history_length(), 2);
    let last_chat = session.get_last_chat().unwrap();
    assert_eq!(*last_chat.role(), Role::Function);
    
    // Verify token_map is populated
    assert!(!token_map.is_empty(), "Token map should be populated after flights_between");
    let first_token_placeholder = "TOKEN_0";
    println!("Token map size: {}", token_map.len());

    // 2. Test get_booking_details call via execute_calls
    let call_details = FunctionCall::new(
        "get_booking_details".to_string(),
        Some(json!({
            "booking_token": first_token_placeholder
        })),
    );
    session.reply_parts(vec![call_details.into()]);

    println!("Executing get_booking_details via execute_calls...");
    execute_calls(&mut session, &mut token_map).await;

    // Verify session has response
    assert_eq!(session.get_history_length(), 4);
    
    // 3. Test get_booking_link call via execute_calls
    // After get_booking_details, we should have more tokens in the map
    // The details response (from details.json) has tokens that get replaced by placeholders
    // Let's assume there's at least one new token added.
    let second_token_placeholder = format!("{TOKEN_PREFIX}{}", token_map.len() - 1);
    
    let call_link = FunctionCall::new(
        "get_booking_link".to_string(),
        Some(json!({
            "token": second_token_placeholder
        })),
    );
    session.reply_parts(vec![call_link.into()]);

    println!("Executing get_booking_link via execute_calls...");
    execute_calls(&mut session, &mut token_map).await;

    // Verify session has response
    assert_eq!(session.get_history_length(), 6);
    let last_response = session.get_last_chat().unwrap().parts()[0].data();
    if let PartType::FunctionResponse(resp) = last_response {
        assert_eq!(resp.name(), "get_booking_link");
        // add_function_response wraps non-object responses in a {"result": ...} object
        assert_eq!(resp.response(), &json!({"result": "https://www.google.com/flights?mock_link"}));
    } else {
        panic!("Expected FunctionResponse");
    }

    println!("execute_calls_test passed successfully!");
}
