use anyhow::Result;
use anyhow::anyhow;
use core_test_support::skip_if_no_network;
use reqwest::Client;
use serde_json::Value;
use serde_json::json;

fn invoice_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "vendor": { "type": "string" },
            "vendor_address": {
                "type": "object",
                "properties": {
                    "street": { "type": "string" },
                    "city": { "type": "string" },
                    "postal_code": { "type": "string" },
                    "country": { "type": "string" }
                },
                "required": ["street", "city", "postal_code", "country"],
                "additionalProperties": false
            },
            "invoice_number": { "type": "string" },
            "invoice_date": { "type": "string", "description": "ISO-8601 date" },
            "line_items": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "description": { "type": "string" },
                        "quantity": { "type": "number" },
                        "unit_price": { "type": "number" }
                    },
                    "required": ["description", "quantity", "unit_price"],
                    "additionalProperties": false
                },
                "minItems": 1
            },
            "total_amount": { "type": "number" },
            "currency": { "type": "string", "minLength": 3, "maxLength": 3 }
        },
        "required": [
            "vendor",
            "vendor_address",
            "invoice_number",
            "invoice_date",
            "line_items",
            "total_amount",
            "currency"
        ],
        "additionalProperties": false
    })
}

fn extract_json_text(response: &Value) -> Option<&str> {
    response["output"]
        .as_array()?
        .first()?
        .get("content")?
        .as_array()?
        .iter()
        .find_map(|item| item.get("text").and_then(Value::as_str))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn grok_structured_output_invoice() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let api_key = match std::env::var("XAI_API_KEY") {
        Ok(key) if !key.trim().is_empty() => key,
        _ => {
            eprintln!("Skipping grok test because XAI_API_KEY is not set.");
            return Ok(());
        }
    };

    let invoice_text = r#"
Vendor: Example Corp
Address: 123 Main St, Springfield, IL 62704, USA
Invoice number: INV-2025-001
Invoice date: February 10, 2025
Items:
  - Widget A, quantity 5, unit price $10.00
  - Widget B, quantity 2, unit price $15.00
Total due: $80.00 USD
"#;

    let request_body = json!({
        "model": "grok-4-fast-reasoning",
        "input": [{
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": format!("Extract the invoice details from the following document and conform to the provided JSON schema. Use ISO 8601 dates and uppercase 3-letter currency codes.\n{invoice_text}")
            }]
        }],
        "response_format": {
            "type": "json_schema",
            "json_schema": {
                "name": "invoice_payload",
                "schema": invoice_schema(),
                "strict": true
            }
        }
    });

    let client = Client::new();
    let response_body: Value = client
        .post("https://api.x.ai/v1/responses")
        .bearer_auth(api_key)
        .json(&request_body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let json_text = extract_json_text(&response_body)
        .ok_or_else(|| anyhow!("missing structured output text: {response_body:?}"))?;

    let invoice_value: Value = serde_json::from_str(json_text.trim())
        .map_err(|err| anyhow!("failed to parse invoice JSON: {err}\n{json_text}"))?;

    let vendor = invoice_value
        .get("vendor")
        .and_then(|value| {
            value.as_str().map(str::to_string).or_else(|| {
                value
                    .get("name")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        })
        .unwrap_or_default();
    assert!(
        vendor.contains("Example"),
        "unexpected vendor field: {invoice_value}"
    );

    let amount_from =
        |key: &str| -> Option<f64> { invoice_value.get(key)?.get("amount")?.as_f64() };

    let total_amount = invoice_value
        .get("total_amount")
        .and_then(Value::as_f64)
        .or_else(|| amount_from("totalDue"))
        .or_else(|| amount_from("total_due"))
        .unwrap_or_default();
    assert!(
        (total_amount - 80.0).abs() < f64::EPSILON,
        "unexpected total: {invoice_value}"
    );

    let currency_from =
        |key: &str| -> Option<&str> { invoice_value.get(key)?.get("currency")?.as_str() };

    let currency = invoice_value
        .get("currency")
        .and_then(Value::as_str)
        .or_else(|| currency_from("totalDue"))
        .or_else(|| currency_from("total_due"))
        .unwrap_or_default()
        .to_string();
    assert_eq!(currency, "USD", "unexpected currency: {invoice_value}");

    let items_len = invoice_value
        .get("line_items")
        .and_then(Value::as_array)
        .map(std::vec::Vec::len)
        .or_else(|| {
            invoice_value
                .get("items")
                .and_then(Value::as_array)
                .map(std::vec::Vec::len)
        })
        .unwrap_or_default();
    assert!(
        items_len >= 2,
        "expected at least two line items: {invoice_value}"
    );

    Ok(())
}
