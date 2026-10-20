-- 0400 自动审种（批 1：地基——《自动审种功能策划案》v1.3 §7/§8/§13）
--
-- 设计决策（策划案 §1.2）：
--   * 不新建审核体系：裁决复用 services/review_decide.rs（裁决同尺），
--     本迁移只落「人机对照台账 + 拒因字典条目 + 配置键」三件；
--   * 「深检中」是台账内部态（verdict=3），不进 torrents 五态机（§14）；
--   * 配置全部 site_settings 键化，meta 成对 INSERT（照抄 0311 纪律），
--     options 一律 [{"v":..,"l":..}] 形态（0039 口径）。
--
-- 全部幂等（ON CONFLICT DO NOTHING），重跑安全。

-- ============ ① 人机对照台账（影子+执法通用，§7/0400） ============
CREATE TABLE IF NOT EXISTS auto_review_log (
  id            BIGSERIAL PRIMARY KEY,
  torrent_id    BIGINT NOT NULL REFERENCES torrents(id),
  uploader_id   BIGINT NOT NULL REFERENCES users(id),
  -- 裁决时点运行模式快照：0=影子 1=半自动（只自动通过）2=全自动
  mode          SMALLINT NOT NULL,
  -- 0=通过 1=拒绝 2=暂缓 3=深检中（台账内部态，不映射 torrents 状态）
  verdict       SMALLINT NOT NULL,
  -- 命中证据：[{"rule":"arule_ads_keywords","action":"deny","score":0,
  --            "detail":"命中词：××网盘"}]（detail 面向人工可读）
  hit_rules     JSONB NOT NULL DEFAULT '[]'::jsonb,
  -- 软规则综合分（H 命中时可空）
  final_score   SMALLINT,
  -- 规则集版本（与 0401 ruleset 快照呼应；批 1 恒 1）
  ruleset_ver   SMALLINT NOT NULL DEFAULT 1,
  -- 0=机器 1=人工补录对照（§13.1「人机相遇」/人工终审回填）
  decided_by    SMALLINT NOT NULL DEFAULT 0,
  -- 人工最终裁决（对照用，可空：0=通过 1=拒绝 2=暂缓放行）
  human_verdict SMALLINT,
  created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS auto_review_log_torrent_idx
  ON auto_review_log(torrent_id, created_at DESC);
CREATE INDEX IF NOT EXISTS auto_review_log_uploader_idx
  ON auto_review_log(uploader_id, created_at DESC);
-- 每种子的「最新一条机器意见」判定用（同规则维持/人机相遇对照查询）
CREATE UNIQUE INDEX IF NOT EXISTS auto_review_log_latest_uq
  ON auto_review_log(torrent_id)
  WHERE decided_by = 0 AND verdict <> 3;

COMMENT ON TABLE auto_review_log IS
  '自动审种人机对照台账：机器意见/裁决 + 人工终审回填，影子与执法共用';

-- ============ ② 拒因字典：自动规则专属条目（§7/0400） ============
-- 人工下拉与自动拒因共用字典（review_notify 的 CONCAT 口径零改动）；
-- sort 90+ 避让 0038 已占的 1..7，前缀「自动审核」便于站长识别。
INSERT INTO torrent_deny_reasons (sort, reason) VALUES
  (91, '自动审核：标题异常（乱码/夹码/命名不规范）'),
  (92, '自动审核：描述不完整（缺少必备说明）'),
  (93, '自动审核：描述含广告或引流信息'),
  (94, '自动审核：内容含站内敏感信息'),
  (95, '自动审核：疑似重复资源'),
  (96, '自动审核：文件异常（伪媒体/可疑文件）')
ON CONFLICT DO NOTHING;

-- ============ ③ 配置键（§3.1 卡片 1-5） ============
-- 出厂缺省 = 「均衡号」预设 + 影子模式（§6 强制两阶段：先影子校准）。
INSERT INTO site_settings (name, value, descr, grp) VALUES
  -- 卡片 1：总开关与模式
  ('auto_review_enabled', '0',
   '自动审种总开关。开启前请先在影子模式观察数日（每日管理摘要会给出一致率/误杀率）。',
   'auto_review'),
  ('auto_review_mode', 'shadow',
   '运行模式：shadow=影子（只记录不行动）/ half=半自动（只自动通过，拒绝留人工）/ full=全自动。',
   'auto_review'),
  ('auto_review_delay_sec', '90',
   '发布/编辑回退后等待多少秒再取件评审（给作者反悔修改的缓冲）。',
   'auto_review'),
  ('auto_review_daily_cap', '0',
   '每天最多自动裁决多少个种子（0=不限）。轮询按 created_at 当日计数。',
   'auto_review'),
  ('auto_review_notify_admin', '1',
   '自动裁决后是否给管理组发站内信摘要（每日一封汇总，影子期即为校准报表）。',
   'auto_review'),
  -- 卡片 2：一键预设（均衡/谨慎/高效/关闭 由设置页选择后批量带出同组键）
  ('auto_review_preset', 'balanced',
   '当前预设：balanced=均衡（出厂）/ cautious=谨慎 / efficient=高效 / off=关闭。选预设会覆盖本组动作与阈值键。',
   'auto_review'),
  -- 卡片 3：规则三要素（缺省动作取 §2.1/§2.2 安全阀口径）
  ('arule_title_garbled_enabled', '1', '规则「标题乱码/夹码」启用。', 'auto_review'),
  ('arule_title_garbled_action', 'deny', '标题乱码/夹码命中后的动作。', 'auto_review'),
  ('arule_descr_missing_enabled', '1', '规则「描述不完整」启用。', 'auto_review'),
  ('arule_descr_missing_action', 'postpone', '描述不完整命中后的动作（缺省暂缓，防误杀小体积资源）。', 'auto_review'),
  ('arule_descr_missing_min_chars', '80', '描述去标签后最少字符数（低于即命中）。', 'auto_review'),
  ('arule_ads_keywords_enabled', '1', '规则「描述含广告引流」启用。', 'auto_review'),
  ('arule_ads_keywords_action', 'deny', '广告引流命中后的动作。', 'auto_review'),
  ('arule_leak_marker_enabled', '1', '规则「泄露站内信息」启用。', 'auto_review'),
  ('arule_leak_marker_action', 'deny', '泄露站内信息命中后的动作。', 'auto_review'),
  ('arule_dupe_check_enabled', '1', '规则「疑似重复资源」启用。', 'auto_review'),
  ('arule_dupe_check_action', 'postpone', '疑似重复命中后的动作（缺省暂缓，相似度有误报面）。', 'auto_review'),
  ('arule_dupe_check_threshold', '92', '标题相似度超过多少百分比算疑似重复（0-100）。', 'auto_review'),
  ('arule_title_format_enabled', '1', '规则「标题命名规范」启用（§2.6 命名引擎）。', 'auto_review'),
  ('arule_title_format_action', 'postpone', '命名不规范命中后的动作（缺省暂缓；任何档位都不为「顺序不规范」自动拒）。', 'auto_review'),
  ('arule_title_format_level', 'warn', '命名检查力度：off=不检查 / warn=只提醒（暂缓附建议标题）/ strict=严格（缺必备要素才计拒）。', 'auto_review'),
  -- 卡片 4：词库与名单
  ('auto_review_ad_words',
   E'微信群\nQQ群\n加微信\n加V\n微信号\ntelegram\nt.me/\n网盘链接\n百度网盘\n夸克网盘\n迅雷云盘\n阿里云盘\n外站注册\n邀请注册\n收费代理\n付费解锁\n招商\n招揽\n推广链接\n私聊购买\n扫码\n二维码\n折扣码\n优惠码\n返利\n佣金\n兼职\n刷单\n代充\n代购',
   '广告/引流词库，一行一个词。命中即算广告（整词包含匹配，大小写不敏感）。',
   'auto_review'),
  ('auto_review_trusted_uploaders', '',
   '白名单 UID（逗号分隔）：这些发布者的种子跳过自动拒绝（仍可暂缓），只做记录。',
   'auto_review'),
  ('auto_review_watch_uploaders', '',
   '观察名单 UID（逗号分隔）：这些发布者的种子只暂缓、不自动通过。',
   'auto_review'),
  -- 卡片 5：通知文案（变量 {username} {torrent} {link} {reason} {site}）
  ('auto_review_mail_pass_subject', '【{site}】您的种子已自动过审',
   '自动通过邮件主题模板。', 'auto_review'),
  ('auto_review_mail_deny_subject', '【{site}】您的种子未通过自动审核——{reason}',
   '自动拒绝邮件主题模板。', 'auto_review'),
  -- 卡片 6：站型与分类适配（§2.5/§2.6）
  ('auto_review_site_preset', '',
   '站型审核预设（§2.5 出厂矩阵，0402 落表）：留空=通用缺省；填站型键即套用该站型覆盖。',
   'auto_review'),
  ('auto_review_title_template', '',
   '标题命名模板（§2.6），如 {中文名}.{年份}.{来源}.{分辨率}-{组名}。留空=不按模板校验顺序。',
   'auto_review'),
  ('auto_review_title_required', '',
   '严格模式下必备要素（逗号分隔，如：中文名,年份,分辨率）。留空=全部模板要素皆必备。',
   'auto_review'),
  ('auto_review_exe_whitelist_cats', '',
   '允许出现 exe/安装包的分类 ID（逗号分隔，D2 白名单）。',
   'auto_review'),
  -- 影子→执法门槛（§6）
  ('auto_review_shadow_min_samples', '200',
   '切执法前影子期最少样本数（也可用时间门槛：上线满 7 天）。',
   'auto_review'),
  ('auto_review_min_agree_pct', '85',
   '切执法前影子一致率门槛（机器与人工同判占比，%）。',
   'auto_review'),
  -- 熔断（§6：日推翻≥N 例自动降级半自动）
  ('auto_review_circuit_daily_overturn', '5',
   '单日人工推翻自动裁决达到该例数即自动降级为半自动并告警（0=关）。',
   'auto_review')
ON CONFLICT (name) DO NOTHING;

-- meta 成对（卡片顺序：1 总开关 10-19 / 2 预设 20 / 3 规则 30-59 /
--            4 词库名单 60-69 / 5 通知 70-79 / 6 站型 80-89 / 门槛熔断 90-99）
INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, unit, group_key, card_order,
   visible, options)
VALUES
  ('auto_review_enabled', 'yesno', '启用自动审种', 'Enable auto review',
   '开启后机器人审核员按下面规则评估待审种子。建议先开影子模式看几天摘要。',
   NULL, 'auto_review', 10, true, NULL),
  ('auto_review_mode', 'enum', '运行模式', 'Auto review mode',
   '影子=只记录不行动（安全）；半自动=只自动通过；全自动=自动通过+自动拒绝。',
   NULL, 'auto_review', 11, true,
   '[{"v":"shadow","l":"影子模式（只记录不行动）"},{"v":"half","l":"半自动（只自动通过）"},{"v":"full","l":"全自动（通过+拒绝）"}]'),
  ('auto_review_delay_sec', 'number', '评审延迟（秒）', 'Review delay (s)',
   '发布后等待多少秒再评审，给作者反悔修改的缓冲。', '秒',
   'auto_review', 12, true, NULL),
  ('auto_review_daily_cap', 'number', '每日自动裁决上限', 'Daily auto cap',
   '0 表示不限。防失控的总闸。', '个/天', 'auto_review', 13, true, NULL),
  ('auto_review_notify_admin', 'yesno', '给管理组发每日摘要', 'Daily digest',
   '每日一封汇总：自动通过/拒绝/暂缓数、与人工对照的一致率。', NULL,
   'auto_review', 14, true, NULL),
  ('auto_review_preset', 'enum', '一键预设', 'Preset',
   '选预设会批量覆盖本组的动作与阈值键；谨慎号几乎不自动拒，高效号面向大站量级。',
   NULL, 'auto_review', 20, true,
   '[{"v":"balanced","l":"均衡（出厂缺省）"},{"v":"cautious","l":"谨慎（几乎不自动拒）"},{"v":"efficient","l":"高效（激进减负）"},{"v":"off","l":"关闭（全部人工）"}]'),
  ('arule_title_garbled_enabled', 'yesno', '规则·标题乱码：启用', 'Rule: garbled title',
   '标题含乱码字符或夹藏编码串。', NULL, 'auto_review', 30, true, NULL),
  ('arule_title_garbled_action', 'enum', '规则·标题乱码：动作', 'Rule: garbled action',
   '发现后怎么办。', NULL, 'auto_review', 31, true,
   '[{"v":"deny","l":"自动拒绝"},{"v":"postpone","l":"只标记暂缓给我看"},{"v":"log","l":"仅记录不动手"}]'),
  ('arule_descr_missing_enabled', 'yesno', '规则·描述不完整：启用', 'Rule: thin descr',
   '描述去标签后过短。', NULL, 'auto_review', 33, true, NULL),
  ('arule_descr_missing_action', 'enum', '规则·描述不完整：动作', 'Rule: thin descr action',
   '缺省暂缓：小体积资源的说明天然短，先看影子数据。', NULL,
   'auto_review', 34, true,
   '[{"v":"deny","l":"自动拒绝"},{"v":"postpone","l":"只标记暂缓给我看"},{"v":"log","l":"仅记录不动手"}]'),
  ('arule_descr_missing_min_chars', 'number', '规则·描述不完整：最少字数', 'Min descr chars',
   '去标签后低于该字数即命中。', '字', 'auto_review', 35, true, NULL),
  ('arule_ads_keywords_enabled', 'yesno', '规则·广告引流：启用', 'Rule: ads',
   '描述命中下方广告词库。', NULL, 'auto_review', 36, true, NULL),
  ('arule_ads_keywords_action', 'enum', '规则·广告引流：动作', 'Rule: ads action',
   '命中词库即触发。', NULL, 'auto_review', 37, true,
   '[{"v":"deny","l":"自动拒绝"},{"v":"postpone","l":"只标记暂缓给我看"},{"v":"log","l":"仅记录不动手"}]'),
  ('arule_leak_marker_enabled', 'yesno', '规则·泄露站内信息：启用', 'Rule: leak marker',
   '描述含 passkey 段或本站 tracker 明文。', NULL, 'auto_review', 38, true, NULL),
  ('arule_leak_marker_action', 'enum', '规则·泄露站内信息：动作', 'Rule: leak action',
   '命中即触发（证据在通知里脱敏）。', NULL, 'auto_review', 39, true,
   '[{"v":"deny","l":"自动拒绝"},{"v":"postpone","l":"只标记暂缓给我看"},{"v":"log","l":"仅记录不动手"}]'),
  ('arule_dupe_check_enabled', 'yesno', '规则·疑似重复：启用', 'Rule: dupe',
   '与近期过审种标题高度相似。', NULL, 'auto_review', 40, true, NULL),
  ('arule_dupe_check_action', 'enum', '规则·疑似重复：动作', 'Rule: dupe action',
   '缺省暂缓：相似度有误报面，附原种链接人工定夺。', NULL,
   'auto_review', 41, true,
   '[{"v":"deny","l":"自动拒绝"},{"v":"postpone","l":"只标记暂缓给我看"},{"v":"log","l":"仅记录不动手"}]'),
  ('arule_dupe_check_threshold', 'number', '规则·疑似重复：相似度门槛（%）', 'Dupe similarity',
   '标题相似度超过该百分比算疑似重复。', '%', 'auto_review', 42, true, NULL),
  ('arule_title_format_enabled', 'yesno', '规则·命名规范：启用', 'Rule: title format',
   '按下方命名模板校验标题。', NULL, 'auto_review', 43, true, NULL),
  ('arule_title_format_action', 'enum', '规则·命名规范：动作', 'Rule: format action',
   '缺省暂缓：命名歧义多，机器附「建议标题」人工定夺。', NULL,
   'auto_review', 44, true,
   '[{"v":"deny","l":"自动拒绝（仅缺必备要素）"},{"v":"postpone","l":"只标记暂缓给我看"},{"v":"log","l":"仅记录不动手"}]'),
  ('arule_title_format_level', 'enum', '规则·命名规范：力度', 'Format level',
   '只提醒=不规范仅暂缓；严格=缺必备要素才计为可拒。', NULL,
   'auto_review', 45, true,
   '[{"v":"off","l":"不检查"},{"v":"warn","l":"只提醒"},{"v":"strict","l":"严格"}]'),
  ('auto_review_ad_words', 'textarea', '广告词库', 'Ad words',
   '一行一个词，命中即算广告（包含匹配，大小写不敏感）。', NULL,
   'auto_review', 60, true, NULL),
  ('auto_review_trusted_uploaders', 'text', '白名单 UID', 'Trusted uploaders',
   '逗号分隔：这些发布者跳过自动拒绝（仍可暂缓）。', NULL,
   'auto_review', 61, true, NULL),
  ('auto_review_watch_uploaders', 'text', '观察名单 UID', 'Watch uploaders',
   '逗号分隔：这些发布者只暂缓、不自动通过。', NULL,
   'auto_review', 62, true, NULL),
  ('auto_review_mail_pass_subject', 'text', '过审邮件主题', 'Pass mail subject',
   '变量：{site} {username} {torrent} {link}。', NULL,
   'auto_review', 70, true, NULL),
  ('auto_review_mail_deny_subject', 'text', '拒绝邮件主题', 'Deny mail subject',
   '变量：{site} {username} {torrent} {link} {reason}。', NULL,
   'auto_review', 71, true, NULL),
  ('auto_review_site_preset', 'text', '站型预设', 'Site-type preset',
   '填站型键（movie/music/anime/…）套用该站型出厂覆盖；留空=通用缺省。详见命名与站型说明。',
   NULL, 'auto_review', 80, true, NULL),
  ('auto_review_title_template', 'text', '标题命名模板', 'Title template',
   '要素拼装：{中文名}.{年份}.{来源}.{分辨率}-{组名}；留空=不校验顺序。',
   NULL, 'auto_review', 81, true, NULL),
  ('auto_review_title_required', 'text', '严格模式必备要素', 'Required elements',
   '逗号分隔（如：中文名,年份,分辨率）；留空=模板要素皆必备。', NULL,
   'auto_review', 82, true, NULL),
  ('auto_review_exe_whitelist_cats', 'text', '允许 exe 的分类 ID', 'Exe whitelist cats',
   '逗号分隔的分类 ID：这些分类允许出现可执行文件。', NULL,
   'auto_review', 83, true, NULL),
  ('auto_review_shadow_min_samples', 'number', '切执法最少影子样本', 'Shadow min samples',
   '影子期至少攒多少人工对照样本才建议切执法。', '例',
   'auto_review', 90, true, NULL),
  ('auto_review_min_agree_pct', 'number', '切执法一致率门槛（%）', 'Min agree pct',
   '影子一致率（机器与人工同判占比）达到才建议切执法。', '%',
   'auto_review', 91, true, NULL),
  ('auto_review_circuit_daily_overturn', 'number', '熔断：单日推翻例数', 'Circuit overturn',
   '单日人工推翻自动裁决达到该数即自动降级半自动并告警；0=关。', '例',
   'auto_review', 92, true, NULL)
ON CONFLICT (name) DO NOTHING;
