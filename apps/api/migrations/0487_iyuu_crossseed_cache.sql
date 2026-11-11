-- 0487: IYUU 辅种查重结果缓存表（定时批量刷新，页面只读库）
--
-- 背景（2026-11-11 站长提醒）：IYUU 的查重是**配额制**的（不是按频率限流，
-- 实测 25 次/15 秒不触发；但总量受限）。而种子详情页是全站最热的页面，
-- 每次点开都打一次 IYUU 显然不行 —— 站点量一大立刻超额，且页面要等出站。
--
-- 所以改成「定时批量 + 库缓存」，这也是 IYUU 生态里各站的通行做法：
--   * worker 每小时挑一批「最久没刷过」的已审核种子，逐条查一次，落本表；
--   * 详情页 `/api/v1/torrents/{id}/crossseed` **只读本表**，零出站。
--
-- 字段：
--   torrent_id  本站种子 id（主键，一条种子一行）
--   hits        命中数组 `[{sid, torrent_id}]`（对方站点 id + 对方种子 id）
--   ok          本次查询是否成功。false = 查过但失败（网络/配额），
--               与「查成功但没人有」区分开，避免把失败当成空结果长期缓存
--   fetched_at  最后一次刷新时间（挑最旧的就是按它升序）
--
-- 幂等：IF NOT EXISTS。

CREATE TABLE IF NOT EXISTS iyuu_crossseed (
    torrent_id BIGINT PRIMARY KEY,
    hits       JSONB NOT NULL DEFAULT '[]'::jsonb,
    ok         BOOLEAN NOT NULL DEFAULT TRUE,
    fetched_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 挑「最久没刷」的种子就是这个索引在干活
CREATE INDEX IF NOT EXISTS idx_iyuu_crossseed_fetched
    ON iyuu_crossseed (fetched_at);
