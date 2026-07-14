//! Kiro Models API 调用

use anyhow::{Context, Result};

use crate::http_client::{build_client, ProxyConfig};
use crate::kiro::model::credentials::KiroCredentials;
use crate::kiro::model::ModelInfo;
use crate::model::config::{Config, TlsBackend};

const KIRO_API_BASE: &str = "https://codewhisperer.us-east-1.amazonaws.com";

/// 调用 ListAvailableModels API 获取账号可用的模型列表
pub async fn list_available_models(
    credential: &KiroCredentials,
    config: &Config,
    proxy_config: Option<&ProxyConfig>,
    tls_backend: TlsBackend,
) -> Result<Vec<ModelInfo>> {
    // API Key 账号直接使用 kiro_api_key 作为 Bearer Token
    let bearer_token = if credential.is_api_key_credential() {
        credential
            .kiro_api_key
            .as_ref()
            .context("API Key is missing for api_key credential")?
    } else {
        credential
            .access_token
            .as_ref()
            .context("Access token is missing")?
    };

    // API Key 账号跳过 ProfileArn（参考 Kiro-Go ResolveProfileArn 逻辑）
    let profile_arn = if credential.is_api_key_credential() {
        String::new()
    } else {
        credential
            .profile_arn
            .as_ref()
            .map(|s| format!("&profileArn={}", urlencoding::encode(s)))
            .unwrap_or_default()
    };

    // 根据账号配置动态确定区域（参考 Kiro-Go kiroRegionForProfile）
    // API Key 账号使用 EffectiveApiRegion（ApiRegion > Region > 全局配置 > us-east-1）
    // OAuth 账号优先从 ProfileArn 提取区域，回退到 Region
    let region = if credential.is_api_key_credential() {
        credential.effective_api_region(config)
    } else {
        // OAuth 账号：尝试从 ProfileArn 提取区域
        credential
            .profile_arn
            .as_ref()
            .and_then(|arn| {
                // ProfileArn 格式: arn:aws:codewhisperer:{region}:...
                arn.split(':').nth(3).filter(|r| !r.is_empty())
            })
            .unwrap_or_else(|| {
                // 回退到 Region 字段
                credential
                    .region
                    .as_deref()
                    .unwrap_or(config.effective_api_region())
            })
    };

    // 构建区域化的 API Base URL
    let api_base = if region == "us-east-1" {
        KIRO_API_BASE.to_string()
    } else {
        // 非 us-east-1 区域：替换主机名为 q.{region}.amazonaws.com
        format!("https://q.{}.amazonaws.com", region)
    };

    let url = format!(
        "{}/ListAvailableModels?origin=AI_EDITOR&maxResults=50{}",
        api_base, profile_arn
    );

    // 调试日志：记录请求详情
    if credential.is_api_key_credential() {
        tracing::debug!(
            "API Key request: url={}, region={}, token_prefix={}, has_tokentype=true",
            url,
            region,
            bearer_token.chars().take(10).collect::<String>()
        );
    }

    let client = build_client(proxy_config, 30, tls_backend)
        .context("Failed to build HTTP client")?;

    // 构建 User-Agent headers（匹配 Kiro-Go buildRuntimeHeaderValues）
    let machine_id = credential
        .machine_id
        .as_deref()
        .or(config.machine_id.as_deref())
        .unwrap_or("");
    let user_agent = format!(
        "aws-sdk-js/1.0.0 ua/2.1 os/{} lang/js md/nodejs#{} api/codewhispererruntime#1.0.0 m/N,E KiroIDE-{}-{}",
        config.system_version,
        config.node_version,
        config.kiro_version,
        machine_id
    );
    let x_amz_user_agent = format!(
        "aws-sdk-js/1.0.0 KiroIDE-{}-{}",
        config.kiro_version,
        machine_id
    );

    let mut request = client
        .get(&url)
        .header("Accept", "application/json")
        .header("User-Agent", user_agent)
        .header("x-amz-user-agent", x_amz_user_agent)
        .header("x-amzn-codewhisperer-optout", "true")
        .header("Authorization", format!("Bearer {}", bearer_token));

    // API Key 账号需要额外的 tokentype 头（匹配 Kiro-Go 逻辑）
    if credential.is_api_key_credential() {
        request = request.header("tokentype", "API_KEY");
    }

    let response = request
        .send()
        .await
        .context("Failed to send request to ListAvailableModels")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        // API Key 错误时额外记录区域信息
        if credential.is_api_key_credential() {
            tracing::warn!(
                "API Key authentication failed: region={}, status={}, body={}",
                region,
                status,
                body
            );
        }

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
