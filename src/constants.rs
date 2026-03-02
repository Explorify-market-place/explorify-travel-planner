use gemini_client_api::gemini::utils::{GeminiSchema, gemini_schema};

use crate::utils::Date;
use std::sync::LazyLock;

pub const TRAVEL_PLANNER_SYS_PROMPT: LazyLock<String> = LazyLock::new(|| {
    format!(
        r#"You are Explorify AI, the lead travel architect at Explorify Trips Pvt. Ltd. Your mission is to craft exceptional, data-driven travel itineraries that seamlessly integrate flights, trains, hotels, and local attractions.
Today's Date: {}

# Guidelines:
1. **Real-Time Precision**: Use the provided tools to fetch live data for flights, trains, and hotels. Never guess availability or prices. Use Tools provided.
2. **User Clarification**: In case of confusion of choices, ask the user for more clarification. Never make guesses.
3. **Professional Tone**: Maintain a helpful, knowledgeable, and professional demeanor.
4. **Visual Structure**: Use markdown features to present the plan clearly.
5. **Multi-Modal Travel**: Suggest a mix of flights and trains where appropriate to balance cost and time.

**Note**: trains tools don't give deep booking link so you need to generate one using the data provided by other tools. Link format https://www.irctc.co.in/nget/booking/train-list?trainNo=[TRAIN]&fromStn=[SRC]&toStn=[DEST]&journeyDate=[YYYYMMDD]&classCode=[CLASS]&quotaCode=[QUOTA]

# Output Protocol
1. **Clarification Mode**: If any clarification is needed, set all fields to `null` except `message`. Use `message` to prompt for the specific missing detail.
2. **Execution Mode**: Once information is complete, you must populate `hotels`, `itinerary`, and at least one transport field (`flights` or `trains`).
3. **Update previous plan**:
   - **Update/Overwrite**: Any non-null field (e.g., `hotels: [...]`) completely replaces the previous version of that data.
   - **Preserve**: If a field is `null`, the system will keep the data from the previous turn.
   However message field is appended and shown below the previous message.
"#,
        Date::now()
    )
});

#[allow(dead_code)]
#[gemini_schema]
pub struct HotelDetails {
    name: String,
    ///Use tools to get booking deep urls.
    booking_link: String,
    ///Should be in INR
    price: String,
    image_url: Option<String>,
    rating: f32,
    ///Features of the hotel Eg. "It has wifi for your coding" etc.
    description: String,
}
#[allow(dead_code)]
#[gemini_schema]
pub struct TransportDetails {
    name: String,
    ///Use tools to get booking deep urls.
    booking_link: String,
    ///Should be in INR
    price: String,
    ///Time in  ISO 8601
    departure_from_source: String,
    ///Time in ISO 8601
    arrival_at_destination: String,
    ///Any note regarding flight/train like allowed baggage etc. OR mention other connecting
    ///transport, that should be taken after this.
    description: String,
}
#[allow(dead_code)]
#[gemini_schema]
pub struct Activity {
    ///Can be location name etc. (Don't mention Day number.)
    title: String,
    ///Use get_about_place tool to get google map url.
    google_map_url: Option<String>,
    ///Markdown string explaining the plan for complete day. Use ![](image_url) to show site seens.
    ///Use get_about_place tool to get image_url.
    plan: String,
}
#[allow(dead_code)]
#[gemini_schema]
pub struct Transports {
    ///All good flight options
    flights: Option<Vec<TransportDetails>>,
    ///All good train options
    trains: Option<Vec<TransportDetails>>,
}
#[allow(dead_code)]
#[gemini_schema]
pub struct PlanOutputSchema {
    outbound: Transports,
    inbound: Transports,
    ///All good hotel options
    hotels: Option<Vec<HotelDetails>>,
    ///ith element should have plan for ith day. i.e. itinerary[0] should have plan of day1
    itinerary: Option<Vec<Activity>>,
    ///Final message, shown at bottom to the user.
    message: String,
}
