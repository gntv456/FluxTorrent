-- 0282：注册/邀请邮箱后缀策略（三模式）
--
-- 需求（2026-10-04）：站长可配置「哪些后缀的邮箱能注册」：
--   none        —— 不限制（缺省，兼容现状）
--   allow_list  —— 仅允许清单内后缀注册（白名单；临时邮箱治理用）
--   deny_list   —— 清单内后缀禁止注册（黑名单）
-- 作用点两处：① 注册（auth_http/register.rs）② 邀请邮件发送
-- （invite_http/email.rs——发到不允许的邮箱等于白发）。
-- 与既有 email_bans（精确邮箱/域名/前缀封禁，0032）分工：bans 是运营
-- 「个案封禁」，本策略是准入「整类后缀门」，两层都过才放行。
--
-- email_policy_list 语义：逗号分隔后缀，存 @domain 形态（如
-- '@qq.com,@163.com,@gmail.com'）；比对大小写不敏感、忽略空格。
-- settings_meta 用 [{v,l}] 结构化选项（0275 形态 1），保存存 v。

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('email_policy', 'none',
        '注册邮箱后缀策略：none=不限制 / allow_list=仅允许清单后缀 / deny_list=清单后缀禁止',
        'security')
ON CONFLICT (name) DO NOTHING;

INSERT INTO site_settings (name, value, descr, grp)
VALUES ('email_policy_list', '',
        '邮箱后缀清单（逗号分隔，@ 开头，如 @qq.com,@gmail.com；策略为 none 时不生效）',
        'security')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta
  (name, type, label_zh, label_en, hint, options, group_key, card_order,
   visible)
VALUES
  ('email_policy', 'enum', '注册邮箱后缀策略',
   'Registration email suffix policy',
   '控制哪些邮箱后缀可以注册：不限制 = 维持现状；仅允许清单后缀 = 白名单模式，'
   '只有清单里的后缀能注册（挡临时邮箱厂）；清单后缀禁止 = 黑名单模式，'
   '清单里的后缀不能注册。清单在下一项「邮箱后缀清单」里填。',
   '[{"v":"none","l":"不限制"},{"v":"allow_list","l":"仅允许清单后缀"},
     {"v":"deny_list","l":"清单后缀禁止"}]'::jsonb,
   'security', 91, true),
  ('email_policy_list', 'text', '邮箱后缀清单',
   'Email suffix list',
   '逗号分隔、每项 @ 开头（如 @qq.com,@gmail.com）。仅在「注册邮箱后缀策略」'
   '选择白名单或黑名单时生效；大小写不敏感。',
   NULL, 'security', 92, true)
ON CONFLICT (name) DO NOTHING;
