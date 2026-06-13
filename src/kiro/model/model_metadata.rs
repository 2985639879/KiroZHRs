//! 静态模型元数据

/// 静态模型元数据(用于补全 API 返回的数据)
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ModelMetadata {
    pub model_id: String,
    pub display_name: String,
    pub context_window: i32,
    pub max_output_tokens: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pricing: Option<PricingInfo>,
    pub model_type: String,
    pub created: i64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PricingInfo {
    pub input_per_1m: f64,
    pub output_per_1m: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_per_1m: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_per_1m: Option<f64>,
}

/// 静态元数据集合(从 JSON 加载)
#[derive(Debug, Clone, serde::Deserialize)]
pub struct StaticMetadataCollection {
    pub models: Vec<ModelMetadata>,
}

impl StaticMetadataCollection {
    /// 从嵌入的 JSON 文件加载静态元数据
    pub fn load_embedded() -> anyhow::Result<Self> {
        const METADATA_JSON: &str = include_str!("static_metadata.json");
        serde_json::from_str(METADATA_JSON).map_err(|e| {
            anyhow::anyhow!("Failed to parse static metadata: {}", e)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_metadata_serialization() {
        let metadata = ModelMetadata {
            model_id: "claude-opus-4.8".to_string(),
            display_name: "Claude Opus 4.8".to_string(),
            context_window: 200000,
            max_output_tokens: 16384,
            pricing: Some(PricingInfo {
                input_per_1m: 15.0,
                output_per_1m: 75.0,
                cache_write_per_1m: Some(18.75),
                cache_read_per_1m: Some(1.5),
            }),
            model_type: "chat".to_string(),
            created: 1779897600,
        };

        let json = serde_json::to_string_pretty(&metadata).unwrap();
        assert!(json.contains("claude-opus-4.8"));
    }
}
