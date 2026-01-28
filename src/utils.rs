use gemini_client_api::gemini::utils::{GeminiSchema, gemini_schema};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt::Display;
use std::sync::LazyLock;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

const AUTH_URL: &str = "https://test.api.amadeus.com/v1/security/oauth2/token";

#[derive(Serialize, Deserialize, Debug, Clone)]
#[gemini_schema]
pub struct Date {
    year: u16,
    month: u8,
    day: u8,
}
impl Date {
    pub fn new(year: u16, month: u8, day: u8) -> Result<Self, String> {
        if day > 31 {
            return Err(format!("Day cannot be more than 31. Found: {day}"));
        }
        if month > 12 {
            return Err(format!("Month cannot be more than 12. Found: {month}"));
        }
        return Ok(Self { year, month, day });
    }
    pub fn to_yyyy_mm_dd(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
    pub fn from_yyyy_mm_dd(date: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut num = date.split('-');
        let year: u16 = num
            .next()
            .ok_or_else(|| "Year not found".to_string())?
            .parse()?;
        let month: u8 = num
            .next()
            .ok_or_else(|| "Month not found".to_string())?
            .parse()?;
        let day: u8 = num
            .next()
            .ok_or_else(|| "Day not found".to_string())?
            .parse()?;

        if let None = num.next() {
            Ok(Self::new(year, month, day)?)
        } else {
            Err("Too many parameters in data".into())
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Time {
    hour: u8,
    minute: u8,
    second: u8,
}
impl Time {
    pub fn new(hour: u8, minute: u8, second: u8) -> Result<Self, String> {
        if hour > 23 {
            return Err(format!("Hour cannot be more than 23. Found: {hour}"));
        }
        if minute > 59 {
            return Err(format!("Minute cannot be more than 59. Found: {minute}"));
        }
        if second > 59 {
            return Err(format!("Second cannot be more than 59. Found: {second}"));
        }
        Ok(Self {
            hour,
            minute,
            second,
        })
    }
    pub fn to_hh_mm_ss(&self) -> String {
        format!("{:02}:{:02}:{:02}", self.hour, self.minute, self.second)
    }
    pub fn from_hh_mm_ss(time: &str) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut parts = time.split(':');
        let hour = parts
            .next()
            .ok_or_else(|| "Hour not found".to_string())?
            .parse()?;
        let minute = parts
            .next()
            .ok_or_else(|| "Minute not found".to_string())?
            .parse()?;
        let second = parts
            .next()
            .ok_or_else(|| "Second not found".to_string())?
            .parse()?;

        if let None = parts.next() {
            Ok(Self::new(hour, minute, second)?)
        } else {
            Err("Too many parameters in time string".into())
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum Currency {
    Inr(f32),
    Usd(f32),
    Eur(f32),
}
impl Currency {
    pub fn parse_currency(
        code: &str,
        amount: &str,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let amount: f32 = amount.parse()?;
        let cur = match code.to_uppercase().as_str() {
            "USD" => Self::Usd(amount),
            "INR" => Self::Inr(amount),
            "EUR" => Self::Eur(amount),
            _ => return Err(format!("Unsupported currency: {}", code).into()),
        };
        Ok(cur)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IataCode(String);
impl GeminiSchema for IataCode {
    fn gemini_schema() -> serde_json::Value {
        json!({"type":"String"})
    }
}
impl IataCode {
    pub fn new(code: String) -> Result<Self, String> {
        if code.len() <= 3 && code.chars().all(|c| c.is_ascii_uppercase()) {
            Ok(Self(code))
        } else {
            Err(format!("Invalid ITATA code: {code}"))
        }
    }
}

impl Display for IataCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Deserialize, Clone)]
struct OAuthTokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
}

struct TokenCache {
    token: OAuthTokenResponse,
    expiry: Instant,
}

static TOKEN_STORAGE: LazyLock<RwLock<Option<TokenCache>>> = LazyLock::new(|| RwLock::new(None));

pub async fn get_bearer_token(
    client_id: &str,
    client_secret: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    {
        let cache = TOKEN_STORAGE.read().await;
        if let Some(ref entry) = *cache {
            // Buffer of 30s to prevent race conditions near expiry
            if entry.expiry > Instant::now() + Duration::from_secs(30) {
                return Ok(entry.token.access_token.clone());
            }
        }
    }

    let mut cache = TOKEN_STORAGE.write().await;

    if let Some(ref entry) = *cache {
        if entry.expiry > Instant::now() + Duration::from_secs(30) {
            return Ok(entry.token.access_token.clone());
        }
    }

    let params = [
        ("grant_type", "client_credentials"),
        ("client_id", client_id),
        ("client_secret", client_secret),
    ];

    let resp = reqwest::Client::new()
        .post(AUTH_URL)
        .form(&params)
        .send()
        .await?
        .error_for_status()?;

    let token: OAuthTokenResponse = resp.json().await?;

    *cache = Some(TokenCache {
        token: token.clone(),
        expiry: Instant::now() + Duration::from_secs(token.expires_in),
    });

    Ok(token.access_token)
}
