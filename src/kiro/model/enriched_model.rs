//! 合并后的完整模型信息

use super::model_metadata::PricingInfo;

/// 合并 API 数据和静态元数据后的完整模型信息
#[derive(Debug, Clone, serde::Serialize)]
pub struct EnrichedModel {
    pub id: String,
    pub object: String,
    pub created: i64,
    pub owned_by: String,
    pub display_name: String,
    pub model_type: String,
    pub max_tokens: i32,
    pub context_window: i32,
    pub supports_image: bool,
    pub input_modalities: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pricing: Option<PricingInfo>,
    pub available_accounts: Vec<String>,
}

impl EnrichedModel {
    /// 构建输入模态列表
    pub fn build_input_modalities(supports_image: bool) -> Vec<String> {
        let mut modalities = vec!["text".to_string()];
        if supports_image {
            modalities.push("image".to_string());
        }
        modalities
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enriched_model_serialization() {
        let model = EnrichedModel {
            id: "claude-opus-4.8".to_string(),
            object: "model".to_string(),
            created: 1779897600,
            owned_by: "anthropic".to_string(),
            display_name: "Claude Opus 4.8".to_string(),
            model_type: "chat".to_string(),
            max_tokens: 16384,
            context_window: 200000,
            supports_image: true,
            input_modalities: vec!["text".to_string(), "image".to_string()],
            pricing: None,
            available_accounts: vec!["account1".to_string()],
        };

        let json = serde_json::to_string(&model).unwrap();
        assert!(json.contains("claude-opus-4.8"));
        assert!(json.contains("anthropic"));
    }

    #[test]
    fn test_build_input_modalities() {
        let modalities = EnrichedModel::build_input_modalities(true);
        assert_eq!(modalities.len(), 2);
        assert_eq!(modalities[0], "text");
        assert_eq!(modalities[1], "image");

        let modalities = EnrichedModel::build_input_modalities(false);
        assert_eq!(modalities.len(), 1);
        assert_eq!(modalities[0], "text");
    }
}
