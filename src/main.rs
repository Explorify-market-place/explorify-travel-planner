mod api_requests;
mod constants;
mod execute_functions;
mod function;
mod utils;

use crate::api_requests::flights::TokenMap;
use crate::function::plan_tour;
use gemini_client_api::gemini::types::request::Role;
use gemini_client_api::{futures::StreamExt, gemini::types::sessions::Session};
use lambda_runtime::{
    LambdaEvent, service_fn,
    streaming::{Body, Response, channel},
    tracing,
};
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string};
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;

const CHUNK_SEPERATOR: &str = "\n";

#[derive(Serialize, Deserialize)]
pub struct ApiRequest {
    pub session: Session,
    pub token_map: Vec<String>,
    pub secret: String,
}

#[derive(Deserialize)]
struct EventBody {
    body: String,
}

async fn stream_handler(
    event: LambdaEvent<EventBody>,
) -> Result<Response<Body>, lambda_runtime::Error> {
    let (mut tx, rx) = channel();
    let mut request: ApiRequest = from_str(&event.payload.body)?;
    if request.secret != env::var("API_SECRET").unwrap() {
        return Ok(rx.into());
    }

    let token_map: TokenMap = Arc::new(Mutex::new(request.token_map));
    tokio::spawn(async move {
        loop {
            let mut response_stream = plan_tour(request.session, &token_map).await;
            let last_chat = response_stream.get_session().get_last_chat().unwrap();
            if Role::Function == *last_chat.role() {
                println!("Sending\n{last_chat:?}");
                let chunk = format!("{}{CHUNK_SEPERATOR}", to_string(last_chat).unwrap()).into();
                tx.send_data(chunk).await.unwrap();
            }
            while let Some(gemini_response) = response_stream.next().await {
                match gemini_response {
                    Ok(data) => {
                        let response = data.get_chat();
                        println!("Sending\n{response:?}");
                        let chunk =
                            format!("{}{CHUNK_SEPERATOR}", to_string(response).unwrap()).into();
                        tx.send_data(chunk).await.unwrap();
                    }
                    Err(error) => {
                        eprintln!("ERROR: Did not send stream due to error:\n{error:?}");
                        break;
                    }
                }
            }
            if !response_stream
                .get_session()
                .get_last_chat()
                .unwrap()
                .has_function_call()
            {
                tx.send_data(to_string(&*token_map.lock().await).unwrap().into())
                    .await
                    .unwrap();
                println!("Response streaming completed.");
                println!(
                    "Last message:\n{}",
                    response_stream
                        .get_session_owned()
                        .get_last_chat()
                        .unwrap()
                        .get_text_all("\n")
                );
                break;
            } else {
                println!("Resolving function calls.");
                request.session = response_stream.get_session_owned();
            }
        }
    });
    Ok(rx.into())
}

#[tokio::main]
async fn main() -> Result<(), lambda_runtime::Error> {
    tracing::init_default_subscriber();
    match lambda_runtime::run(service_fn(stream_handler)).await {
        Ok(_) => {}
        Err(e) => eprint!("Error:\n{e}"),
    }
    Ok(())
}

#[tokio::test]
async fn stream_handler_test() {
    use gemini_client_api::futures::StreamExt;
    use gemini_client_api::gemini::types::sessions::Session;
    use serde_json::to_string;

    let mut session = Session::new(20);
    session.ask(format!(r#"I want to travel to goa from ranchi
I'm planning a 7-day trip for 2 adults starting on {}. I prefer a flight to save time for coding. I’m looking for a mid-range hotel near North Goa with good Wi-Fi. My budget is roughly ₹50,000 for the whole trip."#,crate::utils::Date::now()));
    let body = to_string(&ApiRequest {
        session,
        token_map: Vec::new(),
        secret: env::var("API_SECRET").unwrap(),
    })
    .unwrap();

    let response = stream_handler(LambdaEvent {
        payload: EventBody { body },
        context: lambda_runtime::Context::default(),
    })
    .await
    .unwrap();
    let mut stream = response.stream;
    while let Some(_) = stream.next().await {}
}
