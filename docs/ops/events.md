# 运维事件订阅（webhook）

> 生命周期事件自动广播到管理侧渠道（Discord webhook / Telegram Bot）。
> 配置在后台「站点设定 → 通知」：`webhook_discord`（Discord 频道 webhook URL）、
> `tg_bot_token` + `tg_chat_id`（Telegram Bot）。两者可同时配，可都不配
> （不配即静默，无任何报错）。
>
> 相关：[监控告警](monitoring.md)（Prometheus 侧指标告警——那条是「系统健康」，
> 本文是「业务事件」）· [运营 playbook](../webmaster/playbook/05-anticheat.md)。

## 事件矩阵（E12 起全覆盖）

| 事件 | 触发点 | 文案示例 |
|---|---|---|
| 新用户注册 | 注册成功（含邀请/开放/申请转正） | `新用户注册：alice（第 42 位活跃成员）` |
| 封禁 / 禁言 / 恢复 | 后台用户状态变更 | `用户 bob（#17）封禁：买卖账号` |
| H&R 违规产生 | worker hr_enforce 批次 | `H&R 违规 3 例（hr_enforce job）` |
| 捐赠到账 | 支付回调/补单（达档时附回馈摘要） | `收到捐赠 $20.00（「铜牌赞助者」：魔力 +5000…）` |
| 新申请（申请制） | 用户提交入站申请 | 见 applications 面板 |
| 作弊新嫌疑 | cheat_audit 四件套命中 | 见反作弊巡检篇 |

## 设计口径

1. **尽力而为**：广播失败只落日志，永不阻塞主流程（注册/回调/封禁本身）；
2. **不含 PII**：广播文案只带用户名/ID/计数，不带邮箱、IP、passkey；
3. **管理侧定位**：这是给站长/管理组的「事件流」，不是用户通知（用户通知走
   站内信+邮件的 notice_prefs 体系，见安全姿态文档）；
4. **无重试**：Discord/TG 的 webhook 语义本身就是 at-most-once；漏了就看
   后台对应面板（举报/申请/捐赠订单都有面板兜底）。

## 自定义扩展（站长接自己的自动化）

想接企业微信/飞书/自建机器人：webhook 目标是 Discord 格式的 JSON
（`{"content": "…"}`）与 TG Bot API。中间加一层转换服务（如
discord-webhook-proxy 或自写 Cloudflare Worker）即可桥接任意目标。
代码侧新增事件：在生命周期写入点调 `ops_webhook::broadcast_ops_spawn`
（api 侧）或 `jobs::audit::webhook_broadcast`（worker 侧）。
