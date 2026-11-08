-- 0474 权限接入状态刷新：implemented 标记与代码现状对齐
--
-- 0057 时代标注「未接入」的 7 项中，4 项此后已实现并有真实 enforcement
-- 点（本迁移翻回 true），管理界面不再误标「未接入」；其余 3 项确认仍
-- 未接线，保持 false（授予不生效，界面如实标注）。
--
-- 逐项核对（enforcement 点 = grep perm 常量的非测试引用）：
--   torrent.view_anonymous → detail.rs / aggregate.rs 匿名发布者可见性 ✓
--   torrent.see_banned     → torrents 列表 / 字幕区被禁资源可见性 ✓
--   seed.stats.view        → staff_http/stats.rs 保种统计端点 ✓
--   announce.publish       → http/news_fun.rs 公告发布/编辑 ✓
--   torrent.set_price      → 发种时自带促销/定价仍无端点，保持 false
--   torrent.upload_special → 特殊类别发布限制仍无端点，保持 false
--   torrent.repost         → 转载工具链仍未实现，保持 false
UPDATE permissions SET implemented = true WHERE key IN (
  'torrent.view_anonymous',
  'torrent.see_banned',
  'seed.stats.view',
  'announce.publish'
);
