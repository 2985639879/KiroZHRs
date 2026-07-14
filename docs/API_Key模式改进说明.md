# API Key 模式改进说明

## 概述

本次修改参考 [Kiro-Go PR #137](https://github.com/Quorinex/Kiro-Go/pull/137)，改进了 API Key 认证模式的实现，使其能够正确调用 AWS ListAvailableModels API 获取真实的模型列表。

## 修改内容

### 1. ProfileArn 解析跳过（`src/kiro/api/models.rs`）

**位置：** `list_available_models` 函数

**修改前：**
- 所有凭据类型都会尝试附加 ProfileArn 参数

**修改后：**
```rust
// API Key 账号跳过 ProfileArn（参考 Kiro-Go ResolveProfileArn 逻辑）
let profile_arn = if credential.is_api_key_credential() {
    String::new()
} else {
    credential
        .profile_arn
        .as_ref()
        .map(|s| format!("&profileArn={}", urlencoding::encode(s)))
        .unwrap_or_default()
};
```

**原因：** API Key 账号不需要 ProfileArn，这是 AWS 的设计。只有 OAuth 账号需要 ProfileArn 来标识用户身份。

### 1.5. 区域配置支持（`src/kiro/api/models.rs`）

**关键修改：** 动态区域解析，支持非 us-east-1 区域

**修改前：**
```rust
const KIRO_API_BASE: &str = "https://codewhisperer.us-east-1.amazonaws.com";
let url = format!("{}/ListAvailableModels?...", KIRO_API_BASE);
```

**修改后：**
```rust
// 根据账号类型动态确定区域
let region = if credential.is_api_key_credential() {
    credential.effective_api_region(config)  // API Key: 使用配置的区域
} else {
    // OAuth: 从 ProfileArn 提取区域
    extract_region_from_profile_arn(credential.profile_arn)
};

// 构建区域化 URL
let api_base = if region == "us-east-1" {
    "https://codewhisperer.us-east-1.amazonaws.com"
} else {
    format!("https://q.{}.amazonaws.com", region)
};
```

**原因：** 
- AWS API Key 可以配置在不同区域（eu-central-1, ap-southeast-1 等）
- 硬编码 us-east-1 会导致非美东区域的 API Key 返回 403 错误
- 参考 Kiro-Go 的 `kiroRegionForProfile` 和 `EffectiveApiRegion` 实现

**配置示例：**
```json
{
  "kiroApiKey": "xxx",
  "authMethod": "api_key",
  "apiRegion": "eu-central-1"  // 指定欧洲区域
}
```

**详细说明：** 参见 `docs/API_Key区域配置修复.md`

### 2. ListAvailableModels API 调用（`src/kiro/model_service.rs`）

**位置：** `refresh_account_models` 函数

**修改前：**
- API Key 账号使用硬编码的默认模型列表
- 跳过 ListAvailableModels API 调用

**修改后：**
- 所有账号（包括 API Key）统一调用 ListAvailableModels API
- API Key 账号如果调用失败，直接标记失败
- 不再使用硬编码的默认模型列表

**原因：** 
- API Key 账号应该调用真实 API 获取模型权限
- 如果订阅不支持该 API，应该让用户知道（通过失败状态）
- 不应该使用默认列表掩盖权限问题

### 3. Token 刷新跳过（`src/kiro/model_service.rs`）

**位置：** `refresh_account_models` 函数的错误处理

**修改前：**
- 所有 403 错误都尝试刷新 token

**修改后：**
```rust
// API Key 账号不支持 token 刷新，直接返回错误
if credentials.is_api_key_credential() {
    tracing::warn!(
        "Account {} (API Key) failed to fetch models: {}",
        account_id,
        e
    );
    return Err(e);
}

// OAuth 账号：检查是否是认证错误（403），如果是则尝试刷新 token
```

**原因：** API Key 是静态的，不需要也不支持刷新。尝试刷新会产生无意义的错误日志。

### 4. Provider 中的认证错误处理（`src/kiro/provider.rs`）

**位置：** `call_mcp` 和 `call_api` 函数的 401/403 错误处理

**修改：** 两处都添加了 API Key 检查
```rust
// API Key 凭据不支持 token 刷新，直接标记失败
if ctx.credentials.is_api_key_credential() {
    tracing::warn!(
        "凭据 #{} (API Key) 认证失败: {} {}",
        ctx.id,
        status,
        body
    );
    let has_available = self.token_manager.report_failure(ctx.id);
    if !has_available {
        anyhow::bail!("请求失败（所有凭据已用尽）: {} {}", status, body);
    }
    last_error = Some(anyhow::anyhow!("请求失败: {} {}", status, body));
    continue;
}

// OAuth 凭据：token 被上游失效时，尝试 force-refresh
```

**原因：** 
- 避免对 API Key 凭据进行无意义的刷新尝试
- 提供更清晰的错误日志
- 立即切换到下一个可用凭据，提高效率

## API Key vs OAuth 认证流程对比

### OAuth 流程
1. 使用 RefreshToken 刷新 AccessToken
2. 使用 AccessToken 调用 ListAvailableProfiles 获取 ProfileArn
3. 使用 AccessToken + ProfileArn 调用 ListAvailableModels
4. Token 过期时可以刷新

### API Key 流程
1. ~~跳过 Token 刷新~~ ✅（已有保护）
2. ~~跳过 ProfileArn 解析~~ ✅（本次修改）
3. 直接使用 API Key 作为 Bearer Token，添加 `tokentype: API_KEY` 头
4. 调用 ListAvailableModels API ✅（本次修改）
   - 成功：使用 API 返回的模型列表
   - 失败：标记账号为失败状态
5. ~~Token 过期时不刷新~~ ✅（本次修改）

## 请求头差异

### OAuth 请求头
```
Authorization: Bearer {AccessToken}
```

### API Key 请求头
```
Authorization: Bearer {KiroApiKey}
tokentype: API_KEY
```

关键区别：`tokentype: API_KEY` 头告诉 AWS 服务器使用不同的认证方式验证请求。

## 已删除的代码

- `refresh_api_key_account_with_defaults` 函数已删除
- 不再使用硬编码的默认模型列表
- API Key 账号必须成功调用 ListAvailableModels API

## 测试建议

1. 添加支持 ListAvailableModels 的 API Key 凭据
2. 调用 `/admin/models` API 查看该账号的模型列表
3. 验证返回的模型列表是否来自 AWS API
4. 故意使用无效的 API Key，验证错误处理逻辑（不应尝试刷新）
5. 如果 API Key 订阅不支持 ListAvailableModels，账号状态应为失败
6. 在 MCP/Anthropic API 调用中使用 API Key 凭据，验证请求头正确

## 兼容性说明

本次修改向后兼容：
- 不影响 OAuth 账号的行为
- 不影响现有配置文件格式
- 仅改进 API Key 账号的模型获取方式

## 参考

- [Kiro-Go PR #137](https://github.com/Quorinex/Kiro-Go/pull/137) - API Key 认证模式实现
- `F:\Kiro-Go-main\proxy\kiro_api.go:238-285` - ListAvailableModels 实现
- `F:\Kiro-Go-main\proxy\kiro_api.go:277-285` - ResolveProfileArn 跳过 API Key
- `F:\Kiro-Go-main\proxy\kiro_headers.go:75-99` - 请求头设置逻辑
