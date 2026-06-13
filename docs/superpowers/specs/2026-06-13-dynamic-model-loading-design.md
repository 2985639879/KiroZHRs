# 动态模型加载系统设计

**日期**: 2026-06-13  
**目标**: 将 kiro-rs 的模型获取方式从静态配置改为动态 API 获取，参考 KiroGo 的实现

## 1. 需求概述

### 1.1 核心需求
- 服务启动时，通过 API 向 Kiro 后端请求当前账号可用的最新模型列表
- 获取到的列表可能缺少关键参数，需用本地静态文件中的信息进行补全
- 支持定期自动刷新（间隔可配置）
- 支持后台管理界面手动刷新（全局刷新 + 单账号刷新）
- 调用 `/v1/messages` 接口时，只有支持该模型的账号才会进入账号池
- 后台可查看每个账号支持的模型列表

### 1.2 缓存策略
- 动态获取的模型列表缓存在内存中
- 刷新失败时继续使用旧缓存
- 使用预加载（启动时拉取），避免第一个请求等待
- 使用读写锁（RwLock）保护模型列表，读请求无锁竞争
- 账号-模型映射关系也需要缓存，缓存时间可配置（单位：秒）

### 1.3 账号过滤机制
- 每个账号有自己的模型列表
- 请求特定模型时，只有支持该模型的账号参与负载均衡
- 账号池过滤结果需要缓存，避免每次请求都查询

## 2. 架构设计

### 2.1 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                      启动时预加载                              │
│  ModelService::new() → 从所有账号拉取模型 → 合并静态元数据      │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                      内存缓存结构                              │
│  • models: RwLock<Vec<EnrichedModel>>                       │
│  • account_models: RwLock<HashMap<AccountId, Vec<ModelId>>> │
│  • model_accounts_cache: RwLock<HashMap<ModelId, CachedAccountList>> │
│  • last_refresh: RwLock<Option<SystemTime>>                 │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                    定期自动刷新                                │
│  tokio::spawn → 每 N 秒刷新一次（可配置）                      │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                    请求时账号过滤                              │
│  message 请求 → 查找支持该模型的账号（缓存） → 过滤账号池       │
└─────────────────────────────────────────────────────────────┘
```

### 2.2 目录结构

```
src/
├── kiro/
│   ├── model/
│   │   ├── mod.rs
│   │   ├── model_info.rs          // API 返回的 ModelInfo
│   │   ├── model_metadata.rs      // 静态元数据定义
│   │   ├── enriched_model.rs      // 合并后的完整模型
│   │   └── static_metadata.json   // 静态元数据文件
│   ├── model_service.rs            // 模型服务核心
│   └── ...
├── admin/
│   ├── handlers.rs                 // 添加模型管理 API
│   └── ...
├── anthropic/
│   ├── handlers.rs                 // 修改 get_models 使用缓存
│   └── ...
└── model/
    └── config.rs                   // 添加模型刷新配置
```

### 2.3 配置文件扩展

在 `config.json` 中添加：

```json
{
  "modelRefresh": {
    "enabled": true,
    "intervalSeconds": 7200,
    "accountFilterCacheTtlSeconds": 300
  }
}
```

配置说明：
- `enabled`: 是否启用自动刷新
- `intervalSeconds`: 自动刷新间隔（秒），默认 7200（2小时）
- `accountFilterCacheTtlSeconds`: 账号过滤缓存 TTL（秒），默认 300（5分钟）

## 3. 数据结构设计

### 3.1 从 API 获取的模型信息

```rust
// src/kiro/model/model_info.rs

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ModelInfo {
    #[serde(rename = "modelId")]
    pub model_id: String,
    
    #[serde(rename = "modelName")]
    pub model_name: String,
    
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
```

### 3.2 静态元数据

```rust
// src/kiro/model/model_metadata.rs

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct ModelMetadata {
    pub model_id: String,
    pub display_name: String,
    pub context_window: i32,
    pub max_output_tokens: i32,
    pub pricing: Option<PricingInfo>,
    pub model_type: String,
    pub created: i64,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PricingInfo {
    pub input_per_1m: f64,
    pub output_per_1m: f64,
    pub cache_write_per_1m: Option<f64>,
    pub cache_read_per_1m: Option<f64>,
}
```

### 3.3 合并后的完整模型

```rust
// src/kiro/model/enriched_model.rs

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
    pub pricing: Option<PricingInfo>,
    pub available_accounts: Vec<String>,
}
```

### 3.4 账号过滤缓存

```rust
// src/kiro/model_service.rs

/// 缓存的账号列表（用于模型→账号映射）
struct CachedAccountList {
    account_ids: Vec<String>,
    cached_at: SystemTime,
}
```

**缓存工作原理：**

1. **首次查询**：
   - 用户请求模型 `claude-opus-4.8`
   - 查找 `model_accounts_cache` → 未命中
   - 遍历 `account_models`，找出所有包含该模型的账号
   - 缓存结果到 `model_accounts_cache` 并记录时间戳
   - 返回账号列表

2. **后续查询**（缓存命中）：
   - 用户再次请求 `claude-opus-4.8`
   - 查找 `model_accounts_cache` → 命中
   - 检查时间戳：`now - cached_at < TTL` → 有效
   - 直接返回缓存的账号列表（**无需遍历**）

3. **缓存失效**：
   - 时间超过 TTL（默认 300 秒）
   - 模型列表刷新时清空所有缓存
   - 账号启用/禁用状态变更时清空缓存

4. **并发控制**：
   - 使用 `RwLock` 保护缓存
   - 读取缓存时只需读锁（多个请求可并发读）
   - 更新缓存时需要写锁（短暂阻塞）

**性能优势：**
- 避免每次请求都遍历 `account_models`（O(n) → O(1)）
- 高并发场景下显著减少 CPU 消耗
- TTL 可配置，平衡实时性和性能

## 4. 核心组件实现

### 4.1 ModelService

```rust
pub struct ModelService {
    // 所有模型列表
    models: Arc<RwLock<Vec<EnrichedModel>>>,
    
    // 账号 -> 模型ID列表
    account_models: Arc<RwLock<HashMap<String, Vec<String>>>>,
    
    // 模型ID -> 支持该模型的账号列表（带缓存）
    model_accounts_cache: Arc<RwLock<HashMap<String, CachedAccountList>>>,
    
    // 最后刷新时间
    last_refresh: Arc<RwLock<Option<SystemTime>>>,
    
    // 静态元数据
    static_metadata: HashMap<String, ModelMetadata>,
    
    // 配置
    config: ModelRefreshConfig,
}
```

主要方法：
- `new()`: 初始化服务，启动时预加载
- `refresh_all_accounts()`: 刷新所有账号的模型
- `refresh_account(account_id)`: 刷新指定账号
- `get_models()`: 获取所有模型列表
- `get_account_models(account_id)`: 获取指定账号支持的模型
- `get_accounts_for_model(model_id)`: **获取支持指定模型的账号（带缓存）**
  - 首次查询：遍历 `account_models` 构建结果并缓存
  - 后续查询：直接从 `model_accounts_cache` 读取（O(1)）
  - 缓存过期：超过 TTL 后重新计算
- `clear_account_cache()`: 清空账号过滤缓存（刷新模型时调用）
- `start_auto_refresh()`: 启动自动刷新任务

### 4.2 API 调用层

复用 KiroGo 的 API 调用逻辑：

```rust
// src/kiro/api/models.rs

pub async fn list_available_models(
    account: &Credential,
) -> anyhow::Result<Vec<ModelInfo>> {
    // 调用 Kiro ListAvailableModels API
}
```

### 4.3 静态元数据文件

`src/kiro/model/static_metadata.json`:

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
    }
  ]
}
```

## 5. API 端点设计

### 5.1 后端 API

#### GET /v1/models
返回所有可用模型列表（合并后的数据）

响应示例：
```json
{
  "object": "list",
  "data": [
    {
      "id": "claude-opus-4.8",
      "object": "model",
      "created": 1779897600,
      "owned_by": "anthropic",
      "display_name": "Claude Opus 4.8",
      "model_type": "chat",
      "max_tokens": 16384,
      "context_window": 200000,
      "supports_image": true,
      "input_modalities": ["text", "image"],
      "pricing": {
        "input_per_1m": 15.0,
        "output_per_1m": 75.0
      },
      "available_accounts": ["account1", "account2"]
    }
  ]
}
```

#### POST /admin/api/models/refresh
全局刷新所有账号的模型列表

需要 Admin API Key 认证

响应示例：
```json
{
  "success": true,
  "refreshed_accounts": 5,
  "total_models": 12,
  "timestamp": "2026-06-13T10:30:00Z"
}
```

#### POST /admin/api/accounts/{id}/models/refresh
刷新指定账号的模型列表

需要 Admin API Key 认证

响应示例：
```json
{
  "success": true,
  "account_id": "account1",
  "models_count": 8,
  "timestamp": "2026-06-13T10:30:00Z"
}
```

#### GET /admin/api/accounts/{id}/models
查看指定账号支持的模型列表

需要 Admin API Key 认证

响应示例：
```json
{
  "account_id": "account1",
  "account_email": "user@example.com",
  "models": [
    {
      "model_id": "claude-opus-4.8",
      "model_name": "Claude Opus 4.8",
      "display_name": "Claude Opus 4.8"
    },
    {
      "model_id": "claude-sonnet-4.6",
      "model_name": "Claude Sonnet 4.6",
      "display_name": "Claude Sonnet 4.6"
    }
  ]
}
```

#### GET /admin/api/models/status
查看模型缓存状态

需要 Admin API Key 认证

响应示例：
```json
{
  "total_models": 12,
  "last_refresh": "2026-06-13T10:30:00Z",
  "auto_refresh_enabled": true,
  "next_refresh_in_seconds": 3600,
  "accounts": [
    {
      "account_id": "account1",
      "email": "user@example.com",
      "models_count": 8,
      "last_refresh": "2026-06-13T10:30:00Z"
    }
  ]
}
```

### 5.2 前端界面设计

在 `admin-ui/src/components/dashboard.tsx` 中添加"模型管理"选项卡：

**功能列表：**
1. 显示所有模型列表及其支持的账号数
2. 显示最后刷新时间
3. 全局刷新按钮
4. 账号列表，每个账号显示：
   - 账号邮箱
   - 支持的模型列表
   - 单独的刷新按钮
5. 刷新状态提示（加载中、成功、失败）

## 6. 请求时账号过滤

### 6.1 流程

```
用户请求 POST /v1/messages { model: "claude-opus-4.8" }
  ↓
查找支持该模型的账号（带缓存）
  ├─ 缓存命中 → 直接返回账号列表
  └─ 缓存未命中或过期 → 查询 account_models → 缓存结果
  ↓
过滤账号池（只保留支持该模型的账号）
  ↓
负载均衡选择账号
  ↓
发送请求
```

### 6.2 缓存策略

- **双层缓存结构**：
  1. `account_models`: 账号 → 模型列表（原始数据）
  2. `model_accounts_cache`: 模型 → 账号列表（**反向索引缓存**）

- **缓存逻辑**：
  ```rust
  pub async fn get_accounts_for_model(&self, model_id: &str) -> Vec<String> {
      // 1. 尝试从缓存读取
      {
          let cache = self.model_accounts_cache.read().await;
          if let Some(cached) = cache.get(model_id) {
              let elapsed = SystemTime::now()
                  .duration_since(cached.cached_at)
                  .unwrap_or_default();
              
              // TTL 未过期，直接返回
              if elapsed.as_secs() < self.config.account_filter_cache_ttl_seconds {
                  return cached.account_ids.clone();
              }
          }
      }
      
      // 2. 缓存未命中或过期，重新计算
      let account_models = self.account_models.read().await;
      let mut result = Vec::new();
      
      for (account_id, models) in account_models.iter() {
          if models.contains(&model_id.to_string()) {
              result.push(account_id.clone());
          }
      }
      
      // 3. 更新缓存
      {
          let mut cache = self.model_accounts_cache.write().await;
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

- **缓存失效时机**：
  1. TTL 过期（默认 300 秒，可配置）
  2. 模型列表刷新时清空所有缓存（`clear_account_cache()`）
  3. 账号状态变更时清空相关缓存

- **性能优化**：
  - 读操作只需读锁，支持高并发
  - 避免每次请求都遍历 `account_models`（从 O(n×m) 降到 O(1)）
  - 热门模型的账号列表被频繁命中，响应时间显著降低

### 6.3 实现位置

在 `src/anthropic/handlers.rs` 的 `create_messages` 中：

```rust
// 获取支持该模型的账号列表
let available_accounts = model_service
    .get_accounts_for_model(&request.model)
    .await?;

if available_accounts.is_empty() {
    return Err(ErrorResponse::new(
        "invalid_request_error",
        format!("No account supports model: {}", request.model),
    ));
}

// 过滤账号池
let filtered_pool = account_pool
    .iter()
    .filter(|acc| available_accounts.contains(&acc.id))
    .collect::<Vec<_>>();
```

## 7. 错误处理

### 7.1 API 调用失败
- 刷新失败时保留旧缓存
- 记录错误日志
- 返回错误信息给前端

### 7.2 账号无可用模型
- 返回明确的错误消息
- 建议用户检查账号状态或选择其他模型

### 7.3 配置文件缺失
- 使用默认配置
- 记录警告日志

## 8. 测试计划

### 8.1 单元测试
- 模型合并逻辑测试
- 账号过滤逻辑测试
- 缓存 TTL 测试

### 8.2 集成测试
- API 调用测试
- 刷新流程测试
- 并发访问测试

### 8.3 手动测试
- 启动时预加载验证
- 后台界面刷新验证
- 请求时账号过滤验证
- 缓存过期验证

## 9. 实现步骤

1. **创建数据结构**
   - ModelInfo
   - ModelMetadata
   - EnrichedModel
   - 配置结构

2. **实现 ModelService 核心**
   - 初始化和预加载
   - 刷新逻辑
   - 缓存管理
   - 账号过滤

3. **添加 API 端点**
   - 后台管理 API
   - 修改 /v1/models

4. **集成到请求处理**
   - 修改 create_messages
   - 添加账号过滤

5. **前端界面**
   - 模型管理选项卡
   - 刷新按钮
   - 状态显示

6. **测试和优化**
   - 单元测试
   - 集成测试
   - 性能优化

## 10. 注意事项

- 使用 `Arc<RwLock<>>` 确保线程安全
- 刷新失败时不影响现有缓存
- 账号过滤缓存避免每次请求查询
- 前端刷新时显示加载状态
- 日志记录所有关键操作
- 配置项提供合理的默认值
