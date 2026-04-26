use crate::config::{BotConfig, Visibility};
use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

pub(crate) fn test_config() -> BotConfig {
    BotConfig {
        mastodon_base: "https://mastodon.example".to_string(),
        mastodon_access_token: "mastodon-token".to_string(),
        openai_model: "gpt-test".to_string(),
        openai_reply_model: "gpt-test-reply".to_string(),
        openai_api_key: "openai-token".to_string(),
        streaming_base_url: "wss://mastodon.example/api/v1/streaming".to_string(),
        prompts_path: "config/prompts.json".to_string(),
        bot_db_path: ":memory:".to_string(),
        free_toot_interval: Duration::from_secs(3600),
        reply_temperature: 0.7,
        free_toot_temperature: 0.8,
        visibility: Visibility::Unlisted,
        mastodon_char_limit: 500,
        reply_min_interval: Duration::from_millis(0),
        enable_web_search: false,
    }
}

pub(crate) struct MockHttpServer {
    base_url: String,
}

impl MockHttpServer {
    pub(crate) fn respond(status: &str, body: &str) -> Self {
        Self::start(MockResponse::Immediate { status: status.to_string(), body: body.to_string() })
    }

    pub(crate) fn respond_after(delay: Duration, status: &str, body: &str) -> Self {
        Self::start(MockResponse::Delayed {
            delay,
            status: status.to_string(),
            body: body.to_string(),
        })
    }

    pub(crate) fn base_url(&self) -> &str {
        &self.base_url
    }

    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url, path)
    }

    fn start(response: MockResponse) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base_url = format!("http://{}", listener.local_addr().unwrap());

        thread::spawn(move || {
            let Ok((mut stream, _addr)) = listener.accept() else {
                return;
            };

            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let mut buf = [0_u8; 1024];
            let _ = stream.read(&mut buf);

            match response {
                MockResponse::Immediate { status, body } => {
                    write_response(&mut stream, &status, &body);
                }
                MockResponse::Delayed { delay, status, body } => {
                    thread::sleep(delay);
                    write_response(&mut stream, &status, &body);
                }
            }
        });

        Self { base_url }
    }
}

enum MockResponse {
    Immediate { status: String, body: String },
    Delayed { delay: Duration, status: String, body: String },
}

fn write_response(stream: &mut std::net::TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = stream.write_all(response.as_bytes());
}

pub(crate) fn closed_local_url(path: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    format!("http://{}{}", addr, path)
}

pub(crate) fn closed_local_ws_url(path: &str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    drop(listener);
    format!("ws://{}{}", addr, path)
}
