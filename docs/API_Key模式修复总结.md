# API Key 模式修复总结

## 问题诊断

用户报告 API Key 账号 51 获取模型列表失败：
```
403 Forbidden: {"message":"Your subscription does not support this application. Please contact your administrator.","reason":null}
```

## 根本原因

初始实现使用**硬编码的 us-east-1 区域**：
```rust
const KIRO_API_BASE: &str = "https://codewhisperer.us-east-1.amazonaws.com";
```

如果 API Key 账号配置在其他区域（如 eu-central-1, ap-southeast-1），向 us-east-1 发送请求会被 AWS 拒绝，返回 403 错误。

## 解决方案

参考 Kiro-Go 的实现（`proxy/kiro_api.go`），添加**动态区域解析**：

### 1. API Key 账号区域解析
使用 `effective_api_region()` 方法，优先级链：
```
凭据.apiRegion > 凭据.region > config.apiRegion > config.region > "us-east-1"
```

### 2. OAuth 账号区域解析
从 ProfileArn 提取区域：
```rust
// ProfileArn 格式: arn:aws:codewhisperer:{region}:...
arn.split(':').nth(3)
```

### 3. 区域化 URL 构建
- `us-east-1` → `https://codewhisperer.us-east-1.amazonaws.com`
- 其他区域 → `https://q.{region}.amazonaws.com`

## 修改的文件

1. **src/kiro/api/models.rs**
   - 添加 `config` 参数以访问区域配置
   - 实现动态区域解析逻辑
   - 构建区域化的 API URL

2. **src/kiro/model_service.rs**
   - 更新两处 `list_available_models` 调用，传入 `config`

## 配置方法

### 方案 1：凭据级别配置（推荐）
为每个 API Key 单独指定区域：

```json
{
  "kiroApiKey": "your-api-key",
  "authMethod": "api_key",
  "apiRegion": "eu-central-1"
}
```

### 方案 2：全局配置
在 `config.json` 中设置默认区域（适用于所有未指定区域的凭据）：

```json
{
  "apiRegion": "ap-southeast-1"
}
```

## 常见区域

| 区域代码 | 地理位置 | API URL |
|---------|---------|---------|
| `us-east-1` | 美国东部（弗吉尼亚） | https://codewhisperer.us-east-1.amazonaws.com |
| `us-west-2` | 美国西部（俄勒冈） | https://q.us-west-2.amazonaws.com |
| `eu-central-1` | 欧洲（法兰克福） | https://q.eu-central-1.amazonaws.com |
| `eu-west-1` | 欧洲（爱尔兰） | https://q.eu-west-1.amazonaws.com |
| `ap-southeast-1` | 亚太（新加坡） | https://q.ap-southeast-1.amazonaws.com |
| `ap-northeast-1` | 亚太（东京） | https://q.ap-northeast-1.amazonaws.com |

## 诊断方法

### 1. 查看日志确定问题
如果看到此错误：
```
Your subscription does not support this application
```
**可能原因：**
- 区域配置错误（最常见）
- API Key 无效
- API Key 没有 ListAvailableModels 权限

### 2. 检查凭据配置
```bash
cat config/credentials.json | jq '.[] | select(.id==51)'
```
查看账号 51 是否配置了 `apiRegion` 或 `region` 字段。

### 3. 测试不同区域
在凭据中逐个尝试常见区域：
```json
{"apiRegion": "us-east-1"}    // 美东
{"apiRegion": "us-west-2"}    // 美西
{"apiRegion": "eu-central-1"} // 欧洲
{"apiRegion": "ap-southeast-1"} // 亚太
```

### 4. 查看请求日志
启用 TRACE 级别日志查看实际请求的 URL：
```bash
RUST_LOG=trace cargo run -- -c config/config.json
```

## 兼容性

### ✅ 向后兼容
- 未配置区域的凭据自动使用 `us-east-1`（与修复前行为一致）
- OAuth 账号继续从 ProfileArn 提取区域
- 无需修改现有配置文件即可运行

### ✅ 新功能
- 支持在凭据或全局配置中指定区域
- 支持混合使用不同区域的 API Key
- 支持所有 AWS CodeWhisperer 支持的区域

## 预期结果

修复后，正确配置区域的 API Key 账号应该能够成功获取模型列表：

```
INFO kiro_rs::kiro::model_service: Account 51 fetched 12 models from ListAvailableModels API
INFO kiro_rs::kiro::model_service: Refreshed 12 models for account: 51
```

## 参考文档

- [API_Key区域配置修复.md](./API_Key区域配置修复.md) - 详细技术说明
- [API_Key模式改进说明.md](./API_Key模式改进说明.md) - 完整功能改进
- Kiro-Go 参考实现：`F:\Kiro-Go-main\proxy\kiro_api.go:238-272`

## 下一步

1. 在 `credentials.json` 中为账号 51 添加 `apiRegion` 字段
2. 重启服务
3. 观察日志确认模型列表获取成功
4. 如果仍然失败，尝试其他区域或检查 API Key 权限
