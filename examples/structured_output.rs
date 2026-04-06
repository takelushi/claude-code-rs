// Structured output example using generate_schema and ask_structured.
// The CLI returns JSON matching the schema, which is deserialized into a Rust struct.
//
// Usage: cargo run --example structured_output

use schemars::JsonSchema;
use serde::Deserialize;

#[derive(Debug, Deserialize, JsonSchema)]
struct CityInfo {
    name: String,
    country: String,
    population: u64,
}

#[tokio::main]
async fn main() {
    let schema = claude_code_rs::generate_schema::<CityInfo>().expect("schema generation failed");

    let config = claude_code_rs::ClaudeConfig::builder()
        .model("haiku")
        .max_turns(1)
        .json_schema(&schema)
        .build();
    let client = claude_code_rs::ClaudeClient::new(config);

    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Tell me about Tokyo".into());

    match client.ask_structured::<CityInfo>(&prompt).await {
        Ok(city) => {
            println!("City: {}", city.name);
            println!("Country: {}", city.country);
            println!("Population: {}", city.population);
        }
        Err(e) => eprintln!("Error: {e}"),
    }
}
