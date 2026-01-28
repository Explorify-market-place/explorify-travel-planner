mod api_requests;
mod constants;
mod function;
mod utils;

use crate::function::handle_request;
use gemini_client_api::{futures::StreamExt, gemini::types::request::Role};
use lambda_runtime::{
    LambdaEvent, service_fn,
    streaming::{Body, Response, channel},
    tracing,
};
use serde::Deserialize;

const CHUNK_SEPERATOR: &str = "\n";

#[derive(Deserialize)]
struct EventBody {
    body: String,
}

async fn stream_handler(
    event: LambdaEvent<EventBody>,
) -> Result<Response<Body>, lambda_runtime::Error> {
    let (mut tx, rx) = channel();
    tokio::spawn(async move {
        loop {
            let response = handle_request(&event.payload.body).await;
            match response {
                Ok(mut response_stream) => {
                    while let Some(gemini_response) = response_stream.next().await {
                        match gemini_response {
                            Ok(data) => {
                                let response =
                                    serde_json::to_string(data.get_chat().parts()).unwrap();
                                print!("{response}");
                                let chunk = format!("{response}{CHUNK_SEPERATOR}").into();
                                tx.send_data(chunk).await.unwrap();
                            }
                            Err(error) => {
                                eprintln!("ERROR: Did not send stream due to error:\n{error}")
                            }
                        }
                    }
                    if *response_stream
                        .get_session()
                        .get_last_chat()
                        .unwrap()
                        .role()
                        != Role::Function
                    {
                        println!("Response streaming completed.");
                        break;
                    } else {
                        println!("Resolving function calls.")
                    }
                }
                Err(e) => {
                    eprintln!("ERROR: handle_request failed:\n{e}");
                    tx.send_data(e.to_string().into()).await.unwrap();
                    break;
                }
            }
        }
    });
    Ok(Response::from(rx))
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
    use crate::function::ApiRequest;
    use gemini_client_api::futures::StreamExt;
    use gemini_client_api::gemini::types::sessions::Session;
    use serde_json::to_string;

    let mut session = Session::new(10);
    session.ask_string("I want to travel to goa from ranchi");
    let body = to_string(&ApiRequest { session }).unwrap();

    let response = stream_handler(LambdaEvent {
        payload: EventBody { body },
        context: lambda_runtime::Context::default(),
    })
    .await
    .unwrap();
    let mut stream = response.stream;

    // EXPLICITLY CONSUME THE STREAM
    // This waits until 'tx' is dropped or closed in your spawn block
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(bytes) => {
                let s = String::from_utf8_lossy(&bytes);
                println!("Received chunk: {}", s);
            }
            Err(e) => panic!("Stream error: {:?}", e),
        }
    }
}
