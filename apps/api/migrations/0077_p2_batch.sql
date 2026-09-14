-- 0077 P2 批次（对照 v3 §27-14/15/17/18/19/20/22）：
--   A) 被拒禁发：deny_count 计数（upload 前置校验，阈值 site_settings upload_deny_limit 默认 2）
--   B) 自动促销规则表（U3D 口径）：过审时按 position 匹配第一条命中规则
--   C) 盒子打标：torrents.highspeed（worker 扫 snatches 上传速度超阈值）
--   D) POSTPONED 审核态：approval_status=4（3 已被 0063 用作"下架"，不复用）
--   E) 产出回收对账：月度聚合视图

-- A) 被拒计数（0075 的 approve_streak 配对：被拒清零已有，这里是累计拒绝次数）
ALTER TABLE users ADD COLUMN IF NOT EXISTS deny_count INT NOT NULL DEFAULT 0;

-- B) 自动促销规则（U3D automatic_torrent_freeleech 口径，简化为 kind 六档）
CREATE TABLE IF NOT EXISTS auto_promo_rules (
    id BIGSERIAL PRIMARY KEY,
    position INT NOT NULL DEFAULT 0,
    name TEXT NOT NULL,
    name_regex TEXT NOT NULL DEFAULT '',
    min_size BIGINT NOT NULL DEFAULT 0,       -- 字节；0=不限
    max_size BIGINT NOT NULL DEFAULT 0,       -- 0=不限
    category_id INT REFERENCES categories(id),
    kind TEXT NOT NULL,                        -- free/x2/x2free/half/x2half/p30
    hours INT NOT NULL DEFAULT 72,
    enabled BOOLEAN NOT NULL DEFAULT TRUE
);

-- C) 盒子/高速做种标记（展示用，不惩罚——教育网服务器用户识别后给激励）
ALTER TABLE torrents ADD COLUMN IF NOT EXISTS highspeed BOOLEAN NOT NULL DEFAULT FALSE;

-- E) 火花产出-回收月度对账视图（v3 §27-22：通胀监控的数据底座）
CREATE OR REPLACE VIEW v_spark_flow_monthly AS
SELECT to_char(created_at AT TIME ZONE 'Asia/Shanghai', 'YYYY-MM') AS month,
       SUM(CASE WHEN amount > 0 THEN amount ELSE 0 END) AS minted,
       SUM(CASE WHEN amount < 0 THEN -amount ELSE 0 END) AS burned,
       SUM(amount) AS net,
       COUNT(*) AS entries
FROM spark_ledger GROUP BY 1 ORDER BY 1 DESC;

-- D) POSTPONED（U3D 第四态）：staff 审核可转暂缓（证据不足），暂缓种仅发布者+staff可见
