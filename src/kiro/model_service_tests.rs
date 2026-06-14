//! ModelService 单元测试

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kiro::model::{ModelInfo, ModelMetadata};
    use crate::kiro::token_manager::MultiTokenManager;
    use crate::model::config::{Config, ModelRefreshConfig, TlsBackend};
    use std::sync::Arc;
    use std::time::SystemTime;

    fn create_test_config() -> Config {
        Config {
            host: "127.0.0.1".to_string(),
            port: 8990,
            api_key: "test-key".to_string(),
            region: "us-east-1".to_string(),
            auth_region: None,
            api_region: None,
            kiro_version: "0.9.2".to_string(),
            machine_id: None,
            system_version: None,
            node_version: None,
            tls_backend: TlsBackend::Rustls,
            count_tokens_api_url: None,
            count_tokens_api_key: None,
            count_tokens_auth_type: None,
            proxy_url: None,
            proxy_username: None,
            proxy_password: None,
            admin_api_key: None,
            load_balancing_mode: "priority".to_string(),
            extract_thinking: true,
            default_endpoint: "ide".to_string(),
            model_refresh: ModelRefreshConfig {
                enabled: false,
                interval_seconds: 3600,
                account_filter_cache_ttl_seconds: 300,
                tls_backend: TlsBackend::Rustls,
            },
        }
    }

    fn create_test_token_manager() -> Arc<MultiTokenManager> {
        let config = create_test_config();
        let credentials = vec![];
        let manager = MultiTokenManager::new(config, credentials, None, None, false).unwrap();
        Arc::new(manager)
    }

    #[test]
    fn test_model_service_creation() {
        let token_manager = create_test_token_manager();
        let config = ModelRefreshConfig {
            enabled: false,
            interval_seconds: 3600,
            account_filter_cache_ttl_seconds: 300,
            tls_backend: TlsBackend::Rustls,
        };

        let service = ModelService::new(token_manager, config);
        assert!(service.is_ok());
    }

    #[test]
    fn test_get_models_empty() {
        let token_manager = create_test_token_manager();
        let config = ModelRefreshConfig {
            enabled: false,
            interval_seconds: 3600,
            account_filter_cache_ttl_seconds: 300,
            tls_backend: TlsBackend::Rustls,
        };

        let service = ModelService::new(token_manager, config).unwrap();
        let models = service.get_models();
        assert_eq!(models.len(), 0);
    }

    #[test]
    fn test_get_accounts_for_model_empty() {
        let token_manager = create_test_token_manager();
        let config = ModelRefreshConfig {
            enabled: false,
            interval_seconds: 3600,
            account_filter_cache_ttl_seconds: 300,
            tls_backend: TlsBackend::Rustls,
        };

        let service = ModelService::new(token_manager, config).unwrap();
        let accounts = service.get_accounts_for_model("claude-sonnet-4");
        assert_eq!(accounts.len(), 0);
    }

    #[test]
    fn test_get_accounts_for_model_cache() {
        let token_manager = create_test_token_manager();
        let config = ModelRefreshConfig {
            enabled: false,
            interval_seconds: 3600,
            account_filter_cache_ttl_seconds: 300,
            tls_backend: TlsBackend::Rustls,
        };

        let service = ModelService::new(token_manager, config).unwrap();

        // 第一次调用 - 缓存未命中
        let accounts1 = service.get_accounts_for_model("claude-sonnet-4");
        assert_eq!(accounts1.len(), 0);

        // 第二次调用 - 应该命中缓存
        let accounts2 = service.get_accounts_for_model("claude-sonnet-4");
        assert_eq!(accounts2.len(), 0);
    }

    #[test]
    fn test_clear_account_cache() {
        let token_manager = create_test_token_manager();
        let config = ModelRefreshConfig {
            enabled: false,
            interval_seconds: 3600,
            account_filter_cache_ttl_seconds: 300,
            tls_backend: TlsBackend::Rustls,
        };

        let service = ModelService::new(token_manager, config).unwrap();

        // 触发缓存
        let _ = service.get_accounts_for_model("test-model");

        // 清空缓存
        service.clear_account_cache();

        // 缓存应该被清空，再次查询会重新计算
        let accounts = service.get_accounts_for_model("test-model");
        assert_eq!(accounts.len(), 0);
    }

    #[test]
    fn test_enrich_model() {
        let token_manager = create_test_token_manager();
        let config = ModelRefreshConfig {
            enabled: false,
            interval_seconds: 3600,
            account_filter_cache_ttl_seconds: 300,
            tls_backend: TlsBackend::Rustls,
        };

        let service = ModelService::new(token_manager, config).unwrap();

        let api_model = ModelInfo {
            model_id: "claude-sonnet-4-20250514".to_string(),
            model_name: "Claude Sonnet 4".to_string(),
            token_limits: None,
            created_at: "2025-01-01T00:00:00Z".to_string(),
            updated_at: "2025-01-01T00:00:00Z".to_string(),
        };

        let enriched = service.enrich_model(&api_model, "1");

        assert_eq!(enriched.id, "claude-sonnet-4-20250514");
        assert_eq!(enriched.available_accounts.len(), 1);
        assert_eq!(enriched.available_accounts[0], "1");
    }
}
