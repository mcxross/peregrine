use base64::Engine;
use sha2::Digest;
use sha2::Sha256;
use url::Url;

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub fn normalize_server_url(url: &str) -> Result<Url, url::ParseError> {
    let mut parsed = Url::parse(url)?;
    if parsed.path().ends_with('/') && parsed.path() != "/" {
        let trimmed = parsed.path().trim_end_matches('/').to_owned();
        parsed.set_path(&trimmed);
    }
    Ok(parsed)
}

pub fn sanitize_server_error(status: u16, raw_body: &str) -> (String, Option<String>) {
    if status == 401 {
        return (
            "401 from relayer: wrong delegate key, key not registered on this account, account mismatch, or network mismatch".to_owned(),
            Some("AUTH_REJECTED".to_owned()),
        );
    }

    let mut server_code = None;
    let mut text = raw_body.to_owned();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(raw_body) {
        if let Some(code) = value.get("code").and_then(serde_json::Value::as_str) {
            server_code = Some(code.to_owned());
        } else if let Some(code) = value.get("error").and_then(serde_json::Value::as_str) {
            server_code = Some(code.to_owned());
        }

        if let Some(message) = value.get("message").and_then(serde_json::Value::as_str) {
            text = message.to_owned();
        }
    }

    let sanitized = text
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>()
        .trim()
        .chars()
        .take(200)
        .collect::<String>();

    let message = if sanitized.is_empty() {
        format!("MemWal server error ({status})")
    } else {
        format!("MemWal server error ({status}): {sanitized}")
    };
    (message, server_code)
}

pub fn encode_base64_json<T: serde::Serialize>(value: &T) -> Result<String, serde_json::Error> {
    let json = serde_json::to_vec(value)?;
    Ok(base64::engine::general_purpose::STANDARD.encode(json))
}
