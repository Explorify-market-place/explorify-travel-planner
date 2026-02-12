use crate::utils::{Date, IataCode};
use gemini_client_api::gemini::types::request::{Chat, PartType};
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
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TravelClass {
    Economy,
    PremiumEconomy,
    Business,
    First,
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
            let small_token = update_token_map(&mut token_map, booking_token.to_string());
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
pub async fn get_booking_details(
    booking_token: String,
) -> Result<(Vec<Value>, Vec<String>), Box<dyn Error + Send + Sync>> {
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
            let small_token = update_token_map(&mut token_map, booking_token.to_string());
            flights.insert("token".into(), small_token.into());
        }
    }

    Ok((mem::take(data), token_map))
}

#[gemini_function]
///returns booking link for a given booking_token
pub async fn get_booking_link(token: String) -> Result<String, Box<dyn Error + Send + Sync>> {
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

async fn update_session<F, Fut>(name: &str, session: &mut Session, callback: F)
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Value, Box<dyn Error + Send + Sync>>>,
{
    let response = match callback().await {
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

pub async fn execute_call(last_chat: &Chat, session: &mut Session, token_map: &mut Vec<String>) {
    for part in last_chat.parts() {
        if let PartType::FunctionCall(call) = part.data() {
            let args = call.args().as_ref().unwrap();
            if call.name() == "flights_between" {
                flights_between::execute_with_closure(
                    args,
                    async |origin, destination, date, travel_class, adults| {
                        update_session(call.name(), session, async || {
                            let (response, new_tokens) =
                                flights_between(origin, destination, date, travel_class, adults)
                                    .await?;
                            token_map.extend(new_tokens);
                            Ok(response)
                        })
                        .await;
                    },
                )
                .expect("Wrong agrument format from gemini")
                .await;
            } else if call.name() == "get_booking_details" {
                get_booking_details::execute_with_closure(args, async |booking_token| {
                    update_session(call.name(), session, async || {
                        let (response, new_tokens) =
                            get_booking_details(resolve_token(token_map, &booking_token)?.clone())
                                .await?;
                        token_map.extend(new_tokens);
                        Ok(to_value(response).unwrap())
                    })
                    .await;
                })
                .expect("Wrong agrument format from gemini")
                .await;
            } else if call.name() == "get_booking_link" {
                get_booking_link::execute_with_closure(args, async |token| {
                    update_session(call.name(), session, async || {
                        Ok(get_booking_link(resolve_token(token_map, &token)?.clone())
                            .await?
                            .into())
                    })
                    .await;
                })
                .expect("Wrong agrument format from gemini")
                .await;
            }
        }
    }
}

#[tokio::test]
async fn flights_between_test() {
    println!(
        "{:?}",
        flights_between(
            IataCode::new("LAX".into()).unwrap(),
            IataCode::new("JFK".into()).unwrap(),
            Date::new(2026, 2, 12).unwrap(),
            TravelClass::Economy,
            1,
        )
        .await
        .unwrap()
    );
}
#[tokio::test]
async fn booking_details_test() {
    println!(
        "{:?}",
        get_booking_details(
 "W1syLDEsWyIxIiwwLDAsMF0sWyJDalJJYzI5RlJWRlZURmxCVmxGQlFtOVZNMEZDUnkwdExTMHRMUzB0TFc5NVkydHRNa0ZCUVV GQlIyMU5TVGd3U25aMVdsZEJFZ1ZCUVRFeE9Cb0xDTW45QVJBQUdnTkpUbEk0SFhDQm1BST0iXV0sWyIyMDI2LTAyLTEyIiwiTEFYIiwiSkZLIixbWyJMQVgiLCIyMDI2LTA yLTEyIiwiSkZLIiwiQUEiLCIxMTgiXV1dXQ==".to_string()
        )
        .await
        .unwrap()
    );
}
#[tokio::test]
async fn booking_links_test() {
    println!(
        "{:?}",
        get_booking_link(
 "ADowPOJPeleT1FwIDE2sPs5G43qsFdQvX5EExBefmsUASBYMmkjIa2qUQ8LOr7_J62QwTBdQwDlv0zogw1RMbgaEGuLhpM3-AtF-SLD XiwulrGBEEm9fVIWeY8qIuEfQgBidvA98dMZflLxTIPY_XGtDv-4JAc2-HuRqZsjC_7oTI1gGZGI4KmgKJEQHCM6yDLB-fbGJ8GJz8JjF3l7Zs_pFgyHtMlLcIN79q2LwejY tZMCPCqMCdSN8gdIWtsIFbP58Lw5tUMeiiZUUhajcd-I3m1A7s7Z6oD5wqOXWG8ICkz8xFfRWayqV1oxKe0Q2pXWc60OqFSqkr2PGq1toa7rGWp8_PiXYH40b_opfIk9LOij gkcM71koIyynj2wtwfD1VOhIBbS09bDMd57xSlwmZIUelgPhp7V9S0g0_zGbqRqjXp7rVoH5rsBch9LuQrra-hoc1XU128aWLxHv3IvgvM4zY_YrVntU_VnYtpvjhcNodbtI S9wVF2EQQcqvzcpovzSn4MLCnBWu6EFEdorCKtaR3LSYZM2lissUEsnpdsOoi4Fxls7K5iEJJD5fWXWW4DZjraumapNOLJ_RAJ_uLbQCaLJKKGPVwQ23wRWiEsMZTQsnyuPK ffJQsOPvUgUqrunAfixi17SYG7casyTHBN6A7akVZwQoqJykXNU1IDIk239XbITD5UYsQsnSuG41AXtDt-UpqWLrQmrspSsgWqmhyCdbjOpboI-E5nKBA_14DjCvYwYHEURK d-rc3zoeOolXjMoB_JDhv2ys7TGPjBm9Ffw8em9xLgBPghS8Uohao6W2TAStf22B4VF8YaQjPNy4DrQn1uGRCqb4WqEzahFE-Xs9Gn3iYH2E6r4-qRkjgdfEc7hyJ8ap5Agt WDsyAGZdRY2b4obU-GWi_3mrV-hldnDL0JocwK9-uCUxNIeHQ-fMrsJkOXskHpBI2loNm9JhulzlHNxJWkQ4ePs3uJtC0h2CNWL_2QSVU7sWSqCRR7VZ5Fu-I7UY6XYk8nyO 3SVlgRd1jZyky6LYNLXLvZ8blKjcvYF8-8mikSKBYfXTB__LNcN71VfllV2wtOHmE3m4iuw6Tkpm7EfUSvt3jPeOpTUIDqTvIpzZQ0-uqyLutdl2tw9RgND-X7X8isLMtfo_ EhPSzSezu15brKozIov_1dP50PU34-djOZcps7iW4vTHBy9aULyD2KyUZhENEysEKLk0Yx8bbfkNDgswiyPP4ErY1xb9ipZ_QgwkgmptmpodBuPupeL56RfjQ-HItXsclwXg T-_K47Q7_a80juWhgwguePNuVLeCw6aVpI_6_FjHAVGHBhYpsMOf3FIbg_J_GPOon8jdH_IbRI-B80nHec1dYb8k7hC5nJGE1wt2-3bKVp0fHIt0vD9TUJymaY4Cw1GD73zn djne27E8s6fHDZ0kwf8pb70YvIUMvrwY2ygNXPx9p5b2-cxfIa2ddMQ50v1i0H0havpDuyBvm8XfvnyBzHNFj7l_p3cXZGO9Qbbk2UNW9v1RgNbwwQlKJMWoTkqQjomgdlyX C0jirzS7fRy4NDJSI-ZMptMlPS3Q_C1r8UsCUe71jcrSdprywTjJ08VGG_bwpyqBzRDnlD84vURwZwW0f-h9OSYHhodRZ_cgVYaTagyzkh-wDbVvZoUdVMVtD9PsHq8CqY2b FLKlKuAH9vBlXnNJMpfzCTNCku3mmZ5moBTONg2Fj9gvJ_AtmjDCxYwNYWyBnm7w6u5I3EjzFD-fNBYY-_-EXfUAOn69R8Uc4gnfsu1sRJSxmiBV0jki-HaqK6D0vjUYuaM8 Jzu84rLSRTSMN3XcE3D-3H_RfTgUukLt6PzP_i1Ng_3AhgBEDSm3rEIU8AvgTm80TQKCc2WC8ctwVqA8q8HmuVzPNC42uQKMUocrRmNugJ0Rl5SjfUHd0LJi488AmZu72i_b INXPA0lZew-3Ju2wmcWteipM8FCvD-JyC0GEO9mxIdLucu5NuVuJ5dL096SkAXeIcJn7zLdtNWncJl5xhiTyEe-9KhV1NtSA3e0x_0fxCn4sk0vrPblXvl69-vVQx-VRGjO2 nV0rPKMnPKVDWOOP0ocI5-eNEY9b9g8XFVEWtbZnzeXMRX2Z5obiPQyfOc2V3VFtJ0hyXYsEMxhtM3vsjNUZ27LTSsvdBcIHdmbDu38tGARueN_nrex8oVym5uCuR8Rf4X-W yFMEVVYjRODHjmEZvwzH3kq_4peDlmhtUq2bHdXUSGFqRtl6NZvOOIDIZY_OuUCOUFbcqmCDDCSSqYkXMeaXsPGIdpO2QxX36CF3LSJtTKxjsloZo5SV8YnBf3X-r38hPZ2L V5Cflkth7Ok3YGwF3G_-FCWOwt3cStad8Z3M-2YrEhT6kNg6gGRLatjxRiq0fsR2m0sPy2YQARAWN46DvArcLVXoBoLvuNkzbuMxsilwsAaxk5EUaUpF7x3NS3pLT6oiPCj1 ADz8Io2usS_s2PY87VQckHzVUR0sZxByfJT6opvcwcvCMFV5dhBuBwvGUYAWSZDoMDJutvJZfJhmieDPerpgK_YB5UyrIJ_NcPtRvMj-AEJGIkYlr7hZYh_ql9Y2k5VZw-nq anTFKWk4zT2uMuKwqDScZVMVcqy6BcLaXJD5zFda9Otig0G03xW9ka1jKRQmfPpVtarYG1dkXBaHSo7hKCWbaEzbo8l5eAPe9hOqaxFhZvJvl6UFHVfsxujLtKJRTHVWRbVx Nx0DPTg__ANvSFISBuyVKHA1Kn2cm6h_UEF9gomsrAaGV7VrJwXE0JLqIUO79D5f7peIzV9FR6FwBGgWI-J-kX2n1XgbDVxs2iWRBhnfjLggJq18kodHlc8vNhFBcjzIkx1f FTK1a1_6zkN6KS_u2JJUTiXPd6C97GNGLqr1enfishxzUP2iJNdy64oE-8Y8K7jh5A6KqlMAlXHILjfpVlxGBd-zaPKCod3fc0KiT1_Lv3Bo4IHL2NwzBUapU9B0bfiNrZdI hFFcSWnlPG-_UMUHTcZA2CynEBmowW9cMUJn-0F00GerCyQwCWPa2a3iPEZ1ne30T7N6IJretWZsHmWqkh4mcrPwiJOCCAzsHqfO2fz-gDJg6lletcZxNor-8UlpO8fvs-db WHYYiQiKk6UB0Ofn2dhVIcwPTRGD8a-p8KaoLnwu0sm_fNf-z6HvawSb4-bPd0LHM_NmHklRunrC7CR-ymuzvorKXDX7rr-BRTNKh7LlZTiGa_yeF5PpsVBrLAhXLQs9qB8E f70PNSejQ_cCYr7FRbaiPonXIgPfyJszXYZzZp0mZyqDp-xNwGZNQ8g_KUvJTIGPZmcAMbh0yjUFrwW1Wx5H-HDesfbob8oS5mvbYRVHLwlRnvnrHLt_aTyEM82hcQXIfiTH QNrbFF3J0_tDzi7awrPu_lAgQnuLIIS5JcaL6gPsVMKyLE4HtlQ4OJ4ASviN-MWJ6FX7w6F92RdzRK8lNDEhkW3Zt-kJGnk4BpIn-BAxADKSqpLEKjwCFcgxYtDbeTDplunh LRtHmM-6EYQshggKbdC-WNwxq2VToJUIvLWIfs15uoCpU283DskcPTdhGYGrn7G7tDPx83sk3hTr6sxFrpWR45hr_YgOx6-R9i9Vz-f8XnGwZ6RiWyPxVU0wcx7JI5OWcEcA ArL0-NsQEkVnt26z3Iw1eDaEjxtcw161cHRMgseDs46ceu-sxFDcDcCbGv6fh1EuPi5plY_Yy7xm4B64GazCb4nmEaxCLV1ba-Ocj9qHaXZCoA00dlxkxnlwPTC1sI7Xgczv q79cGJxphCBsMgcqBeVNl-SNquwPqt9Mg_-4_LKANlLyx8ES2cmserwMfqefWuv7aE7pQ4Bw9q96xdwRpokxZqjdRo8ooC3IvfX_cHLPgf6dtZSxauAMGbNl6a6NipCgn14Z G6GLSFCVz9zlOveI1v3yj6BUFNMqmXRmEfUdX5YMyhXyV235zZdztWEpQHDhz5-3bNo3R4n2L2-vWaxv0d9JrTYZVNxb_pMwAS0xrJ9AvN_7DqM46JZdzkbMeDzezsWh3Z06 EnrQt_bh2_aC2JudLTAnEEf__8umu58enjGKoWU6vYh-yU6-fUhy6wksvDQ80t0r5dwnZ9iY_fCm49DUrLUY0W0raysJdZjATnpVHlbFndrHLiDX-lKCQ-QWhhS-r9DP8ahh 9kbjIth2DnR919pSKRHRqwXpsvrHC-2Toljzeuyvxkq5xkHRYl5kZquqN68le2IOZNH6Gjnt4Q8UCYvOSReOy9gcp2vEJ6RbLnq1aWRnlsKy_A2bm_QE60KFQjUVmacsESBQ t1kGv3LTl4EEsqYLKQI5EdCODvnRbpvoreSd1XMvxrrsUKoZgiKsyJeHRGxZdrEGm7ULNtOpEG-lfU_XTf_gDoIvK3wa_vnn_zvgb4sqMYLGeg1LY7eXZ93OBzVauOmHQodo -Qhu9OwcdU62hrQQc6Tx-hmWVc7-KSphCGdLNEseJ_FMlI0hKcvp00PPuB4HvwEtXVKNp5rfqWpDpaEtRSAxh4CBKNfuaiaMXOpr7VqGQw-DaraTk2XlTIpBQFf06N0T4lWD 8f__Y8v1M7NCWNMRBR4GVxlQDpdP30YPW-LJk9fqi-KKTACc2-gVb2e_sxay_5f7hkKdPtmZQD7wX0DJm9_bLvi031-JSWce2gj-285pQjKOfbcrEw_cJeVysQvsuMnvXkva Ehjy3eTfvDlnEW5664Tx5pgBB17qgToslgdBSMgXOy8vdOh22L3Qw2pt5vstEleuEre4mN0qRzCbgzaMk0ObEzX4Y342D4Hx9r8ecXEkcPa68P7tSg4K93KzkGBMt8mfYNCD Gr4R_KZJeVG8p7f31J6mOzEOYA4shsahgcYF-dSEl_yHpekBqQKy2eADemj4bWXkviOC1rgOJ3uZk-9l6Tj8DBryByEvngO4YyuN2KzZM479QdVpXWoXgWhDNXGJDjuNTagg 8gxiiV4gdfqLXC6F5vUKgtRSGAvNkUy402z04LCNPKEyVqOpCHR8k80qWEPO3sNkXqT7FtNF-VUAi0QYXVansLPNEzO0td9Mo2vaDPAya4S2OE1PQFpJeUvSt1bNR0zn-dZR jeCaYomTa3PCt3VG5lfv-TutxUKe_nhY-gwuiyBKQkPo8ph1dx7Giwn3fgZPnW3kKhf424_T08P7T-nRRC6YQ5wK6uxf0s_AvMzWwtNA3CB4pV7Awdg2DaLjK1EIOqvQjf46 mRtJ8NjRi2vTEHRTocuswvWlQAI7xbNTjkuMhtoZ60UCuWHbDx8_oi-iW7rmW2N2ku-OyS4Eo1UeiUfbHyX4cREmf-UkgexRxY8EFyQG0bR_7zy899gw1dtNCG7e2NkOGy7s HBevLyRwNOQJEoZC5o2OtJ0Xo3_Shee3ya0yN79HMp4HUtIM3B7x-l_mwd6ETOCbG5jpG6jzq1UhVgXoKuQsvJ_cnhTNoK4ESfPvkhIvrSBhO1Vnf5IMHBEQxxzofr4LOVsF NpFFMgYlDqQGmDgTaNDmNiSmg0PNmBbXwFv6bZ3nLeR5LX0xmNYV5s0YgUWLd7_ea86034_us6uiUubOjxmZ_e5VRTBM8M8rerwM6KTn1hDQiid5ANI-L_TNILMy2rsC1yvZ qOyt24JHr7FyZRny2EMH2hHYt-5_By5foQcXl29qus_vMsdIcrbmtvrcDz9WUp9VtCit3BdYeRxxfgqW3k2fkcJmDnjnUT59Js6UzfZZ5aSpoxGR6fYOd-2GYm6UdhNBsTLg DtA8POVBEF9U2OaG6YjXE9LKr3hTwch-B1c0W1n2qILjv3yd51Fo_MmYyKXWaAimAtP4eH2j4zD6Njd12dJu12_Oclgdvk2ijJSa9NkiWnQypGXBz7p3HiK4OfMqaDit1oce IkJghakVhlyfvaUF36dfytWw6aUAZqhCqyltWnubq6qmDH3Z61nteba6qRngr-ucPM-HYnwAhOUUCQOdSU17qmRFZbLYHG2wa9hdZ0lJqVPw0PcoWdo1Dp6NhJK2FyIEbaIe Ckkq5UdRNtwd9BeNkeJrl2b_pb0d2bagD5ZJr2BNQOS1p_oXWtI2-R64XVLySGqeFRT6siO37vlg8wvdK9X5SxqsiyWFrmbokmK4rKyIGd1aae0up-567xJKrJ60IJ3c92JG _E1R5o-O2TxjVw4OmOg0TCGCJNtpdwy8s9ex-mPLuixU1b78e5c09BPSn3Iu511NZ5oVeykDtYR3gWPE7w7fAbfHRTvRPPQuZHveHUm60Hx3AOIpJGpjrRchuDEiarllr_nK rCRB0c5ibDD6Uoiw8e3EscWO4CV7Mq6aCs1Qoucbii8_RMFyNHmXqRa2wS5YytzPKvZzTibf_-yf2cXIYUNQ0QI1N6uwRjDw8u7QejnruOdIaMTNouK93l3538ft3sFMniDJ fd4oz8ttCgmEN9UOfD66Md061znQcUTZYMLjDosL1CnM_Ip6yQHQ4DNlaScg_jP0de5FlD13KHeyi24gRR9gj0G7_5dCM04DaN3vKjuye45PeE3fqqQiDyYByoRO3G8zg3-L 0nak9Vjlpc1gifaA34AFRtAO6YBQm92ny52HfX-V8aCszGNCLDMKDejkFW2pzSXEWQ0p3MlhJ_x4A9PYpFXgqJW7QkcmNo1iXlrtbnnZyUWSxtI77gTrFz0G1E1uCFU-3iHf tK6MpiZOJnTZdeXjDFaJQ6F17760rvh7cRfAh3M69sCaCZOKBo_7BC4fWbrphgwwBe8TzgUSslfS_JLWR2jmk2dQbWGV6TIytUQmxjEwDxxeKdjB16ETik4IquETZFuXEtjC Qwypqy62Ph7_iX58hPrMNY30avoxIywrYWHMxbCFjIuCRKm4Iz-Jjsb64R4fxlAgTiwTKKTqPYcDYZvdaaakCoJuPQ7hBRbGJTgqe2eSeGmX8NhIHwL7fJh9u8asYqyORalo sGcHAMZcQp2Cs58fmZEq0NR3V-ufhkzc4gLiZfJsmYxRG2qEhDu3eu662a5kwJk9wLq8PMXkKsRCIk3LbmYe9Z9b8R1n0ktSlROC4jh09PIt4x8hA2XTFJCTx".to_string()
        )
        .await
        .unwrap()
    );
}
