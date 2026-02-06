use anyhow::Result;
use codex_core::ModelProviderInfo;
use codex_core::built_in_model_providers;
use core_test_support::responses;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn stubbed_anthropic_provider(base_url: String) -> ModelProviderInfo {
    let mut provider = built_in_model_providers()["anthropic"].clone();
    provider.base_url = Some(base_url);
    // Avoid depending on real environment variables in tests.
    provider.env_key = None;
    provider.experimental_bearer_token = Some("sk-ant-test".to_string());
    // Avoid slow exponential backoff in rate-limit scenarios.
    provider.request_max_retries = Some(0);
    provider
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn openai_rate_limit_falls_back_to_opus_4_6() -> Result<()> {
    let openai_server = MockServer::start().await;
    let anthropic_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/responses"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("content-type", "application/json")
                .set_body_string(r#"{"error":{"type":"rate_limit_exceeded"}}"#),
        )
        .expect(1)
        .mount(&openai_server)
        .await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_raw(
                    "data: {\"choices\":[{\"delta\":{\"content\":\"hello\"}}]}\n\ndata: [DONE]\n\n",
                    "text/event-stream",
                ),
        )
        .expect(1)
        .mount(&anthropic_server)
        .await;

    let anthropic_base_url = format!("{}/v1", anthropic_server.uri());
    let mut builder = test_codex().with_config(move |config| {
        config.model_provider.request_max_retries = Some(0);
        config.model_providers.insert(
            "anthropic".to_string(),
            stubbed_anthropic_provider(anthropic_base_url.clone()),
        );
    });
    let test = builder.build(&openai_server).await?;
    test.submit_turn("hello").await?;

    let requests = anthropic_server
        .received_requests()
        .await
        .expect("requests captured");
    let req = requests
        .iter()
        .find(|req| req.method.as_str() == "POST" && req.url.path() == "/v1/chat/completions")
        .expect("expected POST /v1/chat/completions");
    let body = req.body_json::<serde_json::Value>()?;
    assert_eq!(body["model"], "claude-opus-4-6");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn anthropic_rate_limit_falls_back_to_gpt_53_codex_xhigh() -> Result<()> {
    let openai_server = responses::start_mock_server().await;
    let anthropic_server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/v1/chat/completions"))
        .respond_with(
            ResponseTemplate::new(429)
                .insert_header("content-type", "application/json")
                .set_body_string(r#"{"error":{"type":"rate_limit_exceeded"}}"#),
        )
        .expect(1)
        .mount(&anthropic_server)
        .await;

    let openai_mock = responses::mount_sse_once(
        &openai_server,
        responses::sse(vec![
            responses::ev_response_created("resp-1"),
            responses::ev_assistant_message("msg-1", "ok"),
            responses::ev_completed("resp-1"),
        ]),
    )
    .await;

    let openai_base_url = format!("{}/v1", openai_server.uri());
    let anthropic_base_url = format!("{}/v1", anthropic_server.uri());
    let mut builder = test_codex()
        .with_model("claude-opus-4-6")
        .with_config(move |config| {
            config.model_provider_id = "anthropic".to_string();
            config.model_provider = stubbed_anthropic_provider(anthropic_base_url.clone());

            let mut openai_provider = built_in_model_providers()["openai"].clone();
            openai_provider.base_url = Some(openai_base_url.clone());
            // Avoid depending on real environment variables in tests.
            openai_provider.env_key = None;
            openai_provider.experimental_bearer_token = Some("sk-openai-test".to_string());
            // Avoid slow exponential backoff in rate-limit scenarios.
            openai_provider.request_max_retries = Some(0);
            config
                .model_providers
                .insert("openai".to_string(), openai_provider);
        });

    let test = builder.build(&openai_server).await?;
    test.submit_turn("hello").await?;

    let request = openai_mock.single_request().body_json();
    assert_eq!(request["model"], "gpt-5.3-codex");
    assert_eq!(request["reasoning"]["effort"], "xhigh");

    Ok(())
}
