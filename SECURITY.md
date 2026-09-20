# 安全策略（Security Policy）

## 支持版本

FluxTorrent 处于积极开发期（pre-1.0），仅对 master 最新提交提供安全修复。

## 报告漏洞

**请勿通过公开 issue 报告安全漏洞。**

请通过 GitHub Security Advisories（仓库 Security 标签页 → Report a vulnerability）
提交报告，或联系维护者邮箱（见仓库所有者资料）。请包含：

- 问题类型（如 SQL 注入 / 越权 / 资金逻辑）
- 复现步骤或 PoC
- 影响评估（涉及资金动账/凭据泄漏的会被优先处理）

我们会在 **72 小时内**确认收到，修复进度会随报告同步。修复发布前请勿公开细节。

## 报告范围外的内容

- 未设置 `TRUST_PROXY=1` 导致的 XFF 伪造（部署配置问题，见 .env.example 说明）
- 站点管理员主动配置造成的风险（如游戏 EV 参数——读取端已有 EV<1 强制校验）
- DEMO 密码（0018 演示账号口令——生产态启动已自动随机化，见 main.rs 防线）

## 部署侧安全基线（站长自查）

- `docker/.env` 三密钥（DB/REDIS/JWT）必须强随机；CORS_ORIGINS 生产必填
- api/web 仅绑 127.0.0.1，公网流量经 TLS 终结的反代；此时设 `TRUST_PROXY=1`
- `/metrics` 配置 ANN_METRICS_TOKEN 后才暴露（默认 404）
- 定期跑 `scripts/backup.sh` + `backup_drill.py` 演练
