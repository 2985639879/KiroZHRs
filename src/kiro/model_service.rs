//! 模型服务 - 管理模型缓存和账号映射

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use parking_lot::RwLock;
use tokio::time::{interval, Duration};

use crate::kiro::api::list_available_models;
use crate::kiro::model::{
    EnrichedModel, ModelInfo, ModelMetadata, PricingInfo, StaticMetadataCollection,
};
use crate::kiro::model::credentials::KiroCredentials;
use crate::kiro::token_manager::MultiTokenManager;
use crate::model::config::ModelRefreshConfig;

/// 缓存的账号列表（用于模型→账号映射）
#[derive(Debug, Clone)]
struct CachedAccountList {
    account_ids: Vec<String>,
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
    pub fn get_account_models(&self, account_id: &str) -> Vec<ModelInfo> {
        // 待实现
        Vec::new()
    }
}
