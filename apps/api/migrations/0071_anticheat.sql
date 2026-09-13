-- 0071 反作弊加固：peer_id 交叉验证列 + Ratio Watch 观察期 + connectable 标记
-- 对照 _doc/反作弊与性能加固方案.md P0-7 / P1-8 / P1-9。

-- ① peer_id 交叉验证：peer_id 正则与 User-Agent 正则双重匹配（NP agent_allowed_family 口径的精简版）。
--    吸血客户端常出现 peer_id 与 UA 声称不一致（如 UA 报 qBittorrent、peer_id 为 -XL 前缀）。
ALTER TABLE agent_rules ADD COLUMN IF NOT EXISTS peer_id_pattern TEXT NOT NULL DEFAULT '';

-- ② Ratio Watch（GZ 口径柔性观察期）：分享率跌破阈值先警告并给 14 天期限，不直接处罚。
ALTER TABLE users ADD COLUMN IF NOT EXISTS ratio_watch_until TIMESTAMPTZ;
ALTER TABLE users ADD COLUMN IF NOT EXISTS ratio_warned_at TIMESTAMPTZ;

-- ③ connectable（tracker 主动回连抽样结果）：-1=未测 0=不可达 1=可达。
--    假保种客户端不可回连且长期零上传；真 NAT 用户不可达但有真实上传，二者靠「可达性 × 上传量」组合区分。
--    默认 1：历史行不因缺数据被误伤做种收益。
ALTER TABLE snatches ADD COLUMN IF NOT EXISTS connectable SMALLINT NOT NULL DEFAULT 1;
