# 安全政策

幻述非常重视安全问题。

## 报告漏洞

如果你发现了一个安全漏洞，请通过以下方式报告：

**不要**在 GitHub Issues 中公开提报安全漏洞。

请发送邮件至项目维护者，或通过 GitHub Security Advisories 页面提交：

https://github.com/billowsand/huanshu/security/advisories/new

## 预期响应时间

- 首次响应：3 个工作日内
- 修复方案确认：7 个工作日内
- 安全更新发布：问题确认后尽快发布

## 支持的版本

| 版本 | 支持状态 |
|------|----------|
| 0.1.x | 🚧 正在接收安全报告 |

## 已知的限制

- 加密功能使用 AES-256-GCM + Argon2id，密码强度由用户决定
- LM Studio 通信不强制 TLS，请在可信网络中使用
