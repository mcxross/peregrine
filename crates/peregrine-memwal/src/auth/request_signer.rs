use reqwest::Method;
use reqwest::header::HeaderMap;
use reqwest::header::HeaderName;
use reqwest::header::HeaderValue;
use uuid::Uuid;

use crate::auth::DelegateKey;
use crate::error::MemWalError;
use crate::utils::sha256_hex;

#[derive(Debug, Clone)]
pub(crate) struct RequestSigner {
    delegate_key: DelegateKey,
    account_id: sui_sdk_types::Address,
}

impl RequestSigner {
    pub(crate) fn new(delegate_key: DelegateKey, account_id: sui_sdk_types::Address) -> Self {
        Self {
            delegate_key,
            account_id,
        }
    }

    pub(crate) fn delegate_key(&self) -> &DelegateKey {
        &self.delegate_key
    }

    pub(crate) fn signed_headers(
        &self,
        method: &Method,
        path_and_query: &str,
        body: &[u8],
    ) -> Result<HeaderMap, MemWalError> {
        let timestamp = (chrono::Utc::now().timestamp()) as u64;
        let nonce = Uuid::new_v4();
        let account_id = self.account_id.to_string();
        let body_hash = sha256_hex(body);
        let message = format!(
            "{}.{}.{}.{}.{}.{}",
            timestamp,
            method.as_str(),
            path_and_query,
            body_hash,
            nonce,
            account_id
        );
        let signature = self.delegate_key.sign_raw(message.as_bytes())?;

        let mut headers = HeaderMap::new();
        insert_header(
            &mut headers,
            "x-public-key",
            &self.delegate_key.public_key_hex(),
        )?;
        insert_header(
            &mut headers,
            "x-signature",
            &hex::encode(signature.as_bytes()),
        )?;
        insert_header(&mut headers, "x-timestamp", &timestamp.to_string())?;
        insert_header(&mut headers, "x-nonce", &nonce.to_string())?;
        insert_header(&mut headers, "x-account-id", &account_id)?;
        Ok(headers)
    }
}

fn insert_header(
    headers: &mut HeaderMap,
    name: &'static str,
    value: &str,
) -> Result<(), MemWalError> {
    headers.insert(
        HeaderName::from_static(name),
        HeaderValue::from_str(value).map_err(|error| MemWalError::config(error.to_string()))?,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use reqwest::Method;

    use super::RequestSigner;
    use crate::auth::DelegateKey;

    #[test]
    fn get_request_signs_empty_body() {
        let delegate_key = DelegateKey::from_hex(
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f",
        )
        .expect("delegate key");
        let signer = RequestSigner::new(delegate_key, "0x2".parse().expect("object id"));

        let headers = signer
            .signed_headers(&Method::GET, "/api/remember/abc", &[])
            .expect("headers");
        assert!(headers.contains_key("x-signature"));
        assert!(headers.contains_key("x-public-key"));
    }
}
