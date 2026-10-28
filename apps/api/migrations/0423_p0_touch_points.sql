-- 0423: 产品分析 P0 批（2026-10-26）——三件结构化收尾。
--
-- ① 发布者正反馈：notice_prefs 白名单扩 2 键（torrent_comment /
--    torrent_thanks）。种子被评论/被感谢此前零通知（论坛侧全齐、种子侧
--    双标），发布是 PT 核心 UGC，缺「有人谢你」回路直接削弱发布留存。
--    u_notice_enabled()（0076）按 JSONB 键查值、缺省=发，无需改函数；
--    api 白名单（notice.rs 两处 16 键数组）与 web 词典四语言同步扩。
-- ② 促销触达：promotions 加 announced_at。促销上线（手工/预约/魔法池
--    自动）此前零通知——promo push topic 在跑但生产者只有愿望单，每次
--    大促的唤醒势能全部浪费。worker 统一播报（见 promos.rs）以
--    announced_at 幂等，站点级促销全员站内信 + push_outbox。
-- ③ 新手保护期：newbie_grace_days 站点设定（注册 N 天内完成的下载
--    不建 H&R 快照——不是延期而是真豁免：新人第一周装客户端、摸索
--    做种习惯，此时落下的 H&R 违规几乎全是环境性误伤）。0=关（出厂）。
--    消费点在 worker hr_snapshot.rs；0226 从严模板应用时写 7。

-- ① promotions 播报幂等锚
ALTER TABLE promotions
    ADD COLUMN IF NOT EXISTS announced_at TIMESTAMPTZ;

-- ③ 新手保护期设定键（出厂 0=关；从严模板应用时置 7，宽进模板不写=关）
INSERT INTO site_settings (name, value, grp)
VALUES ('newbie_grace_days', '0', 'torrent')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, unit,
                           min, max, step, group_key, card_order)
VALUES ('newbie_grace_days', 'number', '新手保护期', 'Newbie grace days',
        '注册后 N 天内完成的下载不计 H&R（帮助新人度过装客户端、学做种的第一周；0=关闭）',
        '天', 0, 90, 1, 'torrent', 7)
ON CONFLICT (name) DO NOTHING;
