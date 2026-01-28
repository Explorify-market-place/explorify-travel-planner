mod api_requests;
mod constants;
mod function;
mod utils;

use crate::function::handle_request;
use gemini_client_api::{futures::StreamExt, gemini::types::sessions::Session};
use lambda_runtime::{
    LambdaEvent, service_fn,
    streaming::{Body, Response, channel},
    tracing,
};
use serde::{Deserialize, Serialize};
use serde_json::from_str;

const CHUNK_SEPERATOR: &str = "\n";

#[derive(Serialize, Deserialize)]
pub struct ApiRequest {
    pub session: Session,
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
    tokio::spawn(async move {
        loop {
            let response = handle_request(request.session).await;
            match response {
                Ok(mut response_stream) => {
                    while let Some(gemini_response) = response_stream.next().await {
                        match gemini_response {
                            Ok(data) => {
                                let response =
                                    serde_json::to_string(data.get_chat().parts()).unwrap();
                                println!("{response}");
                                let chunk = format!("{response}{CHUNK_SEPERATOR}").into();
                                tx.send_data(chunk).await.unwrap();
                            }
                            Err(error) => {
                                eprintln!("ERROR: Did not send stream due to error:\n{error}")
                            }
                        }
                    }
                    if !response_stream
                        .get_session()
                        .get_last_chat()
                        .unwrap()
                        .has_function_call()
                    {
                        println!("Response streaming completed.");
                        break;
                    } else {
                        println!("Resolving function calls.");
                        request.session = response_stream.get_session_owned();
                    }
                }
                Err((session, e)) => {
                    eprintln!("ERROR: handle_request failed:\n{e}\n{:?}", session);
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
    use gemini_client_api::futures::StreamExt;
    use gemini_client_api::gemini::types::sessions::Session;
    use serde_json::to_string;

    let mut session = Session::new(10);
    session.ask_string(r#"I want to travel to goa from ranchi
I'm planning a 7-day trip for 2 adults starting on February 15th. I prefer a flight (IXR to GOI/GOX) to save time for coding. I’m looking for a mid-range hotel near North Goa with good Wi-Fi. My budget is roughly ₹60,000 for the whole trip."#);
    let body = to_string(&ApiRequest { session }).unwrap();

    let response = stream_handler(LambdaEvent {
        payload: EventBody { body },
        context: lambda_runtime::Context::default(),
    })
    .await
    .unwrap();
    let mut stream = response.stream;
    while let Some(_) = stream.next().await {}
}
