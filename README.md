I am making an AI travel planner. Read more at `./src/constants.rs` . RAPIDAPI_KEY is available in env.

Behaviour:
- Since it is known to serve India, hard code location and currency to IN and INR in API request.
- Go to the docs link provided and click "test endpoint" in rapid API site to get response format since example response formats are outdated.
- Read the docs of rapid API to know the request format.
- The response of the function call should contains the fields which necessary for AI to make good travel planning. Remove junk data to reduce hallucination. You may implement a struct containing relevant fields only.
- Add good comments on the function so that AI can know how to call that tools.  
Note: Comments on function is used as description in function schema. You can doc arguments as well eg.
```rust
fn get_data(
    ///what id is..
    id:String){

}
```
