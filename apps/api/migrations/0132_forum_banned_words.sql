-- 0132 论坛 Phase3（第一片）：敏感词 + 搜索增强的配置位
--
-- 敏感词词表存 site_settings.forum_banned_words（换行分隔）——不建新表：
-- 词表是纯运营配置，settings 体系已有 CRUD/UI/审计/分组（settings_meta 是后台界面目录）。
-- 空 = 不启用拦截（默认，存量站点零影响）。
-- 写入口径：发主题 / 回帖 / 编辑三处共用同一闸门（community_http.rs check_banned_words），
-- 命中即 422 拒发（先挡再落库，不做「发后删」——发后删会走通知链造成骚扰）。
-- 搜索增强（标题+正文 body_text+摘要片段）是纯代码改动，无表结构变化，不需要本迁移做什么。

INSERT INTO site_settings (name, value) VALUES ('forum_banned_words', '')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order, min_class, visible) VALUES
  ('forum_banned_words', 'text', '论坛敏感词（换行分隔）', 'Forum banned words (newline separated)', '发主题/回帖/编辑命中即拒发；留空 = 不启用（默认）。每行一个词，至少 2 个字符生效。', '论坛', 1, 99, true)
ON CONFLICT (name) DO NOTHING;
