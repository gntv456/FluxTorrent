-- 0283 深测修复批（2026-10-05）
-- 1) push_outbox：Web Push 发送队列（worker 产出 → api 消费投递）。
--    worker 无法引用 api 的 RFC8291 投递实现（crate 无 lib target），用 outbox
--    解耦：worker 只落库，api 常驻循环消费，加密投递实现保持单份。
CREATE TABLE IF NOT EXISTS push_outbox (
    id           BIGSERIAL PRIMARY KEY,
    user_id      BIGINT      NOT NULL,
    topic        TEXT        NOT NULL,           -- hr / promo / message
    title        TEXT        NOT NULL,
    body         TEXT        NOT NULL,
    dedupe_key   TEXT,                           -- 幂等键（可选）：同键未投递过才入队
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    processed_at TIMESTAMPTZ,                    -- NULL = 待投递
    processed_ok BOOLEAN     NOT NULL DEFAULT false
);
CREATE INDEX IF NOT EXISTS idx_push_outbox_pending
    ON push_outbox (id) WHERE processed_at IS NULL;
CREATE UNIQUE INDEX IF NOT EXISTS idx_push_outbox_dedupe
    ON push_outbox (dedupe_key) WHERE dedupe_key IS NOT NULL AND processed_at IS NULL;

-- 2) agent_rules 种子：开箱即用的已知作弊/刷传客户端封禁规则
--    （模式来源：NP 系站点通行的客户端黑名单口径；mode=deny 拒绝 announce）。
INSERT INTO agent_rules (mode, pattern, peer_id_pattern, note)
SELECT * FROM (VALUES
    -- UA 黑名单（模式来源：NP 系站点通行的客户端黑名单口径；mode=deny 拒绝 announce）
    ('deny', '-UM[0-9]+', '', '旧 uTorrent Monolith（多漏洞版本）'),
    ('deny', '-BT0001-', '', 'BitTorrent 6.x 旧内核'),
    ('deny', '-xl[0-9]+', '', '迅雷（吸血，拒绝汇报）'),
    ('deny', '-SD[0-9]+', '', '迅雷看看/山寨客户端'),
    ('deny', '-XF[0-9]+', '', '迅雷先锋（XunLei Frontier）'),
    ('deny', '-dt[0-9]+', '', 'dt 系伪装客户端'),
    ('deny', '-HL[0-9]+', '', 'Halean 旧作弊内核'),
    ('deny', '-TK[0-9]+', '', 'TorrentKeeper 作弊内核'),
    -- peer_id 黑名单（UA 字段留空，只按 peer_id 前缀判）
    ('deny', '', '^-NV0000-', 'NV 作弊客户端 v0'),
    ('deny', '', '^-[FQ]0000-', 'FQ/QX 刷量客户端'),
    ('deny', '', '^-XL[0-9]+', '迅雷内核 peer_id 特征'),
    ('deny', '', '^-SD[0-9]+', '山寨客户端 peer_id 特征')
) AS seed(mode, pattern, peer_id_pattern, note)
WHERE NOT EXISTS (SELECT 1 FROM agent_rules);
