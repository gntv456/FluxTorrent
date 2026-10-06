-- 0286：发种/审种动线实测审计（_doc/发种审种动线实测审计-2026-10-05.md）的落地面。
--
-- 本轮实测跑出的「站长以为生效、其实没人读」与「用户被静默锁死」两类问题，
-- 需要新增的持久化只有三件：
--   ① 发种闸门参数化（体积上限、内容最低标准、重复发布策略）——默认值全部等于
--      今天的行为，站长按需收紧；不预置新默认是为了不把存量站与 e2e 一把打红。
--   ② `torrent_files` 之外的 screenshots 列开始真有写入方（审核队列的截图数不再恒 0），
--      列本身早在 0001 就有，这里只登记它的闸门参数。
--   ③ `messages.kind/params`：审核结果这类「先存下来、以后再展示」的文本改存 key + 参数，
--      切语言才翻得动（项目 09-27 已定口径；本轮先把写口与读口打通）。
--
-- ⚠️ 值行先于登记行（settings_meta.name 有 FK → site_settings.name，0230 教训）。
-- ⚠️ 全部幂等（IF NOT EXISTS / ON CONFLICT DO NOTHING），撞号或重跑不炸 api。

-- ① 发种闸门（grp/group_key = torrent，与 upload_auto_approve_class 同区）
INSERT INTO site_settings (name, value, descr, grp) VALUES
-- .torrent 体积上限（字节）。原为代码常量 4 MiB，10 TB 级合集（16 MiB 分片 ≈ 13 MB）发不出来
('upload_torrent_max_bytes', '4194304',
 '上传 .torrent 的体积上限（字节）。默认 4 MiB；大合集按 分片数×20B 估算上调，上限 64 MiB。', 'torrent'),
-- 简介最短长度（0 = 不要求）
('upload_min_descr_len', '0',
 '发种时简介正文的最少字符数，0 = 不限制。审核台最常见的驳回理由就是「描述不完整」，'
 '设成本站底线可让不合格内容根本进不了队列。', 'torrent'),
-- 简介最少截图数（0 = 不要求；截图按简介里的 http(s) 图计数）
('upload_require_screenshots', '0',
 '发种时简介里至少要有几张图（按 http/https 图片链接计数），0 = 不限制。'
 '计数来源与详情页/审核队列同源（torrents.screenshots）。', 'torrent'),
-- 必须提交 MediaInfo
('upload_require_mediainfo', 'no',
 'yes = 发种必须提交 MediaInfo 文本（影视站常用）。默认 no 以保持中立站型可用。', 'torrent'),
-- 标题命名规范正则（空 = 不校验）
('upload_title_pattern', '',
 '发种名称需匹配的 Java 正则（命名规范），留空即不校验。'
 '例：^[^.]+\\.[0-9]{4}\\.(1080p|2160p|720p) 只放行「片名.年份.分辨率」格式。', 'torrent'),
-- 重复发布策略
('upload_dup_policy', 'suggest',
 '同内容（pieces_hash 命中）再次发布时的处置：suggest = 只提示不拦（默认，等于今天）；'
 'block = 直接拒；group = 自动并入既有聚合组。', 'torrent')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, group_key, card_order, visible)
VALUES
  ('upload_torrent_max_bytes', 'number', '.torrent 体积上限',
   'Torrent size cap', '字节。默认 4 MiB，可按站上调至 64 MiB（大合集分片表就超 4 MiB）。',
   'torrent', 80, true),
  ('upload_min_descr_len', 'number', '简介最少字符',
   'Min description length', '0 = 不限制。超过 30 字会挡掉「只写一行」的应付式发布。',
   'torrent', 81, true),
  ('upload_require_screenshots', 'number', '简介最少截图数',
   'Min screenshots', '0 = 不限制。按简介里的 http(s) 图片链接计数。',
   'torrent', 82, true),
  ('upload_require_mediainfo', 'yesno', '必须提交 MediaInfo',
   'Require MediaInfo', 'yes = 发种必须带 MediaInfo；默认 no 以免中立站型被影视口径卡住。',
   'torrent', 83, true),
  ('upload_title_pattern', 'text', '发种命名规范（正则）',
   'Title pattern (regex)', '留空不校验；填 Java 正则，不匹配即拒。改坏正则会在发种时报「站点命名正则非法」。',
   'torrent', 84, true),
  ('upload_dup_policy', 'enum', '同内容重复发布',
   'Duplicate upload policy', 'suggest = 只提示；block = 拒绝；group = 自动并入既有聚合组。',
   'torrent', 85, true)
ON CONFLICT (name) DO NOTHING;

-- enum 控件要选项串，否则后台渲染成空下拉（0160 同类修正的口径）
UPDATE settings_meta SET options = '[{"v":"suggest","zh":"只提示","en":"Suggest only"},{"v":"block","zh":"直接拒","en":"Block"},{"v":"group","zh":"自动并组","en":"Auto-group"}]'
 WHERE name = 'upload_dup_policy' AND options IS NULL;

-- ② messages 存 key + 参数（渲染时现取字典，切语言才翻得动）
--    老消息 kind/params 为 NULL → 读端继续用 subject/body 原文，零迁移成本。
ALTER TABLE messages ADD COLUMN IF NOT EXISTS kind text;
ALTER TABLE messages ADD COLUMN IF NOT EXISTS params jsonb;
CREATE INDEX IF NOT EXISTS idx_messages_kind ON messages (kind) WHERE kind IS NOT NULL;

-- ③ 审核台批量裁决权限：沿用 0285 的 torrent.review，不再新键（此处仅补注释与
--    把「批量」能力落到 /admin/torrents/batch 的 approve/reject 动作上，无表变更）。
COMMENT ON COLUMN messages.kind IS
 '站内信文案键（如 review.approved）；非空时前端按键现取字典渲染，params 提供插值';
COMMENT ON COLUMN messages.params IS
 '站内信插值参数（jsonb，如 {"id":123,"name":"…","reason":"…"}）';
