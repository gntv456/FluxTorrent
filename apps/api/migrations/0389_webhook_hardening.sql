-- 0389_webhook_hardening.sql
-- 运维 webhook 出口硬化（评估报告 §1.4）
--
-- 现状（0228 ops_webhook.rs）：两个出口（Discord / Telegram）都是
-- 「拿到 URL 就 POST，失败记一行日志」，没有任何防护：
--   * **无签名**：收方无法验证这条告警是不是本站发的。一旦 webhook URL
--     泄露（它常被贴进工单、截图、日志），任何人都能伪造
--     「作弊告警」「新申请」来钓鱼值班管理员；
--   * **无 SSRF 防护**：URL 存在 site_settings 里，任何有改设置权限的人
--     都能把它指向 `http://169.254.169.254/...`（云元数据）或内网
--     管理端口，借 api 的身份把内网扫一遍；
--   * **无重试、无自动停用**：一次网络抖动就是丢一条告警；而持续失败时
--     每个事件都重试，把 api 的连接池耗在一条死 URL 上。
--
-- 本迁移补三样基础设施（判定与执行在 domain/webhook）：
--   ① 出网 URL 白名单：只允许公网 https 与已登记的内网豁免，
--      私网/环回/链路本地一律拒绝（fail-closed）；
--   ② HMAC 签名密钥 + 最近失败计数与自动停用阈值；
--   ③ 投递台账：每次投递留一行（出口/事件/结果/耗时），
--      「告警到底有没有送到」从此可查，而不是只存在于日志里。
--
-- 【不改变的部分】两路出口的报文格式与触发时机都不动——本条只加护栏。

BEGIN;

-- ① 出网 URL 白名单：内网/环回/链路本地默认全拒。
--    白名单为空 = 只允许公网 https（最严且是安全默认）。
--    确有内网 webhook 的站点由站长显式登记 ip/网段。
CREATE TABLE IF NOT EXISTS webhook_egress_rules (
    id          BIGSERIAL   PRIMARY KEY,
    -- 网段（CIDR）。登记后才允许对该段的 webhook 出网。
    cidr        CIDR        NOT NULL UNIQUE,
    note        TEXT        NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE webhook_egress_rules IS
    'webhook 出网白名单（0389）。默认只允许公网 https；'
    '确有内网 webhook 的站点在这里显式登记网段（fail-closed：'
    '空表 = 拒绝一切非公网目标）。';

-- ② 签名密钥与健康度
ALTER TABLE site_settings
    ADD COLUMN IF NOT EXISTS grp    TEXT,
    ADD COLUMN IF NOT EXISTS secret BOOLEAN NOT NULL DEFAULT false;

INSERT INTO site_settings (name, value) VALUES
    -- HMAC 密钥（十六进制）。为空 = 不签名（保持旧站兼容，但管理面会提示）。
    ('webhook_sign_secret', ''),
    -- 连续失败多少次后自动停用该出口（0 = 不自动停用）
    ('webhook_failure_threshold', '10'),
    -- 重试次数（不含首次）
    ('webhook_retry_times', '2'),
    -- 出网超时（秒）
    ('webhook_timeout_secs', '10')
ON CONFLICT (name) DO NOTHING;

COMMENT ON COLUMN site_settings.secret IS
    '是否为敏感值（API/密钥类）。后台读面一律掩码，不回显明文。';

COMMENT ON TABLE site_settings IS
    '站点配置。webhook_sign_secret：HMAC-SHA256 签名密钥，'
    '非空时每个投递带 X-Flux-Signature 头。webhook_failure_threshold：'
    '连续失败达该次数自动停用该出口（防死 URL 拖垮连接池）。';

-- ③ 投递台账：告警有没有送到，从「看日志」变成「查表」
CREATE TABLE IF NOT EXISTS webhook_deliveries (
    id          BIGSERIAL   PRIMARY KEY,
    -- 出口标识：discord / telegram
    channel     TEXT        NOT NULL,
    -- 事件标识（如 cheat_alert / new_application）
    event       TEXT        NOT NULL DEFAULT '',
    -- ok / failed / skipped（未配置）/ disabled（已自动停用）
    status      TEXT        NOT NULL
                            CHECK (status IN ('ok','failed','skipped','disabled')),
    attempts    INT         NOT NULL DEFAULT 0,
    -- 投递耗时（毫秒）
    elapsed_ms  INT         NOT NULL DEFAULT 0,
    -- 失败原因摘要（不记 URL 与正文：两者都可能含凭据）
    detail      TEXT        NOT NULL DEFAULT '',
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_time
    ON webhook_deliveries (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_webhook_deliveries_channel
    ON webhook_deliveries (channel, created_at DESC);

COMMENT ON TABLE webhook_deliveries IS
    '运维 webhook 投递台账（0389）。此前「告警有没有送到」只存在于'
    '进程日志里，日志轮转即丢失；本表让运营可查、可统计成功率。'
    '不记 URL 与正文：两者都可能含凭据或用户隐私。';

COMMIT;
