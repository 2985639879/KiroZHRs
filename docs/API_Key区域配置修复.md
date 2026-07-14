# API Key 区域配置修复

## 问题描述

初始实现中，`list_available_models` 函数使用硬编码的 `us-east-1` 区域：

```rust
const KIRO_API_BASE: &str = "https://codewhisperer.us-east-1.amazonaws.com";
```

这导致非 `us-east-1` 区域的 API Key 账号调用 ListAvailableModels 时返回 403 错误：

```
ListAvailableModels failed with status 403 Forbidden: 
{
  "message": "Your subscription does not support this application. Please contact your administrator.",
  "reason": null
}
```

## 根本原因

AWS 的 API Key 账号可能配置在不同的区域（us-east-1, eu-central-1, ap-southeast-1 等）。如果向错误的区域发送请求，AWS 会返回 403 错误，提示订阅不支持该应用。

## Kiro-Go 的实现

Kiro-Go 使用 **动态区域解析**（参考 `proxy/kiro_api.go:238-272`）：

### 核心逻辑

```go
// kiroRegionForProfile 函数
func kiroRegionForProfile(account *config.Account, profileArn string) string {
    // API Key 账号使用 EffectiveApiRegion
    if account != nil && account.IsApiKeyCredential() {
        return account.EffectiveApiRegion()  // 👈 关键
    }
    // OAuth 账号从 ProfileArn 提取区域
    if r := regionFromProfileArn(profileArn); r != "" {
        return r
    }
    return "us-east-1"
}

// EffectiveApiRegion 优先级
func (a *Account) EffectiveApiRegion() string {
    if a.ApiRegion != "" { return a.ApiRegion }        // 1. 凭据的 ApiRegion
    if a.Region != "" { return a.Region }              // 2. 凭据的 Region
    if globalApiRegion != "" { return globalApiRegion } // 3. 全局 ApiRegion
    if globalRegion != "" { return globalRegion }       // 4. 全局 Region
    return "us-east-1"                                 // 5. 默认 us-east-1
}
```

### 区域化 URL

```go
// 根据区域构建不同的主机名
func regionalizeURLForRegion(rawURL string, region string) string {
    if region == "us-east-1" {
        return rawURL  // 保持原样
    }
    // 非 us-east-1：替换主机名
    regionalHost := "q." + region + ".amazonaws.com"
    return strings.Replace(rawURL, "codewhisperer.us-east-1.amazonaws.com", regionalHost)
}
```

**示例：**
- `us-east-1` → `https://codewhisperer.us-east-1.amazonaws.com/ListAvailableModels`
- `eu-central-1` → `https://q.eu-central-1.amazonaws.com/ListAvailableModels`
- `ap-southeast-1` → `https://q.ap-southeast-1.amazonaws.com/ListAvailableModels`

## Rust 实现修复

### 修改文件
`src/kiro/api/models.rs`

### 关键代码

```rust
// 1. 根据账号类型动态确定区域
let region = if credential.is_api_key_credential() {
    // API Key：使用 EffectiveApiRegion
    credential.effective_api_region(config)
} else {
    // OAuth：从 ProfileArn 提取区域，回退到 Region
    credential
        .profile_arn
        .as_ref()
        .and_then(|arn| {
            // ProfileArn 格式: arn:aws:codewhisperer:{region}:...
            arn.split(':').nth(3).filter(|r| !r.is_empty())
        })
        .unwrap_or_else(|| {
            credential
                .region
                .as_deref()
                .unwrap_or(config.effective_api_region())
        })
};

// 2. 构建区域化的 API Base URL
let api_base = if region == "us-east-1" {
    KIRO_API_BASE.to_string()  // https://codewhisperer.us-east-1.amazonaws.com
} else {
    format!("https://q.{}.amazonaws.com", region)
};

// 3. 拼接完整 URL
let url = format!(
    "{}/ListAvailableModels?origin=AI_EDITOR&maxResults=50{}",
    api_base, profile_arn
);
```

### 修改点

1. **添加 `config` 参数**
   - 函数签名：`list_available_models(credential, config, proxy_config, tls_backend)`
   - 用于访问全局区域配置

2. **动态区域解析**
   - API Key 账号：`credential.effective_api_region(config)`
   - OAuth 账号：从 `ProfileArn` 提取，回退到 `Region`

3. **区域化 URL 构建**
   - `us-east-1` 使用常量 `KIRO_API_BASE`
   - 其他区域使用 `q.{region}.amazonaws.com`

4. **更新调用点**
   - `model_service.rs:103` - 首次调用
   - `model_service.rs:149` - 刷新后重试

## 配置说明

### 凭据级别配置

在 `credentials.json` 中为每个 API Key 账号配置区域：

```json
{
  "kiroApiKey": "your-api-key",
  "authMethod": "api_key",
  "apiRegion": "eu-central-1",     // 优先级最高
  "region": "us-west-2"             // 回退值
}
```

### 全局配置

在 `config.json` 中配置全局默认区域：

```json
{
  "apiRegion": "us-east-1",         // API 调用区域
  "region": "us-east-1"             // 通用区域
}
```

### 优先级链

对于 API Key 账号：

```
凭据.apiRegion > 凭据.region > config.apiRegion > config.region > "us-east-1"
```

## 测试验证

### 测试用例 1: us-east-1 API Key
```json
{
  "kiroApiKey": "xxx",
  "authMethod": "api_key"
}
```
**预期 URL:** `https://codewhisperer.us-east-1.amazonaws.com/ListAvailableModels?origin=AI_EDITOR&maxResults=50`

### 测试用例 2: eu-central-1 API Key
```json
{
  "kiroApiKey": "xxx",
  "authMethod": "api_key",
  "apiRegion": "eu-central-1"
}
```
**预期 URL:** `https://q.eu-central-1.amazonaws.com/ListAvailableModels?origin=AI_EDITOR&maxResults=50`

### 测试用例 3: ap-southeast-1 API Key
```json
{
  "kiroApiKey": "xxx",
  "authMethod": "api_key",
  "region": "ap-southeast-1"
}
```
**预期 URL:** `https://q.ap-southeast-1.amazonaws.com/ListAvailableModels?origin=AI_EDITOR&maxResults=50`

## 影响范围

### 修复的场景
- ✅ 非 us-east-1 区域的 API Key 账号现在可以正常获取模型列表
- ✅ 多区域部署的系统可以混合使用不同区域的 API Key
- ✅ OAuth 账号继续从 ProfileArn 提取正确的区域

### 兼容性
- ✅ 向后兼容：未配置区域的账号默认使用 `us-east-1`
- ✅ OAuth 账号行为不变：继续从 ProfileArn 提取区域
- ✅ 不需要修改现有配置文件（除非要使用非 us-east-1 区域）

## 相关文件

- `src/kiro/api/models.rs:12-80` - ListAvailableModels 实现
- `src/kiro/model/credentials.rs:212-215` - effective_api_region 方法
- `src/kiro/model_service.rs:103,149` - 调用点
- `src/kiro/token_manager.rs:679-681` - config() getter

## 参考

- Kiro-Go PR #137: https://github.com/Quorinex/Kiro-Go/pull/137
- Kiro-Go 实现：`F:\Kiro-Go-main\proxy\kiro_api.go:238-272`
- AWS CodeWhisperer 区域文档：https://docs.aws.amazon.com/codewhisperer/latest/userguide/regions.html

## 历史

- **2026-07-14**: 初版 - 使用硬编码 us-east-1，导致非 us-east-1 API Key 失败
- **2026-07-14**: 修复 - 实现动态区域解析，与 Kiro-Go 行为对齐
