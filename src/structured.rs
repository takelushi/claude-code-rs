use schemars::{JsonSchema, schema_for};

use crate::error::ClaudeError;

/// Generates a JSON Schema string from a type implementing [`JsonSchema`].
///
/// Use the result with [`ClaudeConfigBuilder::json_schema`](crate::ClaudeConfigBuilder::json_schema)
/// to enable structured output from the CLI.
///
/// # Errors
///
/// Returns [`ClaudeError::ParseError`] if schema serialization fails.
pub fn generate_schema<T: JsonSchema>() -> Result<String, ClaudeError> {
    let schema = schema_for!(T);
    serde_json::to_string(&schema).map_err(ClaudeError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[allow(dead_code)]
    #[derive(schemars::JsonSchema)]
    struct TestStruct {
        name: String,
        count: i32,
    }

    #[test]
    fn generate_schema_returns_valid_json() {
        let schema_str = generate_schema::<TestStruct>().unwrap();
        let value: serde_json::Value = serde_json::from_str(&schema_str).unwrap();
        assert!(value.is_object());
    }

    #[test]
    fn generate_schema_contains_properties() {
        let schema_str = generate_schema::<TestStruct>().unwrap();
        let value: serde_json::Value = serde_json::from_str(&schema_str).unwrap();
        let props = value.get("properties").expect("should have properties");
        assert!(props.get("name").is_some());
        assert!(props.get("count").is_some());
    }
}
