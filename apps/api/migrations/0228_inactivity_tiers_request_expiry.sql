-- 0228_inactivity_tiers_request_expiry.sql — P2 批三：不活跃策略档位化 + 求种过期
--
-- ① 不活跃三档（触点 #23）：mark（打标拦截，现状默认）/ demote（打标+降到最低
--    成长级）/ archive（打标+归档档案页隐藏）。档位切换不动已打标用户，只影响
--    dormant_mark job 的后续行为；归档 = profile_hidden 位，档案页 404 化。
-- ② 求种过期（对齐 UNIT3D AutoRecycleClaimedTorrentRequests，但我们的求种无
--    claim 环节——过期口径是「长期无人应种」）：requests.expires_days 天后自动
--    关闭 status=2（悬赏退还发起人，worker job 执行）。

-- ① 不活跃策略档位
INSERT INTO site_settings (name, value, grp)
VALUES ('inactivity_policy', 'mark', 'ops')
ON CONFLICT (name) DO NOTHING;
INSERT INTO settings_meta (name, type, label_zh, label_en, hint, options, group_key, card_order)
VALUES ('inactivity_policy', 'enum', '不活跃账号策略', 'Inactivity policy',
        'mark=仅打标（登录拦截提示联系管理组，现状缺省）；demote=打标并降至最低成长级（数据保留）；archive=打标并归档（档案页隐藏）。',
        '{"options":[{"value":"mark","label":"仅打标（缺省）"},{"value":"demote","label":"打标+降级"},{"value":"archive","label":"打标+归档"}]}'::jsonb,
        'ops', 4)
ON CONFLICT (name) DO NOTHING;

ALTER TABLE users ADD COLUMN IF NOT EXISTS archived BOOLEAN NOT NULL DEFAULT FALSE;

-- ② 求种过期天数（0=不过期，缺省）
INSERT INTO site_settings (name, value, grp)
VALUES ('request_expire_days', '0', 'torrent')
ON CONFLICT (name) DO NOTHING;
INSERT INTO settings_meta (name, type, label_zh, label_en, hint, unit, min, max, step, group_key, card_order)
VALUES ('request_expire_days', 'number', '求种过期天数', 'Request expiry (days)',
        '发布后 N 天无人应种的求种自动关闭，悬赏退还发起人；0=不过期（缺省）', '天', 0, 365, 1, 'torrent', 8)
ON CONFLICT (name) DO NOTHING;

-- ③ 通知通道第三轨（P2 触点 #22）：TG Bot / Discord webhook
--    webhook_discord：Discord 频道 webhook URL（POST JSON {content}）
--    tg_bot_token + tg_chat_id：Telegram Bot API（sendMessage）
--    用途：管理向重大事件（新申请/作弊告警/DLQ 积压）推到运维群；用户级
--    绑定走既有 tg_bind（0021）不动。
INSERT INTO site_settings (name, value, grp)
VALUES ('webhook_discord', '', 'ops')
ON CONFLICT (name) DO NOTHING;
INSERT INTO settings_meta (name, type, secret, label_zh, label_en, hint, group_key, card_order)
VALUES ('webhook_discord', 'password', true, 'Discord webhook', 'Discord webhook',
        'Discord 频道 webhook URL；重大事件（新申请/作弊告警）推送到该频道。留空=不启用', 'ops', 5)
ON CONFLICT (name) DO NOTHING;

INSERT INTO site_settings (name, value, grp)
VALUES ('tg_bot_token', '', 'ops')
ON CONFLICT (name) DO NOTHING;
INSERT INTO site_settings (name, value, grp)
VALUES ('tg_chat_id', '', 'ops')
ON CONFLICT (name) DO NOTHING;
INSERT INTO settings_meta (name, type, secret, label_zh, label_en, hint, group_key, card_order)
VALUES ('tg_bot_token', 'password', true, 'TG Bot token', 'TG Bot token',
        'Telegram Bot API token（@BotFather）；与 TG chat id 成对配置才启用', 'ops', 6)
ON CONFLICT (name) DO NOTHING;
INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order)
VALUES ('tg_chat_id', 'text', 'TG chat id', 'TG chat id',
        '接收运维通知的 chat id（@userinfobot 可查）', 'ops', 7)
ON CONFLICT (name) DO NOTHING;
