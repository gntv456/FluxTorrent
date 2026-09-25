-- 0190：论坛视频内嵌 V1.5/V2 配套（策划案 §3.5/§4）。
--
-- 1) embed 品类：content_packs.kind 扩 'embed'——视频内嵌规则可作为生态商店
--    规则包上架（payload.tables 键白名单放行 video_embed_rules，纯覆盖口径
--    与 0173 assets 品类一致：apply 侧白名单见 pack_format 校验）。
-- 2) 后台导航：论坛结构独立页下加「视频内嵌」子条目（forums 同权限档 93）。
-- 3) V2 设置行：站内视频上传四开关（缺省 no——重资源能力默认关，0178 中立纪律）。

-- ---------- 1) embed 品类 ----------
ALTER TABLE content_packs DROP CONSTRAINT IF EXISTS content_packs_kind_check;
ALTER TABLE content_packs ADD CONSTRAINT content_packs_kind_check
  CHECK (kind IN ('taxonomy', 'theme', 'rules', 'assets', 'embed'));

-- ---------- 2) 后台导航 ----------
-- staff_panel_entries 无 tab_key 唯一约束，用 NOT EXISTS 防重放重复插入
INSERT INTO staff_panel_entries
  (panel, name, url, info, sort, section, min_class, tab_key)
SELECT 'admin', '视频内嵌', '/admin?tool=embedrules',
       '论坛帖内视频白名单规则', 5, 'content', 93, 'embedrules'
WHERE NOT EXISTS (
  SELECT 1 FROM staff_panel_entries WHERE tab_key = 'embedrules'
);

-- ---------- 3) V2 设置行（0190 落位，端点就绪后生效） ----------
INSERT INTO site_settings (name, value, descr, grp) VALUES
  ('forum_video_upload', 'no',  '论坛站内视频上传开关（0=关）', 'forum'),
  ('video_max_mib',      '200', '站内视频单文件上限（MiB，0=禁用）', 'forum'),
  ('video_quota_mib',    '2048','每人视频配额（MiB，0=不限）', 'forum'),
  ('video_daily_limit',  '5',   '每人每日视频上传条数（0=不限）', 'forum'),
  ('post_video_max',     '3',   '每帖视频块上限（!video 出现次数）', 'forum')
ON CONFLICT (name) DO NOTHING;

-- ---------- 4) attachments 扩列（V2：视频元数据 + 分类账） ----------
-- kind 区分普通附件与视频附件（配额分账：视频只吃 video_quota_mib）；
-- metadata 存前端抽取的 duration/width/height/poster_sha（服务端只兜底魔数）。
ALTER TABLE attachments ADD COLUMN IF NOT EXISTS kind TEXT NOT NULL
  DEFAULT 'file' CHECK (kind IN ('file', 'video'));
ALTER TABLE attachments ADD COLUMN IF NOT EXISTS metadata JSONB
  NOT NULL DEFAULT '{}'::jsonb;
-- 每日频率按 created_at::date 计数：created_at 是 timestamptz，::date 依会话
-- 时区非 IMMUTABLE，不能进索引表达式——直接按裸 timestamptz 建 range 可用
-- 的普通索引，查询侧仍写 created_at::date = current_date（走顺序扫描也够：
-- 单用户视频行数量级很小）。
CREATE INDEX IF NOT EXISTS idx_attachments_user_kind
  ON attachments (user_id, kind, created_at);
