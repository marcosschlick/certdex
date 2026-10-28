use serde::{Deserialize, Serialize};
use std::time::Duration;

const MODEL: &str = "qwen3:1.7b";

#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum OllamaStatus {
    ModelAvailable,
    ModelMissing,
    Unavailable,
}

#[derive(Deserialize)]
struct ModelList {
    models: Vec<Model>,
}

#[derive(Deserialize)]
struct Model {
    name: String,
}

async fn check_status(url: &str) -> Result<OllamaStatus, String> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(3))
        .build()
        .map_err(|error| format!("Could not create Ollama client: {error}"))?;
    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(error) if error.is_connect() || error.is_timeout() => {
            return Ok(OllamaStatus::Unavailable);
        }
        Err(error) => return Err(format!("Could not contact Ollama: {error}")),
    };
    let response = response
        .error_for_status()
        .map_err(|error| format!("Ollama model list request failed: {error}"))?;
    let models: ModelList = response
        .json()
        .await
        .map_err(|error| format!("Invalid Ollama model list response: {error}"))?;
    Ok(if models.models.iter().any(|model| model.name == MODEL) {
        OllamaStatus::ModelAvailable
    } else {
        OllamaStatus::ModelMissing
    })
}

#[tauri::command]
pub(crate) async fn check_ollama() -> Result<OllamaStatus, String> {
    check_status("http://localhost:11434/api/tags").await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    fn respond(body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0; 1024];
            stream.read(&mut request).unwrap();
            write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body).unwrap();
        });
        format!("http://{address}/api/tags")
    }

    #[test]
    fn distinguishes_ollama_and_model_status() {
        tauri::async_runtime::block_on(async {
            let available = respond(r#"{"models":[{"name":"qwen3:1.7b"}]}"#);
            assert_eq!(
                check_status(&available).await.unwrap(),
                OllamaStatus::ModelAvailable
            );
            let missing = respond(r#"{"models":[{"name":"qwen3:4b"}]}"#);
            assert_eq!(
                check_status(&missing).await.unwrap(),
                OllamaStatus::ModelMissing
            );
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            drop(listener);
            assert_eq!(
                check_status(&format!("http://{address}/api/tags"))
                    .await
                    .unwrap(),
                OllamaStatus::Unavailable
            );
            let invalid = respond(r#"{}"#);
            assert!(check_status(&invalid).await.is_err());
        });
    }
}
