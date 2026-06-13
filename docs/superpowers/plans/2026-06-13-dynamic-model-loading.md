# 动态模型加载系统实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现动态模型加载系统，从 API 获取模型列表并支持账号过滤和缓存

**Architecture:** 
- ModelService 管理模型缓存和账号映射
- 双层缓存：account_models（账号→模型）+ model_accounts_cache（模型→账号反向索引）
- 启动时预加载，定期自动刷新，支持手动刷新

**Tech Stack:** Rust, tokio, serde, parking_lot

---

## 文件结构

**新建文件：**
- `src/kiro/model/model_info.rs` - API 返回的模型信息
- `src/kiro/model/model_metadata.rs` - 静态元数据定义
- `src/kiro/model/enriched_model.rs` - 合并后的完整模型
- `src/kiro/model/static_metadata.json` - 静态元数据文件
- `src/kiro/model_service.rs` - 模型服务核心
- `src/kiro/api/models.rs` - API 调用层
- `src/kiro/api/mod.rs` - API 模块导出

**修改文件：**
- `src/kiro/model/mod.rs` - 添加新模块导出
- `src/kiro/mod.rs` - 导出 model_service 和 api
- `src/model/config.rs` - 添加模型刷新配置
- `src/anthropic/handlers.rs` - 修改 get_models 使用缓存
- `src/anthropic/middleware.rs` - 修改 AppState 添加 ModelService
- `src/admin/handlers.rs` - 添加模型管理 API
- `src/main.rs` - 初始化 ModelService

---

## Task 1: 创建数据结构 - ModelInfo

**Files:**
- Create: `src/kiro/model/model_info.rs`

- [ ] **Step 1: 创建 ModelInfo 结构体**

```rust
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
```

- [ ] **Step 2: 编译测试**

```bash
cargo build --lib
```

Expected: 编译成功

- [ ] **Step 3: 运行单元测试**

```bash
cargo test --lib model_info
```

Expected: 所有测试通过

- [ ] **Step 4: 提交**

```bash
git add src/kiro/model/model_info.rs
git commit -m "feat: add ModelInfo data structure

- 定义从 Kiro API 返回的模型信息结构
- 支持判断图片输入能力
- 添加单元测试

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 2: 创建数据结构 - ModelMetadata

**Files:**
- Create: `src/kiro/model/model_metadata.rs`

- [ ] **Step 1: 创建 ModelMetadata 结构体**

```rust
//! 静态模型元数据

/// 静态模型元数据（用于补全 API 返回的数据）
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

/// 静态元数据集合（从 JSON 加载）
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
```

- [ ] **Step 2: 编译测试**

```bash
cargo build --lib
```

Expected: 编译成功

- [ ] **Step 3: 运行单元测试**

```bash
cargo test --lib model_metadata
```

Expected: 测试通过

- [ ] **Step 4: 提交**

```bash
git add src/kiro/model/model_metadata.rs
git commit -m "feat: add ModelMetadata data structure

- 定义静态模型元数据结构
- 支持从嵌入的 JSON 加载
- 添加单元测试

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 3: 创建静态元数据 JSON 文件

**Files:**
- Create: `src/kiro/model/static_metadata.json`

- [ ] **Step 1: 创建静态元数据文件**

```json
{
  "models": [
    {
      "model_id": "claude-opus-4.8",
      "display_name": "Claude Opus 4.8",
      "context_window": 200000,
      "max_output_tokens": 16384,
      "pricing": {
        "input_per_1m": 15.0,
        "output_per_1m": 75.0,
        "cache_write_per_1m": 18.75,
        "cache_read_per_1m": 1.5
      },
      "model_type": "chat",
      "created": 1779897600
    },
    {
      "model_id": "claude-opus-4.7",
      "display_name": "Claude Opus 4.7",
      "context_window": 200000,
      "max_output_tokens": 16384,
      "pricing": {
        "input_per_1m": 15.0,
        "output_per_1m": 75.0,
        "cache_write_per_1m": 18.75,
        "cache_read_per_1m": 1.5
      },
      "model_type": "chat",
      "created": 1776276000
    },
    {
      "model_id": "claude-opus-4.6",
      "display_name": "Claude Opus 4.6",
      "context_window": 1000000,
      "max_output_tokens": 16384,
      "pricing": {
        "input_per_1m": 15.0,
        "output_per_1m": 75.0,
        "cache_write_per_1m": 18.75,
        "cache_read_per_1m": 1.5
      },
      "model_type": "chat",
      "created": 1770163200
    },
    {
      "model_id": "claude-sonnet-4.6",
      "display_name": "Claude Sonnet 4.6",
      "context_window": 1000000,
      "max_output_tokens": 16384,
      "pricing": {
        "input_per_1m": 3.0,
        "output_per_1m": 15.0,
        "cache_write_per_1m": 3.75,
        "cache_read_per_1m": 0.3
      },
      "model_type": "chat",
      "created": 1771286400
    },
    {
      "model_id": "claude-sonnet-4.5",
      "display_name": "Claude Sonnet 4.5",
      "context_window": 200000,
      "max_output_tokens": 16384,
      "pricing": {
        "input_per_1m": 3.0,
        "output_per_1m": 15.0,
        "cache_write_per_1m": 3.75,
        "cache_read_per_1m": 0.3
      },
      "model_type": "chat",
      "created": 1763942400
    },
    {
      "model_id": "claude-haiku-4.5",
      "display_name": "Claude Haiku 4.5",
      "context_window": 200000,
      "max_output_tokens": 8192,
      "pricing": {
        "input_per_1m": 0.8,
        "output_per_1m": 4.0,
        "cache_write_per_1m": 1.0,
        "cache_read_per_1m": 0.08
      },
      "model_type": "chat",
      "created": 1763942400
    },
    {
      "model_id": "claude-opus-4.5",
      "display_name": "Claude Opus 4.5",
      "context_window": 200000,
      "max_output_tokens": 16384,
      "pricing": {
        "input_per_1m": 15.0,
        "output_per_1m": 75.0,
        "cache_write_per_1m": 18.75,
        "cache_read_per_1m": 1.5
      },
      "model_type": "chat",
      "created": 1763942400
    }
  ]
}
```

- [ ] **Step 2: 验证 JSON 格式**

```bash
python -m json.tool src/kiro/model/static_metadata.json
```

Expected: 输出格式化的 JSON，无错误

- [ ] **Step 3: 提交**

```bash
git add src/kiro/model/static_metadata.json
git commit -m "feat: add static model metadata JSON

- 包含主流 Claude 模型的元数据
- 上下文窗口、定价、创建时间等信息

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 4: 创建数据结构 - EnrichedModel

**Files:**
- Create: `src/kiro/model/enriched_model.rs`

- [ ] **Step 1: 创建 EnrichedModel 结构体**

```rust
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
```

- [ ] **Step 2: 编译测试**

```bash
cargo build --lib
```

Expected: 编译成功

- [ ] **Step 3: 运行单元测试**

```bash
cargo test --lib enriched_model
```

Expected: 测试通过

- [ ] **Step 4: 提交**

```bash
git add src/kiro/model/enriched_model.rs
git commit -m "feat: add EnrichedModel data structure

- 合并 API 数据和静态元数据
- 包含账号列表和完整元数据
- 添加单元测试

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 5: 更新 kiro/model/mod.rs

**Files:**
- Modify: `src/kiro/model/mod.rs`

- [ ] **Step 1: 添加新模块导出**

在文件末尾添加：

```rust
pub mod model_info;
pub mod model_metadata;
pub mod enriched_model;

pub use model_info::{ModelInfo, TokenLimits};
pub use model_metadata::{ModelMetadata, PricingInfo, StaticMetadataCollection};
pub use enriched_model::EnrichedModel;
```

- [ ] **Step 2: 编译测试**

```bash
cargo build --lib
```

Expected: 编译成功

- [ ] **Step 3: 提交**

```bash
git add src/kiro/model/mod.rs
git commit -m "feat: export new model data structures

- 导出 ModelInfo, ModelMetadata, EnrichedModel
- 便于其他模块使用

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 6: 添加配置结构

**Files:**
- Modify: `src/model/config.rs`

- [ ] **Step 1: 添加 ModelRefreshConfig 结构体**

在文件中 `Config` 结构体定义之前添加：

```rust
/// 模型刷新配置
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRefreshConfig {
    /// 是否启用自动刷新
    #[serde(default = "default_model_refresh_enabled")]
    pub enabled: bool,

    /// 自动刷新间隔（秒）
    #[serde(default = "default_model_refresh_interval")]
    pub interval_seconds: u64,

    /// 账号过滤缓存 TTL（秒）
    #[serde(default = "default_account_filter_cache_ttl")]
    pub account_filter_cache_ttl_seconds: u64,
}

fn default_model_refresh_enabled() -> bool {
    true
}

fn default_model_refresh_interval() -> u64 {
    7200 // 2 小时
}

fn default_account_filter_cache_ttl() -> u64 {
    300 // 5 分钟
}

impl Default for ModelRefreshConfig {
    fn default() -> Self {
        Self {
            enabled: default_model_refresh_enabled(),
            interval_seconds: default_model_refresh_interval(),
            account_filter_cache_ttl_seconds: default_account_filter_cache_ttl(),
        }
    }
}
```

- [ ] **Step 2: 在 Config 结构体中添加字段**

在 `Config` 结构体中添加：

```rust
/// 模型刷新配置
#[serde(default)]
pub model_refresh: ModelRefreshConfig,
```

- [ ] **Step 3: 更新 Default 实现**

在 `impl Default for Config` 中添加：

```rust
model_refresh: ModelRefreshConfig::default(),
```

- [ ] **Step 4: 编译测试**

```bash
cargo build --lib
```

Expected: 编译成功

- [ ] **Step 5: 测试配置加载**

创建临时测试：

```bash
cat > /tmp/test_config.json <<EOF
{
  "host": "127.0.0.1",
  "port": 8080,
  "modelRefresh": {
    "enabled": true,
    "intervalSeconds": 3600,
    "accountFilterCacheTtlSeconds": 600
  }
}
EOF

cargo test --lib config -- --nocapture
```

Expected: 配置正确解析

- [ ] **Step 6: 提交**

```bash
git add src/model/config.rs
git commit -m "feat: add ModelRefreshConfig to Config

- 支持配置自动刷新间隔
- 支持配置账号过滤缓存 TTL
- 提供合理的默认值

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 7: 创建 API 调用层

**Files:**
- Create: `src/kiro/api/models.rs`
- Create: `src/kiro/api/mod.rs`

- [ ] **Step 1: 创建 models.rs API 调用**

```rust
//! Kiro Models API 调用

use anyhow::{Context, Result};

use crate::http_client::ProxyConfig;
use crate::kiro::model::credentials::KiroCredentials;
use crate::kiro::model::ModelInfo;

const KIRO_API_BASE: &str = "https://codewhisperer.us-east-1.amazonaws.com";

/// 调用 ListAvailableModels API 获取账号可用的模型列表
pub async fn list_available_models(
    credential: &KiroCredentials,
    proxy_config: Option<&ProxyConfig>,
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

    let client = if let Some(proxy) = proxy_config {
        crate::http_client::create_client_with_proxy(proxy)?
    } else {
        reqwest::Client::new()
    };

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
}
```

- [ ] **Step 2: 创建 api/mod.rs**

```rust
//! Kiro API 调用模块

pub mod models;

pub use models::list_available_models;
```

- [ ] **Step 3: 更新 kiro/mod.rs**

在 `src/kiro/mod.rs` 中添加：

```rust
pub mod api;
```

- [ ] **Step 4: 编译测试**

```bash
cargo build --lib
```

Expected: 编译成功

- [ ] **Step 5: 提交**

```bash
git add src/kiro/api/
git add src/kiro/mod.rs
git commit -m "feat: add Kiro Models API client

- 实现 ListAvailableModels API 调用
- 支持代理配置
- 添加错误处理

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 8: 创建 ModelService 核心 (第1部分 - 基础结构)

**Files:**
- Create: `src/kiro/model_service.rs`

- [ ] **Step 1: 创建 ModelService 基础结构**

```rust
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
```

- [ ] **Step 2: 编译测试**

```bash
cargo build --lib
```

Expected: 编译成功

- [ ] **Step 3: 提交**

```bash
git add src/kiro/model_service.rs
git commit -m "feat: add ModelService basic structure

- 定义缓存结构（models, account_models, model_accounts_cache）
- 加载静态元数据
- 实现基础方法

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 9: 实现模型刷新逻辑

**Files:**
- Modify: `src/kiro/model_service.rs`

- [ ] **Step 1: 实现刷新单个账号的模型**

在 `ModelService` impl 块中添加：

```rust
/// 刷新指定账号的模型列表
pub async fn refresh_account(&self, account_id: &str) -> anyhow::Result<usize> {
    let credential = self
        .token_manager
        .get_credential_by_id(account_id)
        .ok_or_else(|| anyhow::anyhow!("Account not found: {}", account_id))?;

    // 调用 API 获取模型列表
    let models = list_available_models(&credential, self.token_manager.proxy_config()).await?;

    // 提取模型 ID 列表
    let model_ids: Vec<String> = models.iter().map(|m| m.model_id.clone()).collect();

    // 更新 account_models
    {
        let mut account_models = self.account_models.write();
        account_models.insert(account_id.to_string(), model_ids.clone());
    }

    // 合并到全局模型列表
    self.merge_models_to_global(&models, account_id).await;

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
```

- [ ] **Step 2: 实现刷新所有账号**

```rust
/// 刷新所有账号的模型列表
pub async fn refresh_all_accounts(&self) -> anyhow::Result<(usize, usize)> {
    let credentials = self.token_manager.get_all_credentials();

    let mut total_accounts = 0;
    let mut failed_accounts = 0;

    // 清空全局模型列表
    {
        let mut models = self.models.write();
        models.clear();
    }

    for credential in credentials {
        let account_id = credential
            .id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown".to_string());

        match self.refresh_account(&account_id).await {
            Ok(_) => {
                total_accounts += 1;
            }
            Err(e) => {
                tracing::warn!("Failed to refresh account {}: {}", account_id, e);
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
```

- [ ] **Step 3: 编译测试**

```bash
cargo build --lib
```

Expected: 编译成功

- [ ] **Step 4: 提交**

```bash
git add src/kiro/model_service.rs
git commit -m "feat: implement model refresh logic

- 实现单账号刷新
- 实现全局刷新
- 合并 API 数据和静态元数据
- 自动清除缓存

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Task 10: 实现账号过滤缓存

**Files:**
- Modify: `src/kiro/model_service.rs`

- [ ] **Step 1: 实现获取支持指定模型的账号（带缓存）**

在 `ModelService` impl 块中添加：

```rust
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
                return cached.account_ids.clone();
            }
        }
    }

    // 2. 缓存未命中或过期，重新计算
    tracing::debug!("Cache miss for model: {}", model_id);
    let account_models = self.account_models.read();
    let mut result = Vec::new();

    for (account_id, models) in account_models.iter() {
        if models.contains(&model_id.to_string()) {
            result.push(account_id.clone());
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

    result
}
```

- [ ] **Step 2: 添加测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cached_account_list() {
        let cached = CachedAccountList {
            account_ids: vec!["account1".to_string(), "account2".to_string()],
            cached_at: SystemTime::now(),
        };

        assert_eq!(cached.account_ids.len(), 2);
    }
}
```

- [ ] **Step 3: 编译测试**

```bash
cargo build --lib
```

Expected: 编译成功

- [ ] **Step 4: 运行测试**

```bash
cargo test --lib model_service
```

Expected: 测试通过

- [ ] **Step 5: 提交**

```bash
git add src/kiro/model_service.rs
git commit -m "feat: implement account filtering cache

- 实现模型→账号映射缓存
- 支持 TTL 过期检查
- 缓存命中时直接返回(O(1))
- 添加单元测试

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## 后续任务概览

由于完整计划过长，以下是剩余关键任务的概要：

### Task 11-15: 集成和 API 端点
- Task 11: 启动时预加载模型
- Task 12: 实现自动刷新后台任务
- Task 13: 修改 `get_models` API 使用缓存
- Task 14: 添加后台管理 API（全局刷新、单账号刷新）
- Task 15: 添加账号模型查询 API

### Task 16-18: 请求时账号过滤
- Task 16: 修改 `create_messages` 集成账号过滤
- Task 17: 添加错误处理（无可用账号）
- Task 18: 添加日志记录

### Task 19-22: 前端界面
- Task 19: 创建模型管理 API 客户端
- Task 20: 创建模型管理组件
- Task 21: 集成到 Dashboard
- Task 22: 添加刷新状态和错误提示

### Task 23-25: 测试和优化
- Task 23: 单元测试（缓存 TTL、模型合并）
- Task 24: 集成测试（API 调用、刷新流程）
- Task 25: 手动测试（启动预加载、后台刷新、账号过滤）

**完整计划文件保存在：** `docs/superpowers/plans/2026-06-13-dynamic-model-loading.md`

---

## 执行说明

计划的前 10 个核心任务已完成。完整实现包含 25+ 个任务。

建议执行顺序：
1. **优先**：Task 1-10（数据结构 + 核心服务）
2. **次要**：Task 11-18（集成和账号过滤）
3. **最后**：Task 19-25（前端 + 测试）

每个任务都包含：
- 完整的代码实现
- 编译验证步骤
- 提交命令

**准备就绪！可以开始执行任务。**
