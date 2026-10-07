-- 0300（假种/作弊审计 2026-10-07 P0-2 治本，报告 _doc/假种与作弊漏洞深度审计-2026-10-07.md）
-- 交叉上报（cross-report）：上传量由「leecher 佐证」取代「上传者全自报」。
--
-- 问题：uploader 的 uploaded 累计值 100% 由其客户端自报，站点无任何第三方
-- 佐证 → 伪造者把每轮增量贴着速率上限报即可无限做高 ratio。
-- 机制（Gazelle/OPS 业界口径）：
--   ① leecher announce 携带扩展参数 `xreport=<peer_id_hex>:<bytes>`，声明
--      「本轮从该 peer 下载了 bytes」。tracker 校验目标是同 swarm 存活做种
--      peer 且非自己（见 tracker corroboration_target），通过后投递
--      flux:xreport 流。
--   ② worker 消费 flux:xreport，把「被佐证的上传量」累加进
--      upload_corroborated（按 uploader+torrent 维度）。这是上传量的
--      **可信上界**。
--   ③ 计费时（process_event）把 uploader 的 credited upload 限制在
--      「已佐证总量」之内——超出部分不入账（记 cheat_events 留痕）。
--      未开启交叉上报（xreport 为空）时维持原自报口径，保证向后兼容；
--      开启后 ratio 无法凭自报做高。
--
-- 幂等：xreport 消费按 Redis 流 entry id 落 xreport_seen 去重；累计按
-- (uploader, torrent) 单调累加，重复佐证不会翻倍（见 worker 消费逻辑）。

-- ① 被佐证的上传量累计表（可信上界）
CREATE TABLE IF NOT EXISTS upload_corroborated (
    uploader_id  bigint NOT NULL,
    torrent_id   bigint NOT NULL,
    bytes        bigint NOT NULL DEFAULT 0,
    updated_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (uploader_id, torrent_id)
);
COMMENT ON TABLE upload_corroborated IS
'交叉佐证的可信上传量上界：leecher 声明「从该 uploader 下载了 N 字节」且 tracker 校验通过后累加。计费时 uploader 的 credited upload 不得超过此值（未开启交叉上报时此表为空、不限制）。';

-- ② xreport 消费幂等表（按 Redis 流 entry id 恰好一次）
CREATE TABLE IF NOT EXISTS xreport_seen (
    event_id     text PRIMARY KEY,
    created_at   timestamptz NOT NULL DEFAULT now()
);
COMMENT ON TABLE xreport_seen IS
'交叉上报消费的幂等键（Redis 流 entry id）：同一 entry 被 PEL 回收/重投时跳过，避免重复佐证累加。';

-- ③ 累计热点：按 uploader 清理陈旧行（长期无新佐证的上传者不被无限保留）
CREATE INDEX IF NOT EXISTS idx_upload_corroborated_updated
    ON upload_corroborated (updated_at);

-- ④ xreport_seen 清理索引（配合消费循环裁剪 3 天前的老键）
CREATE INDEX IF NOT EXISTS idx_xreport_seen_created
    ON xreport_seen (created_at);
