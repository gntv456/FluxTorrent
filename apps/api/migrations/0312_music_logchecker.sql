-- 0312：音乐站 Logchecker 落地面（发种侧日志解析 + 详情侧展示）。
--
-- 背景：站型包早在 0037 就注册了 music / lossless，但音乐站真正的核心能力
-- ——EAC / CUETools / WHipper 抓轨日志的解析与打分——全仓没有实现方。
-- 竞品口径（RED/What.CD）以「日志分」作为无损区的准入与排序依据，
-- 没有它，音乐站型只是换了套分类名的影视站。
--
-- 本迁移只做两件持久化：
--   ① `torrent_logs`：一枚种可挂多张碟的日志（1:N，按 ordinal 排序），
--      正文内联存储。内联而非进 attachments：日志是**站方判定依据**而非
--      用户上传的展示素材，必须与种子同生命周期（ON DELETE CASCADE）、
--      不受附件配额与 MIME 白名单摆布。
--   ② 四个闸门参数进 site_settings。默认 policy=off：新装与存量站行为不变，
--      站长按站开启（对齐 0286「站方可配的最低内容标准默认全关」口径）。
--
-- 分数不写回 torrents 列：日志分是多张碟的 MIN，派生即可（读路径算），
-- 加列会牵动 TorrentRow / domain-types / 编辑回退三处，属未来账。
--
-- ⚠️ 值行先于登记行（settings_meta.name 有 FK → site_settings.name，0230 教训）。
-- ⚠️ 全部幂等（IF NOT EXISTS / ON CONFLICT DO NOTHING），撞号或重跑不炸 api。

-- ============ ① 日志落库 ============
CREATE TABLE IF NOT EXISTS torrent_logs (
  torrent_id BIGINT NOT NULL REFERENCES torrents(id) ON DELETE CASCADE,
  ordinal    INT NOT NULL,
  filename   TEXT NOT NULL,
  -- 引擎名不设 CHECK：识别集按真实日志样本增补，加约束等于每次扩引擎改表
  engine     TEXT NOT NULL,
  -- 0–100；NULL = 认不出引擎或日志不完整，无法定分（≠ 0 分）
  log_score  SMALLINT,
  tracks     INT NOT NULL DEFAULT 0,
  issues     JSONB NOT NULL DEFAULT '[]'::jsonb,
  facts      JSONB NOT NULL DEFAULT '{}'::jsonb,
  size       BIGINT NOT NULL,
  body       TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (torrent_id, ordinal)
);

CREATE INDEX IF NOT EXISTS idx_tlogs_score ON torrent_logs (log_score);

-- ============ ② 闸门参数 ============
INSERT INTO site_settings (name, value, descr, grp) VALUES
('logcheck_policy', 'off',
 '抓轨日志（EAC / CUETools / WHipper…）检查力度：off = 不解析也不接收日志附件；'
 'tag = 解析并打分、在详情页标注，但不拦发布；require = 命中「强制站型」的种'
 '必须带可解析日志且分数达线，否则发种报错。默认 off。', 'torrent'),
('logcheck_min_score', '100',
 '日志分下限（0–100）。policy=require 时低于该分直接拒发。无损区的通行底线是'
 ' 100（安全模式 + 校验通过），放宽到 99 即放行「单次拷贝」这类未复验的抓轨。',
 'torrent'),
('logcheck_max_kib', '1024',
 '单个日志文件体积上限（KiB）。EAC 日志通常 20–200 KiB，多碟/含 CUETools 区块'
 '会更大；超限的日志 part 直接拒收而不是截断（截断的日志必然解析失败）。',
 'torrent'),
('logcheck_gate_site_types', 'music,lossless',
 'policy=require 时**强制**交日志的站型 code 清单（逗号分隔）。留空 = 全站型强制。'
 '非清单内站型仍可附日志（tag 效果），只是不作为发种门槛。', 'torrent')
ON CONFLICT (name) DO NOTHING;

-- options 列在 unit 之后：unit 不适用的行必须显式写 NULL，
-- 否则整列错位（0308 同类修正的口径）
INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, options, group_key, card_order,
   visible)
VALUES
  ('logcheck_policy', 'enum', '抓轨日志检查', 'Rip log checking',
   'off = 关闭；tag = 只打分标注；require = 音频站型必须交达线日志才能发种。',
   NULL,
   '[{"v":"off","zh":"关闭","en":"Off"},'
   '{"v":"tag","zh":"打分标注","en":"Score and label"},'
   '{"v":"require","zh":"打分并强制","en":"Score and require"}]',
   'torrent', 90, true),
  ('logcheck_min_score', 'number', '日志分下限', 'Min log score',
   '仅 policy=require 时生效：低于该分拒发。', '分', NULL,
   'torrent', 91, true),
  ('logcheck_max_kib', 'number', '单日志体积上限', 'Max log size',
   'KiB。超限的日志 part 会被拒收（不是截断后硬解析）。', 'KiB', NULL,
   'torrent', 92, true),
  ('logcheck_gate_site_types', 'text', '强制交日志的站型',
   'Site types requiring logs',
   '逗号分隔的站型 code，如 music,lossless；留空表示所有站型都强制。',
   NULL, NULL, 'torrent', 93, true)
ON CONFLICT (name) DO NOTHING;
