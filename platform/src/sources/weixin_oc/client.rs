use std::collections::HashMap;

use reqwest::Client as HttpClient;
use serde_json::Value;
use tracing::debug;

use super::crypto;

pub struct WeixinOCClient {
    pub base_url: String,
    pub cdn_base_url: String,
    pub api_timeout_ms: u64,
    pub token: Option<String>,
    http: HttpClient,
}

impl WeixinOCClient {
    pub fn new(
        base_url: impl Into<String>,
        cdn_base_url: impl Into<String>,
        api_timeout_ms: u64,
        token: Option<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            cdn_base_url: cdn_base_url.into(),
            api_timeout_ms,
            token,
            http: HttpClient::new(),
        }
    }

    pub fn with_http(
        http: HttpClient,
        base_url: impl Into<String>,
        cdn_base_url: impl Into<String>,
        api_timeout_ms: u64,
        token: Option<String>,
    ) -> Self {
        Self {
            base_url: base_url.into(),
            cdn_base_url: cdn_base_url.into(),
            api_timeout_ms,
            token,
            http,
        }
    }

    fn resolve_url(&self, endpoint: &str) -> String {
        format!(
            "{}/{}",
            self.base_url.trim_end_matches('/'),
            endpoint.trim_start_matches('/')
        )
    }

    fn build_headers(&self, token_required: bool) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        headers.insert(
            "AuthorizationType",
            "ilink_bot_token".parse().unwrap(),
        );
        if token_required {
            if let Some(ref token) = self.token {
                headers.insert(
                    reqwest::header::AUTHORIZATION,
                    format!("Bearer {token}").parse().unwrap(),
                );
            }
        }
        headers
    }

    pub async fn request_json(
        &self,
        method: &str,
        endpoint: &str,
        params: Option<&HashMap<String, String>>,
        payload: Option<Value>,
        token_required: bool,
        timeout_ms: Option<u64>,
    ) -> Result<Value, String> {
        let url = self.resolve_url(endpoint);
        let timeout = std::time::Duration::from_millis(timeout_ms.unwrap_or(self.api_timeout_ms));
        let headers = self.build_headers(token_required);

        let mut req = match method {
            "GET" => self.http.get(&url).headers(headers.clone()),
            "POST" => self.http.post(&url).headers(headers.clone()),
            m => return Err(format!("unsupported method: {m}")),
        };

        if let Some(p) = params {
            req = req.query(&p);
        }
        if let Some(p) = payload {
            req = req.json(&p);
        }

        let resp = req
            .timeout(timeout)
            .send()
            .await
            .map_err(|e| format!("request failed: {e}"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("HTTP {status}: {body}"));
        }

        resp.json::<Value>()
            .await
            .map_err(|e| format!("parse response failed: {e}"))
    }

    pub async fn upload_to_cdn(
        &self,
        upload_full_url: Option<&str>,
        upload_param: &str,
        file_key: &str,
        aes_key_hex: &str,
        data: &[u8],
    ) -> Result<String, String> {
        let cdn_url = upload_full_url
            .map(|s| s.to_string())
            .unwrap_or_else(|| self.build_cdn_upload_url(upload_param, file_key));

        let aes_key = hex::decode(aes_key_hex).map_err(|e| format!("hex decode: {e}"))?;
        let encrypted = crypto::aes_ecb_encrypt(&aes_key, data);

        debug!(
            "CDN upload: url={cdn_url} plain_size={} cipher_size={}",
            data.len(),
            encrypted.len()
        );

        let resp = self
            .http
            .post(&cdn_url)
            .header("Content-Type", "application/octet-stream")
            .body(encrypted)
            .timeout(std::time::Duration::from_millis(self.api_timeout_ms))
            .send()
            .await
            .map_err(|e| format!("CDN upload failed: {e}"))?;

        let status = resp.status();
        if status.is_client_error() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("CDN upload client error {status}: {body}"));
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("CDN upload failed {status}: {body}"));
        }

        resp.headers()
            .get("x-encrypted-param")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or_else(|| "missing x-encrypted-param header".to_string())
    }

    pub async fn download_cdn_bytes(&self, encrypted_query_param: &str) -> Result<Vec<u8>, String> {
        let url = self.build_cdn_download_url(encrypted_query_param);
        let resp = self
            .http
            .get(&url)
            .timeout(std::time::Duration::from_millis(self.api_timeout_ms))
            .send()
            .await
            .map_err(|e| format!("CDN download failed: {e}"))?;

        let status = resp.status();
        if status.is_client_error() || status.is_server_error() {
            return Err(format!("CDN download {status}"));
        }

        resp.bytes()
            .await
            .map(|b| b.to_vec())
            .map_err(|e| format!("CDN download body: {e}"))
    }

    pub async fn download_and_decrypt_media(
        &self,
        encrypted_query_param: &str,
        aes_key_value: &str,
    ) -> Result<Vec<u8>, String> {
        let encrypted = self.download_cdn_bytes(encrypted_query_param).await?;
        let key = crypto::parse_media_aes_key(aes_key_value)
            .map_err(|e| format!("parse aes key: {e}"))?;
        Ok(crypto::aes_ecb_decrypt(&key, &encrypted))
    }

    fn build_cdn_upload_url(&self, upload_param: &str, file_key: &str) -> String {
        format!(
            "{}/upload?encrypted_query_param={upload_param}&filekey={file_key}",
            self.cdn_base_url.trim_end_matches('/')
        )
    }

    fn build_cdn_download_url(&self, encrypted_query_param: &str) -> String {
        format!(
            "{}/download?encrypted_query_param={encrypted_query_param}",
            self.cdn_base_url.trim_end_matches('/')
        )
    }
}
