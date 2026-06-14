//! 从 Kiro API 返回的模型信息

/// 从 ListAvailableModels API 返回的模型信息
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ModelInfo {
    #[serde(rename = "modelId")]
    pub model_id: String,

    #[serde(rename = "modelName")]
    pub model_name: String,

    #[serde(default)]
    pub description: String,

    #[serde(rename = "supportedInputTypes", default)]
    pub input_types: Vec<String>,

    #[serde(rename = "rateMultiplier", default)]
    pub rate_multiplier: f64,

    #[serde(rename = "tokenLimits")]
    pub token_limits: Option<TokenLimits>,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct TokenLimits {
    #[serde(rename = "maxInputTokens")]
    pub max_input_tokens: i32,

    #[serde(rename = "maxOutputTokens")]
    pub max_output_tokens: i32,
}

impl ModelInfo {
    /// 判断模型是否支持图片输入
    pub fn supports_image(&self) -> bool {
        self.input_types
            .iter()
            .any(|t| t.to_lowercase().contains("image") || t.to_lowercase().contains("vision"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_info_deserialization() {
        let json = r#"{
            "modelId": "claude-opus-4.8",
            "modelName": "Claude Opus 4.8",
            "description": "Most capable model",
            "supportedInputTypes": ["text", "image"],
            "rateMultiplier": 1.0,
            "tokenLimits": {
                "maxInputTokens": 200000,
                "maxOutputTokens": 16384
            }
        }"#;

        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.model_id, "claude-opus-4.8");
        assert!(model.supports_image());
    }

    #[test]
    fn test_model_info_without_optional_fields() {
        let json = r#"{
            "modelId": "test-model",
            "modelName": "Test Model"
        }"#;

        let model: ModelInfo = serde_json::from_str(json).unwrap();
        assert_eq!(model.model_id, "test-model");
        assert_eq!(model.description, "");
        assert_eq!(model.input_types.len(), 0);
    }
}
