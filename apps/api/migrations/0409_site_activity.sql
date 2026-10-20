-- 0409 全站动态独立流（修正 0406 的承接方选型）。
--
-- 0406 把字幕/评选播报写进 announcements（首页公告区），实测违背
-- 「公告区=管理员官方发声」的产品语义——系统动态把站长公告冲掉。
-- 本迁移建独立 site_activity 时间线，两写入点（本批代码）改写这里；
-- announcements 回归纯管理公告，0406 期间写入的字幕/评选播报行
-- 迁入新表后从公告区删除（本地库实测 10 条「新字幕上架」）。
--
-- 幂等：表 CREATE IF NOT EXISTS；数据搬迁 WHERE badge IN (...) 限定
-- 0406 写入形态，重跑安全（迁完后源行不存在，INSERT..SELECT 零行）。

CREATE TABLE IF NOT EXISTS site_activity (
  id          BIGSERIAL PRIMARY KEY,
  -- 事件类型：subtitle_upload / award_reveal（后续事件扩充枚举即可）
  kind        TEXT NOT NULL,
  -- 展示正文（成品文案；不搞 params 后渲染——时间线不需要事后翻译）
  text        TEXT NOT NULL,
  -- 关联对象（字幕 id / 评选期号），便于溯源跳转；可空
  ref_id      BIGINT,
  -- 发布者 uid（匿名字幕=匿名文案但留真实 uid 供 staff 溯源；展示层不露出）
  actor_id    BIGINT,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS site_activity_recent_idx
  ON site_activity(created_at DESC);

COMMENT ON TABLE site_activity IS
  '全站动态时间线：系统事件流（字幕上架/评选揭晓/…）。与公告（announcements=管理员官方发布）、聊天室（shoutbox=用户聊天）三渠分立';

-- 0406 期间的播报行搬迁：announcements(字幕/评选) → site_activity
INSERT INTO site_activity (kind, text, actor_id, created_at)
SELECT
  CASE badge WHEN '字幕' THEN 'subtitle_upload' ELSE 'award_reveal' END,
  body,
  author_id,
  created_at
FROM announcements
WHERE badge IN ('字幕', '评选');

DELETE FROM announcements WHERE badge IN ('字幕', '评选');
