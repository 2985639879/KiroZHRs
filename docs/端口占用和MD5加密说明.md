# 端口占用和 API Key 安全传输说明

## 1. 端口占用问题修复

### 问题描述
之前当端口被占用时，程序会直接 panic 并显示不友好的错误信息：
```
thread 'main' panicked at src/main.rs:284:63:
called `Result::unwrap()` on an `Err` value: Os { code: 98, kind: AddrInUse, message: "Address already in use" }
```

### 修复后的行为
现在程序会优雅地处理端口占用错误，并提供详细的排查步骤：

```
无法绑定端口 0.0.0.0:8080: Address already in use (os error 98)
端口 8080 已被占用，请检查：
  1. 是否有其他 Kiro 实例正在运行
  2. 使用 'lsof -i :8080' (Linux/Mac) 或 'netstat -ano | findstr 8080' (Windows) 查看占用进程
  3. 修改配置文件中的 listenAddr 使用其他端口
```

### 如何修改端口
编辑 `config/config.json`：
```json
{
  "host": "0.0.0.0",
  "port": 8081,  // 改成其他可用端口
  ...
}
```

## 2. API Key MD5 加密传输

### 安全改进
为了提高安全性，前端现在使用 MD5 哈希传输 API Key，而不是明文传输。

### 工作原理

#### 前端
1. 用户输入的 API Key 存储在 localStorage（明文）
2. 发送请求时，自动计算 API Key 的 MD5 哈希
3. 在 HTTP header 中发送哈希值而不是明文

```typescript
// 示例：API Key "sk-kiro-rs-test123" 
// 发送的是其 MD5 哈希: "f8e7d6c5b4a3..."
config.headers['x-api-key'] = md5(apiKey)
```

#### 后端
1. 接收客户端发送的值
2. 自动识别是 MD5 哈希（32 个十六进制字符）还是明文
3. 如果是哈希，计算服务端配置的 API Key 的 MD5 进行比对
4. 如果是明文，直接比对（向后兼容）
5. 所有比对使用常量时间算法，防止时序攻击

### 兼容性
- ✅ **新前端 + 新后端**：使用 MD5 哈希传输（推荐）
- ✅ **旧客户端 + 新后端**：继续使用明文（向后兼容）
- ⚠️ **新前端 + 旧后端**：不兼容（需要升级后端）

### 测试验证

#### 测试 MD5 哈希传输
```bash
# 假设 API Key 是 "sk-kiro-rs-test123"
# 其 MD5 哈希为 "f8e7d6c5b4a3..." (实际计算的值)

# 使用 MD5 哈希访问
curl -X GET http://localhost:8080/api/admin/credentials \
  -H "x-api-key: $(echo -n 'sk-kiro-rs-test123' | md5sum | cut -d' ' -f1)"

# 或者直接使用明文（向后兼容）
curl -X GET http://localhost:8080/api/admin/credentials \
  -H "x-api-key: sk-kiro-rs-test123"
```

### 注意事项

1. **MD5 不是加密**
   - MD5 是哈希算法，不可逆
   - 不能从哈希值反推出原始 API Key
   - 主要防止网络传输中的明文泄露

2. **完整安全方案还需要**
   - 使用 HTTPS 加密传输（防止中间人攻击）
   - API Key 定期轮换
   - 访问日志监控

3. **localStorage 安全性**
   - API Key 在浏览器 localStorage 中仍是明文存储
   - 只有传输过程使用 MD5
   - 不要在不受信任的设备上保存 API Key

## 3. 升级步骤

### 后端升级
```bash
cd /path/to/kiro.rs
git pull
cargo build --release
```

### 前端升级
```bash
cd /path/to/kiro.rs/admin-ui
git pull
pnpm install
pnpm build
```

### 运行
```bash
# 使用配置文件启动
cargo run -- -c config/config.json --credentials config/credentials.json

# 或者使用 release 版本
./target/release/kiro-rs -c config/config.json
```

## 4. 常见问题

### Q: 为什么不使用 SHA-256？
A: MD5 对于 API Key 传输场景已足够，且计算速度更快。完整的安全性应该由 HTTPS 保证。

### Q: 旧客户端还能用吗？
A: 可以，后端会自动识别并兼容明文传输。

### Q: 如何强制只接受 MD5 哈希？
A: 目前暂不支持，保持向后兼容性是设计目标。
