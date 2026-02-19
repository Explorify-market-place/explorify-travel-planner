use chrono::{DateTime, Datelike, FixedOffset, Local, Utc};
use gemini_client_api::gemini::utils::{GeminiSchema, gemini_schema};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fmt::Display;

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
    pub fn now() -> String {
        let utc_now: DateTime<Utc> = Utc::now();
        let ist_offset = FixedOffset::east_opt(5 * 3600 + 30 * 60).unwrap();
        let ist_now = utc_now.with_timezone(&ist_offset);
        ist_now.format("%a %b %e %H:%M:%S %Z %Y").to_string()
    }
    pub fn new_now() -> Self {
        let local = Local::now();
        Date {
            year: local.year() as u16,
            month: local.month() as u8,
            day: local.day() as u8,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct IataCode(String);
impl GeminiSchema for IataCode {
    fn gemini_schema() -> serde_json::Value {
        json!({
            "description":"IATA code e.g. NDLS",
            "type":"String"
        })
    }
}
impl IataCode {
    pub fn new(code: impl Into<String>) -> Result<Self, String> {
        let code = code.into();
        if code.len() <= 3 && code.chars().all(|c| c.is_ascii_uppercase()) {
            Ok(Self(code))
        } else {
            Err(format!("Invalid IATA code: {code}"))
        }
    }
}
impl Display for IataCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
