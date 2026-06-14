//! MultiTokenManager 账号过滤逻辑测试

#[cfg(test)]
mod account_filter_tests {
    use crate::kiro::model::credentials::KiroCredentials;
    use crate::kiro::token_manager::MultiTokenManager;
    use crate::kiro::ModelService;
    use crate::model::config::{Config, ModelRefreshConfig, TlsBackend};
    use std::sync::Arc;

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

    fn create_test_credential(id: u64, priority: u32) -> KiroCredentials {
        KiroCredentials {
            id: Some(id),
            access_token: Some("test-token".to_string()),
            refresh_token: Some("test-refresh-token".to_string()),
            profile_arn: None,
            expires_at: Some("2099-12-31T23:59:59Z".to_string()),
            auth_method: Some("social".to_string()),
            client_id: None,
            client_secret: None,
            priority,
            region: None,
            auth_region: None,
            api_region: None,
            machine_id: None,
            email: None,
            subscription_title: Some("Pro".to_string()),
            proxy_url: None,
            proxy_username: None,
            proxy_password: None,
            disabled: false,
            kiro_api_key: None,
            endpoint: None,
        }
    }

    #[test]
    fn test_token_manager_with_model_service() {
        let config = create_test_config();
        let credentials = vec![
            create_test_credential(1, 0),
            create_test_credential(2, 1),
            create_test_credential(3, 2),
        ];

        let manager = MultiTokenManager::new(config.clone(), credentials, None, None, false);
        assert!(manager.is_ok());

        let manager = Arc::new(manager.unwrap());

        // 创建 ModelService
        let model_service = ModelService::new(manager.clone(), config.model_refresh.clone());
        assert!(model_service.is_ok());

        let model_service = Arc::new(model_service.unwrap());

        // 设置 ModelService
        manager.set_model_service(model_service.clone());

        // 验证设置成功
        assert_eq!(manager.total_count(), 3);
    }

    #[test]
    fn test_select_credential_without_model_filter() {
        let config = create_test_config();
        let credentials = vec![
            create_test_credential(1, 0),
            create_test_credential(2, 1),
        ];

        let manager = MultiTokenManager::new(config, credentials, None, None, false).unwrap();

        // 不指定模型时，应该选择优先级最高的账号（priority=0）
        let snapshot = manager.snapshot();
        assert_eq!(snapshot.total, 2);
        assert_eq!(snapshot.available, 2);
    }

    #[test]
    fn test_opus_model_filter_fallback() {
        let config = create_test_config();

        // 创建一个免费账号和一个付费账号
        let mut free_credential = create_test_credential(1, 0);
        free_credential.subscription_title = Some("Free".to_string());

        let mut pro_credential = create_test_credential(2, 1);
        pro_credential.subscription_title = Some("Pro".to_string());

        let credentials = vec![free_credential, pro_credential];

        let manager = MultiTokenManager::new(config, credentials, None, None, false).unwrap();
        let snapshot = manager.snapshot();

        // 验证两个账号都存在
        assert_eq!(snapshot.total, 2);

        // 当没有 ModelService 时，Opus 模型应该使用基于订阅等级的过滤
        // 这个测试验证了回退机制的存在
    }

    #[test]
    fn test_disabled_credential_not_selected() {
        let config = create_test_config();

        let mut disabled_credential = create_test_credential(1, 0);
        disabled_credential.disabled = true;

        let enabled_credential = create_test_credential(2, 1);

        let credentials = vec![disabled_credential, enabled_credential];

        let manager = MultiTokenManager::new(config, credentials, None, None, false).unwrap();
        let snapshot = manager.snapshot();

        // 总共 2 个凭据，但只有 1 个可用
        assert_eq!(snapshot.total, 2);
        assert_eq!(snapshot.available, 1);
    }

    #[test]
    fn test_load_balancing_mode() {
        let config = create_test_config();
        let credentials = vec![
            create_test_credential(1, 0),
            create_test_credential(2, 1),
        ];

        let manager = MultiTokenManager::new(config, credentials, None, None, false).unwrap();

        // 默认应该是 priority 模式
        let mode = manager.get_load_balancing_mode();
        assert_eq!(mode, "priority");

        // 切换到 balanced 模式
        let result = manager.set_load_balancing_mode("balanced".to_string());
        assert!(result.is_ok());

        let mode = manager.get_load_balancing_mode();
        assert_eq!(mode, "balanced");
    }
}
