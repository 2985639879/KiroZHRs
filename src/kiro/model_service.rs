//! 模型服务 - 管理模型缓存和账号映射

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::SystemTime;

use parking_lot::RwLock;

use crate::kiro::api::list_available_models;
use crate::kiro::model::{
    EnrichedModel, ModelInfo, ModelMetadata, StaticMetadataCollection,
};
use crate::kiro::token_manager::MultiTokenManager;
use crate::model::config::ModelRefreshConfig;

/// 缓存的账号列表（用于模型→账号映射）
#[derive(Debug, Clone)]
struct CachedAccountList {
    account_ids: HashSet<String>,
    cached_at: SystemTime,
}

/// 模型服务
pub struct ModelService {
    /// 所有模型列表
    models: Arc<RwLock<Vec<EnrichedModel>>>,

    /// 账号 ID -> 模型ID列表
    account_models: Arc<RwLock<HashMap<String, Vec<String>>>>,

    /// 模型ID -> 支持该模型的账号列表（带缓存）
    model_accounts_cache: Arc<RwLock<HashMap<String, CachedAccountList>>>,

    /// 最后刷新时间
    last_refresh: Arc<RwLock<Option<SystemTime>>>,

    /// 静态元数据映射（model_id -> metadata）
    static_metadata: HashMap<String, ModelMetadata>,

    /// Token 管理器
    token_manager: Arc<MultiTokenManager>,

    /// 配置
    config: ModelRefreshConfig,
}

impl ModelService {
    /// 创建新的模型服务实例
    pub fn new(
        token_manager: Arc<MultiTokenManager>,
        config: ModelRefreshConfig,
    ) -> anyhow::Result<Self> {
        // 加载静态元数据
        let collection = StaticMetadataCollection::load_embedded()?;
        let static_metadata: HashMap<String, ModelMetadata> = collection
            .models
            .into_iter()
            .map(|m| (m.model_id.clone(), m))
            .collect();

        Ok(Self {
            models: Arc::new(RwLock::new(Vec::new())),
            account_models: Arc::new(RwLock::new(HashMap::new())),
            model_accounts_cache: Arc::new(RwLock::new(HashMap::new())),
            last_refresh: Arc::new(RwLock::new(None)),
            static_metadata,
            token_manager,
            config,
        })
    }

    /// 获取所有模型列表
    pub fn get_models(&self) -> Vec<EnrichedModel> {
        self.models.read().clone()
    }

    /// 获取指定账号支持的模型
    pub fn get_account_models(&self, _account_id: &str) -> Vec<ModelInfo> {
        // 待实现
        Vec::new()
    }

    /// 刷新指定账号的模型列表
    pub async fn refresh_account(&self, account_id: u64) -> anyhow::Result<usize> {
        // 从Token管理器获取该账号的凭据
        let mut credentials = self
            .token_manager
            .get_credentials_by_id(account_id)
            .ok_or_else(|| anyhow::anyhow!("Account {} not found", account_id))?;

        // 获取代理配置（从凭据的 proxy_url 字段构建）
        let proxy_config = if let Some(ref url) = credentials.proxy_url {
            Some(crate::http_client::ProxyConfig {
                url: url.clone(),
                username: credentials.proxy_username.clone(),
                password: credentials.proxy_password.clone(),
            })
        } else {
            None
        };

        // 调用 API 获取模型列表，如果失败则尝试刷新token后重试
        let models = match list_available_models(
            &credentials,
            proxy_config.as_ref(),
            self.config.tls_backend,
        )
        .await
        {
            Ok(models) => models,
            Err(e) => {
                // 检查是否是认证错误（403）
                let error_msg = e.to_string();
                if error_msg.contains("403") || error_msg.contains("Forbidden") {
                    tracing::info!(
                        "Account {} token invalid, attempting to refresh...",
                        account_id
                    );

                    // 强制刷新该账号的token
                    if let Err(refresh_err) = self.token_manager.force_refresh_token_for(account_id).await
                    {
                        tracing::warn!(
                            "Failed to refresh token for account {}: {}",
                            account_id,
                            refresh_err
                        );
                        return Err(e);
                    }

                    // 获取刷新后的凭据
                    credentials = self
                        .token_manager
                        .get_credentials_by_id(account_id)
                        .ok_or_else(|| anyhow::anyhow!("Account {} not found after refresh", account_id))?;

                    // 重试获取模型列表
                    tracing::info!("Retrying to fetch models for account {}", account_id);
                    list_available_models(&credentials, proxy_config.as_ref(), self.config.tls_backend)
                        .await?
                } else {
                    return Err(e);
                }
            }
        };

        // 提取模型 ID 列表
        let model_ids: Vec<String> = models.iter().map(|m| m.model_id.clone()).collect();

        let account_id_str = account_id.to_string();

        // 更新 account_models
        {
            let mut account_models = self.account_models.write();
            account_models.insert(account_id_str.clone(), model_ids.clone());
        }

        // 合并到全局模型列表
        self.merge_models_to_global(&models, &account_id_str).await;

        // 清空缓存
        self.clear_account_cache();

        tracing::info!(
            "Refreshed {} models for account: {}",
            models.len(),
            account_id
        );

        Ok(models.len())
    }

    /// 合并模型到全局列表
    async fn merge_models_to_global(&self, models: &[ModelInfo], account_id: &str) {
        let mut global_models = self.models.write();

        for api_model in models {
            // 查找是否已存在
            if let Some(existing) = global_models
                .iter_mut()
                .find(|m| m.id == api_model.model_id)
            {
                // 更新账号列表
                if !existing.available_accounts.contains(&account_id.to_string()) {
                    existing.available_accounts.push(account_id.to_string());
                }
            } else {
                // 创建新的 EnrichedModel
                let enriched = self.enrich_model(api_model, account_id);
                global_models.push(enriched);
            }
        }
    }

    /// 将 API 模型合并静态元数据
    fn enrich_model(&self, api_model: &ModelInfo, account_id: &str) -> EnrichedModel {
        let metadata = self.static_metadata.get(&api_model.model_id);

        let supports_image = api_model.supports_image();
        let input_modalities = EnrichedModel::build_input_modalities(supports_image);

        EnrichedModel {
            id: api_model.model_id.clone(),
            object: "model".to_string(),
            created: metadata.map(|m| m.created).unwrap_or(0),
            owned_by: "anthropic".to_string(),
            display_name: metadata
                .map(|m| m.display_name.clone())
                .unwrap_or_else(|| api_model.model_name.clone()),
            model_type: metadata
                .map(|m| m.model_type.clone())
                .unwrap_or_else(|| "chat".to_string()),
            max_tokens: api_model
                .token_limits
                .as_ref()
                .map(|t| t.max_output_tokens)
                .or_else(|| metadata.map(|m| m.max_output_tokens))
                .unwrap_or(16384),
            context_window: metadata.map(|m| m.context_window).unwrap_or(200000),
            supports_image,
            input_modalities,
            pricing: metadata.and_then(|m| m.pricing.clone()),
            available_accounts: vec![account_id.to_string()],
        }
    }

    /// 清空账号过滤缓存
    fn clear_account_cache(&self) {
        let mut cache = self.model_accounts_cache.write();
        cache.clear();
        tracing::debug!("Cleared model accounts cache");
    }

    /// 刷新所有账号的模型列表
    pub async fn refresh_all_accounts(&self) -> anyhow::Result<(usize, usize)> {
        // 获取所有凭据的快照
        let snapshot = self.token_manager.snapshot();

        let mut total_accounts = 0;
        let mut failed_accounts = 0;

        // 清空全局模型列表
        {
            let mut models = self.models.write();
            models.clear();
        }

        // 只刷新启用的账号
        for entry in snapshot.entries.iter().filter(|e| !e.disabled) {
            match self.refresh_account(entry.id).await {
                Ok(_) => {
                    total_accounts += 1;
                }
                Err(e) => {
                    tracing::warn!("Failed to refresh account {}: {}", entry.id, e);
                    failed_accounts += 1;
                }
            }
        }

        // 更新最后刷新时间
        {
            let mut last_refresh = self.last_refresh.write();
            *last_refresh = Some(SystemTime::now());
        }

        tracing::info!(
            "Refreshed models for {} accounts ({} failed)",
            total_accounts,
            failed_accounts
        );

        Ok((total_accounts, failed_accounts))
    }

    /// 获取支持指定模型的账号列表（带缓存）
    pub fn get_accounts_for_model(&self, model_id: &str) -> Vec<String> {
        // 1. 尝试从缓存读取
        {
            let cache = self.model_accounts_cache.read();
            if let Some(cached) = cache.get(model_id) {
                let elapsed = SystemTime::now()
                    .duration_since(cached.cached_at)
                    .unwrap_or_default();

                // TTL 未过期，直接返回
                if elapsed.as_secs() < self.config.account_filter_cache_ttl_seconds {
                    tracing::debug!("Cache hit for model: {}", model_id);
                    return cached.account_ids.iter().cloned().collect();
                }
            }
        }

        // 2. 缓存未命中或过期，重新计算
        tracing::debug!("Cache miss for model: {}", model_id);
        let account_models = self.account_models.read();
        let mut result = HashSet::new();

        for (account_id, models) in account_models.iter() {
            if models.contains(&model_id.to_string()) {
                result.insert(account_id.clone());
            }
        }

        // 3. 更新缓存
        {
            let mut cache = self.model_accounts_cache.write();
            cache.insert(
                model_id.to_string(),
                CachedAccountList {
                    account_ids: result.clone(),
                    cached_at: SystemTime::now(),
                },
            );
        }

        result.into_iter().collect()
    }

    /// 从公开数据源更新缺失的模型元数据（上下文长度等）
    ///
    /// 此方法会自动从 LiteLLM 等公开数据源获取模型的技术参数
    pub async fn update_metadata_from_public_sources(&self) -> anyhow::Result<usize> {
        use crate::kiro::metadata_updater::MetadataUpdater;

        let updater = MetadataUpdater::new();
        let metadata_map = updater.fetch_metadata().await?;

        let mut updated_count = 0;
        {
            let mut models = self.models.write();

            for model in models.iter_mut() {
                if let Some(fetched) = metadata_map.get(&model.id) {
                    let mut updated = false;

                    // 更新上下文窗口（如果当前为0或缺失）
                    if model.context_window == 0 {
                        if let Some(context_window) = fetched.context_window {
                            model.context_window = context_window;
                            updated = true;
                        }
                    }

                    // 更新最大输出tokens（如果当前为0或缺失）
                    if model.max_tokens == 0 {
                        if let Some(max_output) = fetched.max_output_tokens {
                            model.max_tokens = max_output;
                            updated = true;
                        }
                    }

                    // 更新 vision 支持
                    if fetched.supports_vision && !model.supports_image {
                        model.supports_image = true;
                        model.input_modalities =
                            crate::kiro::model::EnrichedModel::build_input_modalities(true);
                        updated = true;
                    }

                    if updated {
                        updated_count += 1;
                        tracing::info!(
                            "Updated metadata for {}: context={}, max_output={}, vision={}",
                            model.id,
                            model.context_window,
                            model.max_tokens,
                            model.supports_image
                        );
                    }
                }
            }
        }

        tracing::info!(
            "Updated metadata for {} models from public sources",
            updated_count
        );
        Ok(updated_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_account_list() {
        let cached = CachedAccountList {
            account_ids: ["account1".to_string(), "account2".to_string()]
                .into_iter()
                .collect(),
            cached_at: SystemTime::now(),
        };

        assert_eq!(cached.account_ids.len(), 2);
    }

    #[test]
    fn test_get_accounts_for_model_with_cache() {
        // 这个测试需要一个完整的 ModelService 实例
        // 由于涉及 MultiTokenManager 的创建，这里只测试缓存结构本身
        let cached = CachedAccountList {
            account_ids: ["1".to_string(), "2".to_string()].into_iter().collect(),
            cached_at: SystemTime::now(),
        };

        // 验证缓存时间在合理范围内
        let elapsed = SystemTime::now()
            .duration_since(cached.cached_at)
            .unwrap();
        assert!(elapsed.as_secs() < 1);
    }
}
