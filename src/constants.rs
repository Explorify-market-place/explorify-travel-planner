use serde_json::{json, Value};
use std::sync::LazyLock;

pub static FUNCTION_SCHEMAS: LazyLock<Vec<Value>> = LazyLock::new(|| {
    vec![
        // From src/api_requests/trains/rapidapi.rs
        json!({
            "name": "trains_between",
            "description": "Search for trains running between two stations on a specific date.",
            "parameters": {
                "type": "object",
                "properties": {
                    "source": { "type": "string", "description": "Source station code (e.g., 'NDLS')" },
                    "destination": { "type": "string", "description": "Destination station code (e.g., 'BCT')" },
                    "date": {
                        "type": "object",
                        "properties": {
                            "year": { "type": "integer" },
                            "month": { "type": "integer" },
                            "day": { "type": "integer" }
                        },
                        "required": ["year", "month", "day"]
                    }
                },
                "required": ["source", "destination", "date"]
            }
        }),
        json!({
            "name": "train_details",
            "description": "Get full details of a train including its route and station stops.",
            "parameters": {
                "type": "object",
                "properties": {
                    "train_number": { "type": "string", "description": "Train number (e.g., '12002')" }
                },
                "required": ["train_number"]
            }
        }),
        json!({
            "name": "train_seats_available",
            "description": "Check seat availability and status for a specific train and class.",
            "parameters": {
                "type": "object",
                "properties": {
                    "train_number": { "type": "string" },
                    "from_station": { "type": "string" },
                    "to_station": { "type": "string" },
                    "date": {
                        "type": "object",
                        "properties": {
                            "year": { "type": "integer" },
                            "month": { "type": "integer" },
                            "day": { "type": "integer" }
                        },
                        "required": ["year", "month", "day"]
                    },
                    "class": { "type": "string", "description": "Class code (e.g., '2A', '3A', 'SL')" },
                    "quota": { "type": "string", "description": "Quota code (e.g., 'GN', 'TQ')" }
                },
                "required": ["train_number", "from_station", "to_station", "date", "class", "quota"]
            }
        }),
        // From src/api_requests/hotels/amadeus.rs
        json!({
            "name": "hotels_in_city",
            "description": "Find hotel offers in a city by IATA code with check-in date and budget constraints.",
            "parameters": {
                "type": "object",
                "properties": {
                    "city_code": { "type": "string", "description": "IATA city code (e.g., 'DEL')" },
                    "check_in_date": {
                        "type": "object",
                        "properties": {
                            "year": { "type": "integer" },
                            "month": { "type": "integer" },
                            "day": { "type": "integer" }
                        },
                        "required": ["year", "month", "day"]
                    },
                    "adults": { "type": "integer", "description": "Number of adult guests" },
                    "currency_code": { "type": "string", "description": "3-letter currency code (e.g., 'INR')" },
                    "budget": { "type": "number", "description": "Maximum budget amount" }
                },
                "required": ["city_code", "check_in_date", "adults", "currency_code", "budget"]
            }
        }),
        // From src/api_requests/flights/amadeus.rs
        json!({
            "name": "flights_between",
            "description": "Search for flight offers between two cities on a specific departure date.",
            "parameters": {
                "type": "object",
                "properties": {
                    "source": { "type": "string", "description": "IATA origin city code (e.g., 'JFK')" },
                    "destination": { "type": "string", "description": "IATA destination city code (e.g., 'LAX')" },
                    "least_departure": {
                        "type": "object",
                        "properties": {
                            "year": { "type": "integer" },
                            "month": { "type": "integer" },
                            "day": { "type": "integer" }
                        },
                        "required": ["year", "month", "day"]
                    },
                    "adult_count": { "type": "integer", "description": "Number of adult passengers" },
                    "currency_code": { "type": "string", "description": "3-letter currency code (e.g., 'USD')" }
                },
                "required": ["source", "destination", "least_departure", "adult_count", "currency_code"]
            }
        }),
        json!({
            "name": "flight_seats_available",
            "description": "Retrieve seat maps and availability for a specific flight offer ID.",
            "parameters": {
                "type": "object",
                "properties": {
                    "flight_offer_id": { "type": "string", "description": "The unique ID of the flight offer" }
                },
                "required": ["flight_offer_id"]
            }
        }),
        json!({
            "name": "get_about_place",
            "description": "Get detailed information about a specific location or point of interest using Google Places.",
            "parameters": {
                "type": "object",
                "properties": {
                    "location": { "type": "string", "description": "The name of the place to search for (e.g., 'Eiffel Tower', 'Manali')" }
                },
                "required": ["location"]
            }
        }),
    ]
});

pub const TRAVEL_PLANNER_SYS_PROMPT: &str = r#"You are Explorify AI, the lead travel architect at Explorify Trips Pvt. Ltd. Your mission is to craft exceptional, data-driven travel itineraries that seamlessly integrate flights, trains, hotels, and local attractions.

Guidelines:
1. Real-Time Precision: Use the provided tools to fetch live data for flights, trains, and hotels. Never hallucinate availability or prices.
2. Comprehensive Planning: A complete plan should ideally include transport (flight/train), accommodation (hotels), and a list of top sites to visit using 'get_site_seeing'.
3. User Clarification: If the user provides an incomplete request (e.g., missing destination, budget, travel dates, or passenger count), do not assume. Politely ask for the missing details to ensure accuracy.
4. Professional Tone: Maintain a helpful, knowledgeable, and professional demeanor.
5. Visual Structure: Use markdown tables and lists to present itineraries clearly. Use ![](image_url) to show site seens and images of hotels etc.

Tools at your disposal:
- flights_between && flight_seats_available: For air travel options in https://test.api.amadeus.com/v2/shopping/flight-offers api response format.
- trains_between & train_seats_available: For rail travel options in https://irctc1.p.rapidapi.com/api/v1/checkSeatAvailability api response format.
- hotels_in_city: Get all hotels in a city and their details in https://api.amadeus.com/v3/shopping/hotel-offers api response format.
- get_about_place: Get details about a place in https://maps.googleapis.com/maps/api/place/textsearch/json api response format."#;
