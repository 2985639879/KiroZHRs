//! 公共认证工具函数

use axum::{
    body::Body,
    http::{Request, header},
};
use subtle::ConstantTimeEq;

/// 从请求中提取 API Key（支持明文或 MD5 哈希）
///
/// 支持两种认证方式：
/// - `x-api-key` header
/// - `Authorization: Bearer <token>` header
pub fn extract_api_key(request: &Request<Body>) -> Option<String> {
    // 优先检查 x-api-key
    if let Some(key) = request
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
    {
        return Some(key.to_string());
    }

    // 其次检查 Authorization: Bearer
    request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.to_string())
}

/// 验证 API Key（支持明文或 MD5 哈希）
///
/// - 如果客户端发送的是 32 字符的十六进制字符串，则视为 MD5 哈希
/// - 否则视为明文，与服务端配置的明文比对
/// - 所有比对均使用常量时间算法防止时序攻击
pub fn verify_api_key(client_key: &str, server_key: &str) -> bool {
    // 检查客户端发送的是否为 MD5 哈希（32 个十六进制字符）
    if client_key.len() == 32 && client_key.chars().all(|c| c.is_ascii_hexdigit()) {
        // 客户端发送的是 MD5 哈希，计算服务端密钥的 MD5 进行比对
        let server_key_md5 = format!("{:x}", md5::compute(server_key.as_bytes()));
        constant_time_eq(client_key, &server_key_md5)
    } else {
        // 客户端发送的是明文，直接比对
        constant_time_eq(client_key, server_key)
    }
}

/// 常量时间字符串比较，防止时序攻击
///
/// 无论字符串内容如何，比较所需的时间都是恒定的，
/// 这可以防止攻击者通过测量响应时间来猜测 API Key。
///
/// 使用经过安全审计的 `subtle` crate 实现
pub fn constant_time_eq(a: &str, b: &str) -> bool {
    a.as_bytes().ct_eq(b.as_bytes()).into()
}
