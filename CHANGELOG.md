# 更新日志

## [2026.3.3] - 2026-07-14

### 修复

#### 1. API Key 模式模型获取修复（对齐 Kiro-Go）
- **问题描述**: API Key 账号调用 `ListAvailableModels` 时返回 403 错误（"The bearer token included in the request is invalid" / "Your subscription does not support this application"）
- **根本原因**: 请求缺少 AWS SDK 要求的 User-Agent 相关 headers，且区域选择逻辑不完整
- **修复内容**:
  - 补全 `User-Agent`、`x-amz-user-agent`、`x-amzn-codewhisperer-optout` headers，完全对齐 Kiro-Go 的 `buildRuntimeHeaderValues`
  - 使用小写 `tokentype: API_KEY` header（与 Kiro-Go net/http 规范一致）
  - 修复 `effective_api_region()` 优先级链：`ApiRegion > Region > GlobalApiRegion > GlobalRegion > us-east-1`，此前遗漏了账号级 `Region` 字段
- **用户价值**: API Key 账号可正常获取模型列表并发起请求

#### 2. API Key 凭据不再尝试 Token 刷新
- **问题描述**: API Key 凭据本身不支持 OAuth 刷新流程，此前 401/403 时会错误地尝试 force-refresh
- **修复内容**: `provider.rs` 中区分 API Key 与 OAuth 凭据，API Key 认证失败直接标记失败并切换到下一个可用凭据
- **用户价值**: 避免无意义的刷新尝试，加快失败切换速度

### 新增功能

#### 3. 凭据区域配置的编辑支持
- **功能描述**: Admin UI 支持查看和编辑凭据的 `region`/`authRegion`/`apiRegion` 字段
- **实现细节**:
  - `CredentialEntrySnapshot` 新增区域字段用于前端展示
  - `PUT /admin/credentials/:id` 支持更新区域配置
  - 编辑对话框自动回填现有区域配置
  - 新增"一键获取区域"功能，从 AWS SSO Portal URL 自动提取区域
- **用户价值**: 无需手动编辑配置文件即可调整账号区域

#### 4. 端口占用错误处理与 MD5 加密传输
- **功能描述**: 启动时端口占用给出友好错误提示；Admin API Key 以 MD5 哈希方式传输
- **实现细节**:
  - 前端 axios 拦截器自动为请求附加 `x-api-key = md5(apiKey)` header
  - 端口占用时输出清晰的错误信息

### 技术改进

- API Key 模式的请求构建完全对齐 Kiro-Go 参考实现（headers、区域、URL 构建、profileArn 处理）
- 新增详细的调试日志，便于诊断 API Key 认证问题

### 文档

- 新增 `docs/API_Key模式改进说明.md`
- 新增 `docs/API_Key区域配置修复.md`
- 新增 `docs/API_Key模式修复总结.md`
- 新增 `docs/区域配置和编辑功能改进.md`
- 新增 `docs/端口占用和MD5加密说明.md`

---

## [2026.3.2] - 2026-06-13

### 新增功能

#### 1. 智能账号过滤 (Task 1)
- **功能描述**: 在发送 API 请求前，基于请求的模型参数自动过滤出支持该模型的账号
- **实现细节**:
  - `ModelService` 提供 `get_accounts_for_model` 方法，根据模型ID返回支持该模型的账号列表
  - 账号过滤结果带缓存（TTL可配置），减少重复计算
  - `MultiTokenManager` 的 `select_next_credential` 方法集成模型过滤逻辑
  - 过滤优先级：ModelService 账号列表 > 基于订阅等级的 Opus 过滤 > 默认选择
- **用户价值**: 自动将请求路由到支持目标模型的账号，避免因账号不支持模型而导致的请求失败

#### 2. 账号刷新失败处理 (Task 2)
- **功能描述**: 当账号 Token 刷新失败时，自动标记该账号并尝试切换到下一个可用账号
- **实现细节**:
  - `acquire_context` 方法捕获刷新失败异常，区分永久失效（RefreshTokenInvalidError）和临时失败
  - 刷新失败累计到 `refresh_failure_count`，达到阈值后自动禁用账号
  - 永久失效的账号立即禁用，避免浪费重试次数
- **用户价值**: 提高服务可用性，单个账号失效不影响整体服务

#### 3. 请求失败时的账号切换 (Task 3)
- **功能描述**: API 请求失败时自动切换到下一个可用账号，实现智能重试和容错
- **实现细节**:
  - `call_api_with_retry` 方法实现完整的错误分类和处理逻辑
  - 401/403 错误：Token 疑似失效时尝试强制刷新（每账号仅一次），刷新失败则切换账号
  - 402 错误：额度用尽时标记账号并切换
  - 429/408/5xx 错误：瞬态错误，重试但不禁用账号
  - 400 错误：客户端请求错误，不重试
  - 重试策略：单凭据最多重试 3 次，总重试次数上限 9 次
- **用户价值**: 大幅提升请求成功率，自动处理各种失败场景

#### 4. 模型列表展示组件 (Task 4)
- **功能描述**: Web 管理界面新增模型管理标签页，展示所有可用模型及其详细信息
- **实现细节**:
  - 新增 `models-page.tsx` 组件，使用卡片式布局展示模型信息
  - 显示模型属性：名称、ID、类型、Token 限制、上下文窗口、定价、支持账号等
  - 支持 Vision 模型标识和输入模态性显示
  - 加载和错误状态的友好提示
- **用户价值**: 直观了解所有账号的可用模型和能力

#### 5. 模型刷新功能 (Task 5)
- **功能描述**: 支持手动刷新所有账号或指定账号的模型列表
- **实现细节**:
  - 后端 API：`POST /api/admin/models/refresh` 刷新所有账号
  - 后端 API：`POST /api/admin/models/refresh/:account_id` 刷新指定账号
  - 前端提供"Refresh All"按钮，显示刷新进度和结果
  - 刷新完成后自动重新加载模型列表
- **用户价值**: 及时获取最新的模型可用性信息

#### 6. 模型管理集成到主界面 (Task 6)
- **功能描述**: 将模型管理功能集成到 Admin UI 的标签页系统
- **实现细节**:
  - Dashboard 组件新增 Tabs 布局
  - 两个主标签页：Credentials（凭据管理）和 Models（模型管理）
  - 标签页切换不刷新页面，保持状态
  - 新增 `@radix-ui/react-tabs` UI 组件
- **用户价值**: 统一的管理界面，方便在凭据和模型管理之间切换

### 技术改进

- **代码架构**: 
  - `MultiTokenManager` 新增 `model_service` 字段，支持运行时设置 ModelService
  - `ModelService` 的 `get_accounts_for_model` 方法实现账号过滤缓存
  - 前端 API 层新增 `models.ts`，封装模型相关接口

- **类型安全**:
  - 新增 TypeScript 类型定义：`ModelInfo`, `ModelsResponse`, `RefreshResponse`

### 待办事项

以下功能已准备就绪但需要完成的步骤：

1. **前端依赖安装** (Task 7):
   ```bash
   cd admin-ui && pnpm add @radix-ui/react-tabs
   ```

2. **前端构建**:
   ```bash
   cd admin-ui && pnpm build
   ```

3. **单元测试** (Task 8):
   - 为 `ModelService::get_accounts_for_model` 编写测试
   - 为 `MultiTokenManager::select_next_credential` 的模型过滤逻辑编写测试

4. **性能优化** (Task 9):
   - 考虑增加模型数据的持久化缓存
   - 优化刷新策略，支持增量刷新

### 配置说明

新增配置项（可选）：

```json
{
  "model_refresh": {
    "enabled": true,
    "interval_seconds": 3600,
    "account_filter_cache_ttl_seconds": 300
  }
}
```

- `enabled`: 是否启用后台自动刷新模型列表
- `interval_seconds`: 刷新间隔（秒）
- `account_filter_cache_ttl_seconds`: 账号过滤缓存有效期（秒）

### API 端点

新增 Admin API 端点：

- `GET /api/admin/models` - 获取所有模型列表
- `POST /api/admin/models/refresh` - 刷新所有账号的模型列表
- `POST /api/admin/models/refresh/:account_id` - 刷新指定账号的模型列表

### 破坏性变更

无

### 已知问题

无

---

## 如何升级

1. 备份现有配置文件
2. 拉取最新代码
3. 安装前端依赖：`cd admin-ui && pnpm install`
4. 构建前端：`pnpm build`
5. 编译后端：`cargo build --release`
6. 启动服务

## 贡献者

- AI Assistant (Claude)
