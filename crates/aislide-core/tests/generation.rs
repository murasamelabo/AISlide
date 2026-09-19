use aislide_core::{generation::{GenerationInput, ProviderConfig, generate_report}, report::sample_report};
use serde_json::{Value, json};
use std::{io::{Read, Write}, net::TcpListener, sync::mpsc, thread};

fn input() -> GenerationInput {
    GenerationInput { prompt: "Create a report from the supplied notes".into(), source_text: "Example notes only, not verified facts.".into(), slide_count: 12, allow_remote: false, max_repairs: 0, outline: Vec::new() }
}

fn serve_once(status: &str, body: String, extra_headers: &str) -> (String, mpsc::Receiver<Value>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/v1", listener.local_addr().unwrap());
    let (sender, receiver) = mpsc::channel();
    let response = format!("HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n{extra_headers}\r\n{body}", body.len());
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(std::time::Duration::from_secs(5))).unwrap();
        let mut request = Vec::new();
        let mut buffer = [0; 4096];
        let header_end = loop {
            let count = stream.read(&mut buffer).unwrap();
            assert!(count > 0);
            request.extend_from_slice(&buffer[..count]);
            if let Some(index) = request.windows(4).position(|value| value == b"\r\n\r\n") { break index + 4; }
            assert!(request.len() < 64 * 1024);
        };
        let headers = String::from_utf8_lossy(&request[..header_end]);
        assert!(headers.starts_with("POST /v1/chat/completions HTTP/1.1"));
        let size: usize = headers.lines().find_map(|line| line.split_once(':').filter(|(name, _)| name.eq_ignore_ascii_case("content-length")).map(|(_, value)| value.trim().parse().unwrap())).unwrap();
        while request.len() - header_end < size {
            let count = stream.read(&mut buffer).unwrap();
            assert!(count > 0);
            request.extend_from_slice(&buffer[..count]);
        }
        sender.send(serde_json::from_slice(&request[header_end..header_end + size]).unwrap()).unwrap();
        let _ = stream.write_all(response.as_bytes());
    });
    (address, receiver, handle)
}

#[test]
fn provider_policy_requires_explicit_remote_opt_in() {
    for address in ["http://127.0.0.1:1234/v1", "http://[::1]:1234/v1", "http://localhost:1234/v1"] {
        assert!(ProviderConfig::new(address, "test-model", None, false).is_ok(), "{address}");
    }
    for address in ["http://example.com/v1", "http://169.254.169.254/v1", "file:///models", "https://user:secret@example.com/v1", "https://example.com/v1?key=secret", "https://example.com/v1#fragment"] {
        assert!(ProviderConfig::new(address, "test-model", None, true).is_err(), "{address}");
    }
    assert!(ProviderConfig::new("https://example.com/v1", "test-model", None, false).is_err());
    assert!(ProviderConfig::new("https://example.com/v1", "test-model", None, true).is_ok());
}

#[test]
fn generated_report_is_validated_and_compiled_by_the_same_core() {
    let content = serde_json::to_string(&sample_report()).unwrap();
    let (address, requests, server) = serve_once("200 OK", json!({"model":"fixture-model","choices":[{"finish_reason":"stop","message":{"role":"assistant","content":content}}]}).to_string(), "");
    let config = ProviderConfig::new(&address, "test-model", None, false).unwrap();
    let result = generate_report(&config, &input()).unwrap();
    server.join().unwrap();
    let request = requests.recv().unwrap();
    assert_eq!(request["model"], "test-model");
    assert_eq!(request["messages"][0]["role"], "system");
    assert_eq!(request["messages"][1]["role"], "user");
    assert_eq!(request["stream"], false);
    assert_eq!(result.compiled.deck.slides.len(), 12);
    assert_eq!(result.provenance.mode, "model");
    assert!(!result.provenance.remote);
    assert!(result.compiled.issues.iter().any(|issue| issue.code == "AI_CONTENT_UNVERIFIED"));
}

#[test]
fn malformed_truncated_and_refused_model_outputs_never_become_a_deck() {
    for response in [
        json!({"choices":[{"finish_reason":"stop","message":{"content":"not JSON"}}]}),
        json!({"choices":[{"finish_reason":"length","message":{"content":serde_json::to_string(&sample_report()).unwrap()}}]}),
        json!({"choices":[{"finish_reason":"stop","message":{"refusal":"Cannot comply", "content":"{}"}}]}),
        json!({"choices":[{"finish_reason":"stop","message":{"content":"{\"title\":\"incomplete\"}"}}]}),
    ] {
        let (address, _requests, server) = serve_once("200 OK", response.to_string(), "");
        let config = ProviderConfig::new(&address, "test-model", None, false).unwrap();
        assert!(generate_report(&config, &input()).is_err());
        server.join().unwrap();
    }
}

#[test]
fn response_errors_do_not_echo_provider_bodies_or_credentials() {
    let (address, _requests, server) = serve_once("401 Unauthorized", "sensitive-response-body".into(), "");
    let config = ProviderConfig::new(&address, "test-model", Some("test-not-a-real-key".into()), false).unwrap();
    let message = generate_report(&config, &input()).err().unwrap().to_string();
    server.join().unwrap();
    assert!(message.contains("401"));
    assert!(!message.contains("sensitive-response-body"));
    assert!(!message.contains("test-not-a-real-key"));
}

#[test]
fn redirects_are_not_followed() {
    let (address, _requests, server) = serve_once("307 Temporary Redirect", "{}".into(), "Location: http://127.0.0.1:1/forbidden\r\n");
    let config = ProviderConfig::new(&address, "test-model", None, false).unwrap();
    assert!(generate_report(&config, &input()).err().unwrap().to_string().contains("307"));
    server.join().unwrap();
}

#[test]
fn invalid_inputs_fail_before_network_access() {
    let config = ProviderConfig::new("http://127.0.0.1:1/v1", "test-model", None, false).unwrap();
    let mut request = input();
    request.slide_count = 0;
    assert!(generate_report(&config, &request).err().unwrap().to_string().contains("slide"));
    request.slide_count = 12;
    request.prompt.clear();
    assert!(generate_report(&config, &request).err().unwrap().to_string().contains("prompt"));
    let remote = ProviderConfig::new("https://example.com/v1", "test-model", None, true).unwrap();
    assert!(generate_report(&remote, &input()).err().unwrap().to_string().contains("consent"));
}

#[test]
fn oversized_and_wrong_slide_count_outputs_are_rejected() {
    for body in [
        " ".repeat(1024 * 1024 + 1),
        json!({"choices":[{"finish_reason":"stop","message":{"content":serde_json::to_string(&sample_report()).unwrap()}}]}).to_string(),
    ] {
        let (address, _requests, server) = serve_once("200 OK", body, "");
        let config = ProviderConfig::new(&address, "test-model", None, false).unwrap();
        let mut request = input();
        request.slide_count = 3;
        assert!(generate_report(&config, &request).is_err());
        server.join().unwrap();
    }
}

#[test]
fn cancellation_interrupts_a_provider_that_never_replies() {
    use aislide_core::generation::{CancellationToken, generate_report_with_cancel};
    use std::time::Duration;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/v1", listener.local_addr().unwrap());
    let (connected, ready) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut buffer = [0; 4096];
        assert!(stream.read(&mut buffer).unwrap() > 0);
        connected.send(()).unwrap();
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(_) => continue,
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => break,
                Err(error) => panic!("cancelled HTTP connection remained open: {error}"),
            }
        }
    });
    let config = ProviderConfig::new(&address, "test-model", None, false).unwrap();
    let cancellation = CancellationToken::new();
    let signal = cancellation.clone();
    let (finished, result) = mpsc::channel();
    let worker = thread::spawn(move || {
        finished.send(generate_report_with_cancel(&config, &input(), signal).err().unwrap().to_string()).unwrap();
    });
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    cancellation.cancel();
    assert!(result.recv_timeout(Duration::from_secs(3)).unwrap().contains("cancelled"));
    worker.join().unwrap();
    server.join().unwrap();
}

#[test]
fn cancelled_request_does_not_start_network_access() {
    use aislide_core::generation::{CancellationToken, generate_report_with_cancel};
    let config = ProviderConfig::new("http://127.0.0.1:1/v1", "test-model", None, false).unwrap();
    let cancellation = CancellationToken::new();
    cancellation.cancel();
    assert!(generate_report_with_cancel(&config, &input(), cancellation).err().unwrap().to_string().contains("cancelled"));
}

#[test]
fn generation_async_api_works_inside_the_tauri_runtime() {
    use aislide_core::generation::{CancellationToken, generate_report_async};
    let content = serde_json::to_string(&sample_report()).unwrap();
    let (address, _requests, server) = serve_once("200 OK", json!({"choices":[{"finish_reason":"stop","message":{"content":content}}]}).to_string(), "");
    let config = ProviderConfig::new(&address, "fixture-model", None, false).unwrap();
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let result = runtime.block_on(generate_report_async(&config, &input(), CancellationToken::new())).unwrap();
    assert_eq!(result.compiled.deck.slides.len(), 12);
    server.join().unwrap();
}

#[test]
fn local_assist_strict_model_response_and_schema_are_not_template_fallbacks() {
    use aislide_core::{text_assist::{assist, Input, Task}, generation::CancellationToken};
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let input = Input { task: Task::Proofread, text: "This are a test.".into(), language: "en".into(), target_language: None };
    for content in [r#"{"text":"This is a test."}"#, r#"{"text":"Text","kind":"html"}"#, r#"{"text":""}"#, r#"{"text":"First\nSecond"}"#, "not JSON", r#"{"text":"first","text":"duplicate"}"#] {
        let (address, requests, server) = serve_once("200 OK", json!({"choices":[{"finish_reason":"stop","message":{"content":content}}]}).to_string(), "");
        let config = ProviderConfig::new(&address,"fixture-not-real-ai",None,false).unwrap();
        let result = runtime.block_on(assist(&config,&input,CancellationToken::new()));
        server.join().unwrap();
        let request = requests.recv().unwrap();
        assert_eq!(request["response_format"]["json_schema"]["strict"],true);
        assert_eq!(request["response_format"]["json_schema"]["schema"]["additionalProperties"],false);
        if content.contains("This is a test.") { assert_eq!(result.unwrap().candidate.text,"This is a test."); } else { assert!(result.is_err(),"{content}"); }
    }
}

#[test]
fn local_assist_rejects_remote_even_with_consent_and_early_cancel() {
    use aislide_core::{text_assist::{assist, Input, Task}, generation::CancellationToken};
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let input = Input { task: Task::Translate, text: "Hello.".into(), language: "en".into(), target_language: Some("ja-JP".into()) };
    let remote = ProviderConfig::new("https://example.invalid/v1","test",None,true).unwrap();
    assert!(runtime.block_on(assist(&remote,&input,CancellationToken::new())).unwrap_err().to_string().contains("forbids remote"));
    let local = ProviderConfig::new("http://127.0.0.1:1/v1","test",None,false).unwrap();
    let token = CancellationToken::new(); token.cancel();
    assert!(runtime.block_on(assist(&local,&input,token)).unwrap_err().to_string().contains("cancelled"));
    let mut oversized = input; oversized.text = "x".repeat(8001);
    assert!(runtime.block_on(assist(&local,&oversized,CancellationToken::new())).is_err());
}

#[test]
fn local_assist_cancellation_interrupts_an_inflight_http_request() {
    use aislide_core::{text_assist::{assist, Input, Task}, generation::CancellationToken};
    use std::time::Duration;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = format!("http://{}/v1", listener.local_addr().unwrap());
    let (connected, ready) = mpsc::channel();
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let mut buffer = [0; 4096];
        assert!(stream.read(&mut buffer).unwrap() > 0);
        connected.send(()).unwrap();
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break,
                Ok(_) => {},
                Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => break,
                Err(error) => panic!("cancelled HTTP remained open: {error}"),
            }
        }
    });
    let token = CancellationToken::new();
    let worker_token = token.clone();
    let worker = thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let config = ProviderConfig::new(&address, "bounded-fixture", None, false).unwrap();
        let input = Input { task: Task::Proofread, text: "This are a test.".into(), language: "en".into(), target_language: None };
        runtime.block_on(assist(&config, &input, worker_token)).unwrap_err().to_string()
    });
    ready.recv_timeout(Duration::from_secs(5)).unwrap();
    token.cancel();
    assert!(worker.join().unwrap().contains("cancelled"));
    server.join().unwrap();
}