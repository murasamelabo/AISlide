use aislide_core::execute_request;
use serde_json::json;

#[test]
fn shared_protocol_compiles_exports_and_inspects() {
    let report = execute_request(json!({"op":"sample"})).unwrap();
    let compiled = execute_request(json!({"op":"compile","report":report})).unwrap();
    let exported = execute_request(json!({"op":"export","deck":compiled["deck"]})).unwrap();
    let inspected = execute_request(json!({"op":"inspect","base64":exported["base64"]})).unwrap();
    assert_eq!(inspected["slides"].as_array().unwrap().len(), 12);
    let untouched = execute_request(json!({"op":"roundtrip","base64":exported["base64"]})).unwrap();
    assert_eq!(exported["base64"], untouched["base64"]);
}

#[test]
fn protocol_rejects_unknown_operations_fields_and_malformed_base64() {
    for value in [json!({"op":"shell","command":"anything"}), json!({"op":"sample","path":"secret"}), json!({"op":"inspect","base64":"!invalid!"})] {
        assert!(execute_request(value).is_err());
    }
}

#[test]
fn protocol_rejects_extra_scene_fields() {
    let report = execute_request(json!({"op":"sample"})).unwrap();
    let mut compiled = execute_request(json!({"op":"compile","report":report})).unwrap();
    compiled["deck"]["slides"][0]["elements"][0]["script"] = json!("bad");
    assert!(execute_request(json!({"op":"export","deck":compiled["deck"]})).is_err());
}

#[test]
fn provider_status_exposes_configuration_without_credentials() {
    let status = execute_request(json!({"op":"provider_status"})).unwrap();
    assert!(status["configured"].is_boolean());
    assert!(status["remote"].is_boolean());
    assert!(status.get("api_key").is_none());
    assert!(status.get("authorization").is_none());
    assert!(execute_request(json!({"op":"provider_status","endpoint":"http://example.com"})).is_err());
}

#[test]
fn generation_requests_do_not_accept_provider_settings() {
    let invalid = json!({"op":"generate","input":{"prompt":"Test","source_text":"","slide_count":3,"api_key":"not-a-secret","base_url":"http://example.com"}});
    assert!(execute_request(invalid).is_err());
}