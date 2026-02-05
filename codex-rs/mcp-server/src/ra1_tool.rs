//! RA1 Art Generator tool - generates AI images via netwrck.com API.

use mcp_types::CallToolResult;
use mcp_types::ContentBlock;
use mcp_types::TextContent;
use mcp_types::Tool;
use mcp_types::ToolInputSchema;
use schemars::JsonSchema;
use schemars::r#gen::SchemaSettings;
use serde::Deserialize;
use serde::Serialize;
use std::env;

const NETWRCK_API_KEY_ENV: &str = "NETWRCK_API_KEY";
const RA1_API_URL: &str = "https://netwrck.com/api/ra1-art-generator";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Ra1ArtGeneratorParams {
    /// The prompt describing the image to generate.
    pub prompt: String,

    /// Image size (e.g. "1024x1024", "1360x768"). Defaults to "1024x1024".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Ra1ArtGeneratorResponse {
    pub image_url: String,
    pub prompt_used: String,
    pub size_used: String,
    pub cost: String,
}

#[derive(Debug, Deserialize)]
pub struct Ra1ArtGeneratorError {
    pub error: String,
}

pub fn is_ra1_available() -> bool {
    env::var(NETWRCK_API_KEY_ENV).is_ok()
}

pub fn create_tool_for_ra1_art_generator() -> Tool {
    let schema = SchemaSettings::draft2019_09()
        .with(|s| {
            s.inline_subschemas = true;
            s.option_add_null_type = false;
        })
        .into_generator()
        .into_root_schema_for::<Ra1ArtGeneratorParams>();

    #[expect(clippy::expect_used)]
    let schema_value =
        serde_json::to_value(&schema).expect("RA1 tool schema should serialise to JSON");

    let tool_input_schema =
        serde_json::from_value::<ToolInputSchema>(schema_value).unwrap_or_else(|e| {
            panic!("failed to create Tool from schema: {e}");
        });

    Tool {
        name: "ra1-art-generator".to_string(),
        title: Some("RA1 Art Generator".to_string()),
        input_schema: tool_input_schema,
        output_schema: None,
        description: Some(
            "Generate AI images using the RA1 art generator. Returns an image URL.".to_string(),
        ),
        annotations: None,
    }
}

pub async fn handle_ra1_art_generator(arguments: Option<serde_json::Value>) -> CallToolResult {
    let api_key = match env::var(NETWRCK_API_KEY_ENV) {
        Ok(key) => key,
        Err(_) => {
            return CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: format!("{NETWRCK_API_KEY_ENV} environment variable not set"),
                    annotations: None,
                })],
                is_error: Some(true),
                structured_content: None,
            };
        }
    };

    let params: Ra1ArtGeneratorParams = match arguments {
        Some(json_val) => match serde_json::from_value(json_val) {
            Ok(p) => p,
            Err(e) => {
                return CallToolResult {
                    content: vec![ContentBlock::TextContent(TextContent {
                        r#type: "text".to_string(),
                        text: format!("Failed to parse parameters: {e}"),
                        annotations: None,
                    })],
                    is_error: Some(true),
                    structured_content: None,
                };
            }
        },
        None => {
            return CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: "Missing arguments; the `prompt` field is required.".to_string(),
                    annotations: None,
                })],
                is_error: Some(true),
                structured_content: None,
            };
        }
    };

    let size = params.size.unwrap_or_else(|| "1024x1024".to_string());

    let client = reqwest::Client::new();
    let request_body = serde_json::json!({
        "api_key": api_key,
        "prompt": params.prompt,
        "size": size
    });

    let response = match client
        .post(RA1_API_URL)
        .header("Content-Type", "application/json")
        .json(&request_body)
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            return CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: format!("HTTP request failed: {e}"),
                    annotations: None,
                })],
                is_error: Some(true),
                structured_content: None,
            };
        }
    };

    let status = response.status();
    let body = match response.text().await {
        Ok(b) => b,
        Err(e) => {
            return CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: format!("Failed to read response body: {e}"),
                    annotations: None,
                })],
                is_error: Some(true),
                structured_content: None,
            };
        }
    };

    if !status.is_success() {
        if let Ok(err) = serde_json::from_str::<Ra1ArtGeneratorError>(&body) {
            return CallToolResult {
                content: vec![ContentBlock::TextContent(TextContent {
                    r#type: "text".to_string(),
                    text: format!("API error: {}", err.error),
                    annotations: None,
                })],
                is_error: Some(true),
                structured_content: None,
            };
        }
        return CallToolResult {
            content: vec![ContentBlock::TextContent(TextContent {
                r#type: "text".to_string(),
                text: format!("API error ({}): {}", status, body),
                annotations: None,
            })],
            is_error: Some(true),
            structured_content: None,
        };
    }

    match serde_json::from_str::<Ra1ArtGeneratorResponse>(&body) {
        Ok(resp) => CallToolResult {
            content: vec![ContentBlock::TextContent(TextContent {
                r#type: "text".to_string(),
                text: format!(
                    "Image generated successfully!\nURL: {}\nPrompt: {}\nSize: {}\nCost: ${}",
                    resp.image_url, resp.prompt_used, resp.size_used, resp.cost
                ),
                annotations: None,
            })],
            is_error: Some(false),
            structured_content: None,
        },
        Err(e) => CallToolResult {
            content: vec![ContentBlock::TextContent(TextContent {
                r#type: "text".to_string(),
                text: format!("Failed to parse API response: {e}\nRaw: {body}"),
                annotations: None,
            })],
            is_error: Some(true),
            structured_content: None,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_ra1_tool_json_schema() {
        let tool = create_tool_for_ra1_art_generator();
        assert_eq!(tool.name, "ra1-art-generator");
        assert!(tool.description.is_some());
        let schema = serde_json::to_value(&tool.input_schema).unwrap();
        let props = schema.get("properties").unwrap();
        assert!(props.get("prompt").is_some());
        assert!(props.get("size").is_some());
        let required = schema.get("required").unwrap().as_array().unwrap();
        assert!(required.iter().any(|v| v.as_str() == Some("prompt")));
    }

    #[test]
    fn is_ra1_available_respects_env() {
        // SAFETY: This is a test and we're only removing a test env var
        unsafe { std::env::remove_var(NETWRCK_API_KEY_ENV) };
        assert!(!is_ra1_available());
    }
}
