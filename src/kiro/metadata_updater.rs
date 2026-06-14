//! 模型元数据更新器 - 从公开数据源自动获取上下文长度等元数据

use std::collections::HashMap;

use serde::Deserialize;

/// LiteLLM 模型元数据结构
#[derive(Debug, Deserialize)]
struct LiteLLMModelData {
    #[serde(default)]
    max_tokens: Option<i32>,
    #[serde(default)]
    max_input_tokens: Option<i32>,
    #[serde(default)]
    max_output_tokens: Option<i32>,
    #[serde(default)]
    supports_vision: Option<bool>,
    #[serde(default)]
    supports_function_calling: Option<bool>,
}

/// 从公开数据源获取的模型元数据
#[derive(Debug, Clone)]
pub struct FetchedMetadata {
    pub context_window: Option<i32>,
    pub max_output_tokens: Option<i32>,
    pub supports_vision: bool,
}

/// 元数据更新器
pub struct MetadataUpdater {
    client: reqwest::Client,
    /// 公开的元数据源URL列表（按优先级排序）
    sources: Vec<String>,
}

impl MetadataUpdater {
    /// 创建新的元数据更新器
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .user_agent("KiroRS/1.0")
                .build()
                .unwrap_or_default(),
            sources: vec![
                // LiteLLM 官方维护的模型数据（GitHub raw）
                "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json".to_string(),
                // 备用：jsdelivr CDN
                "https://cdn.jsdelivr.net/gh/BerriAI/litellm@main/model_prices_and_context_window.json".to_string(),
            ],
        }
    }

    /// 从公开数据源获取模型元数据
    pub async fn fetch_metadata(&self) -> anyhow::Result<HashMap<String, FetchedMetadata>> {
        for (idx, source_url) in self.sources.iter().enumerate() {
            tracing::info!("Fetching model metadata from source {}: {}", idx + 1, source_url);

            match self.fetch_from_url(source_url).await {
                Ok(data) => {
                    tracing::info!("Successfully fetched metadata for {} models", data.len());
                    return Ok(data);
                }
                Err(e) => {
                    tracing::warn!("Failed to fetch from source {}: {}", idx + 1, e);
                    if idx == self.sources.len() - 1 {
                        anyhow::bail!("All metadata sources failed");
                    }
                    // 继续尝试下一个源
                }
            }
        }

        anyhow::bail!("No metadata sources available")
    }

    /// 从指定URL获取元数据
    async fn fetch_from_url(&self, url: &str) -> anyhow::Result<HashMap<String, FetchedMetadata>> {
        let response = self.client.get(url).send().await?;

        if !response.status().is_success() {
            anyhow::bail!("HTTP error: {}", response.status());
        }

        // LiteLLM 的数据格式是 { "model_name": { model_data } }
        let raw_data: HashMap<String, LiteLLMModelData> = response.json().await?;

        let mut metadata_map = HashMap::new();

        for (model_id, data) in raw_data {
            let context_window = data.max_input_tokens.or(data.max_tokens);
            let max_output = data.max_output_tokens.or(data.max_tokens);

            // 只保留有有效元数据的模型
            if context_window.is_some() || max_output.is_some() {
                metadata_map.insert(
                    model_id,
                    FetchedMetadata {
                        context_window,
                        max_output_tokens: max_output,
                        supports_vision: data.supports_vision.unwrap_or(false),
                    },
                );
            }
        }

        Ok(metadata_map)
    }
}

impl Default for MetadataUpdater {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_updater_creation() {
        let updater = MetadataUpdater::new();
        assert!(!updater.sources.is_empty());
        assert!(updater.sources[0].contains("litellm"));
    }

    #[test]
    fn test_fetched_metadata_creation() {
        let metadata = FetchedMetadata {
            context_window: Some(8192),
            max_output_tokens: Some(4096),
            supports_vision: false,
        };

        assert_eq!(metadata.context_window, Some(8192));
        assert_eq!(metadata.max_output_tokens, Some(4096));
        assert!(!metadata.supports_vision);
    }
}
