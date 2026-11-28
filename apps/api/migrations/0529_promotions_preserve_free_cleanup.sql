-- 0529 保种区免费短券：过期行清理 + 防暴涨索引（2026-11-28）
--
-- 发现：preserve_free 短券每小时续期=每小时插新行，旧行永不清 →
-- promotions 表暴涨到 19.6 万行（占 73%），promo 合成子查询
-- （全站列表/RSS 都走的 promo.rs LATERAL）Bitmap Scan 每次拉 1.3 万行
-- 候选再逐行过滤，保种区列表单查询 30s+。
--
-- ① 清理已过期的 preserve_free 短券（每小时续期=每小时插新行，
--    旧行永不清则表按小时暴涨：生产实证一天 19.6 万行占 73%）
-- ② (torrent_id, source, ends_at) 复合索引：种子级点查 promo 命中
--    从「拉全量候选再过滤」变成索引直达

DELETE FROM promotions
WHERE source = 'preserve_free'
  AND ends_at < now();

CREATE INDEX IF NOT EXISTS idx_promotions_torrent_source_ends
  ON promotions (torrent_id, source, ends_at DESC)
  WHERE torrent_id IS NOT NULL;
