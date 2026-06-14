//! Kiro Models API 调用

use anyhow::{Context, Result};

use crate::http_client::{build_client, ProxyConfig};
use crate::kiro::model::credentials::KiroCredentials;
use crate::kiro::model::ModelInfo;
use crate::model::config::TlsBackend;

const KIRO_API_BASE: &str = "https://codewhisperer.us-east-1.amazonaws.com";

/// 调用 ListAvailableModels API 获取账号可用的模型列表
pub async fn list_available_models(
    credential: &KiroCredentials,
    proxy_config: Option<&ProxyConfig>,
    tls_backend: TlsBackend,
) -> Result<Vec<ModelInfo>> {
    let access_token = credential
        .access_token
        .as_ref()
        .context("Access token is missing")?;

    let profile_arn = credential
        .profile_arn
        .as_ref()
        .map(|s| format!("&profileArn={}", urlencoding::encode(s)))
        .unwrap_or_default();

    let url = format!(
        "{}/ListAvailableModels?origin=AI_EDITOR&maxResults=50{}",
        KIRO_API_BASE, profile_arn
    );

    let client = build_client(proxy_config, 30, tls_backend)
        .context("Failed to build HTTP client")?;

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await
        .context("Failed to send request to ListAvailableModels")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!(
            "ListAvailableModels failed with status {}: {}",
            status,
            body
        );
    }

    let body = response
        .json::<serde_json::Value>()
        .await
        .context("Failed to parse response")?;

    let models = body["models"]
        .as_array()
        .context("Response missing 'models' array")?
        .iter()
        .filter_map(|v| serde_json::from_value::<ModelInfo>(v.clone()).ok())
        .collect();

    Ok(models)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_construction() {
        let url = format!(
            "{}/ListAvailableModels?origin=AI_EDITOR&maxResults=50",
            KIRO_API_BASE
        );
        assert!(url.contains("ListAvailableModels"));
        assert!(url.contains("maxResults=50"));
    }

    #[test]
    fn test_profile_arn_encoding() {
        let profile_arn = "arn:aws:iam::123456789012:user/test";
        let encoded = urlencoding::encode(profile_arn);
        assert!(encoded.contains("%3A"));
        assert!(encoded.contains("%2F"));
    }
}
