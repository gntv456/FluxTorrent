-- 0227_captcha_drivers_webauthn.sql — C7+ P2 批一：可插拔验证码 + passkey
--
-- ① captcha_provider：none（缺省，沿用自研算术题）| turnstile | recaptcha | hcaptcha
--    注册链路在第三方模式下改走「前端拿 token → 后端 siteverify」；
--    site 密钥走 settings_meta 的 secret 列（GET 回掩码）。
-- ② passkey（WebAuthn）：注册凭证表。流程为无库依赖的最小实现——
--    challenge 存 Redis 5 分钟（与图形验证码同口径），凭证公钥落本表。
--    用户可在 usercp 绑定/解绑；登录页可作为第二通道（用户名 + passkey 断言）。

-- ① 验证码驱动三键
INSERT INTO site_settings (name, value, grp)
VALUES ('captcha_provider', 'none', 'main')
ON CONFLICT (name) DO NOTHING;
INSERT INTO site_settings (name, value, grp)
VALUES ('captcha_site_key', '', 'main')
ON CONFLICT (name) DO NOTHING;
INSERT INTO site_settings (name, value, grp)
VALUES ('captcha_secret', '', 'main')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, options, group_key, card_order)
VALUES ('captcha_provider', 'enum', '验证码驱动', 'Captcha provider',
        'none=自研算术题（缺省零依赖）；turnstile=Cloudflare Turnstile；recaptcha=Google reCAPTCHA v2；hcaptcha=hCaptcha。第三方模式需同时配站点键与密钥，注册页将改用对应组件。',
        '{"options":[{"value":"none","label":"自研算术题"},{"value":"turnstile","label":"Cloudflare Turnstile"},{"value":"recaptcha","label":"reCAPTCHA v2"},{"value":"hcaptcha","label":"hCaptcha"}]}'::jsonb,
        'main', 31)
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order)
VALUES ('captcha_site_key', 'text', '验证码站点键（site key）', 'Captcha site key',
        '第三方验证码的控制台站点键（公开值）', 'main', 32)
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, secret, label_zh, label_en, hint, group_key, card_order)
VALUES ('captcha_secret', 'password', true, '验证码密钥（secret）', 'Captcha secret',
        '第三方验证码的 siteverify 密钥（回显掩码，留空保持不变）', 'main', 33)
ON CONFLICT (name) DO NOTHING;

-- ② passkey 凭证表（一个用户可绑多枚：平台认证器 + 安全钥匙）
CREATE TABLE IF NOT EXISTS user_passkeys (
    id              BIGSERIAL PRIMARY KEY,
    user_id         BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    label           TEXT NOT NULL DEFAULT '',          -- 用户起的名（如「笔记本」）
    cred_id         TEXT NOT NULL UNIQUE,              -- base64url(credential id)
    public_key      BYTEA NOT NULL,                    -- COSE 格式公钥
    sign_count      BIGINT NOT NULL DEFAULT 0,         -- 克隆检测基线
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at    TIMESTAMPTZ
);
CREATE INDEX IF NOT EXISTS idx_user_passkeys_user ON user_passkeys (user_id);

-- ③ 通知偏好（P2 触点 #22）：三开关缺省全开（与现状一致，零行为变化）
ALTER TABLE users ADD COLUMN IF NOT EXISTS notify_reply BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS notify_system BOOLEAN NOT NULL DEFAULT TRUE;
ALTER TABLE users ADD COLUMN IF NOT EXISTS notify_marketing BOOLEAN NOT NULL DEFAULT TRUE;

-- ④ 申请制入站（P2 触点 #3）：applications 表 + 设定开关
CREATE TABLE IF NOT EXISTS applications (
    id              BIGSERIAL PRIMARY KEY,
    username        TEXT NOT NULL,
    email           TEXT NOT NULL,
    proof_images    TEXT NOT NULL DEFAULT '',           -- 附件路径，逗号分隔
    proof_links     TEXT NOT NULL DEFAULT '',           -- 他站主页等佐证链接，换行分隔
    reason          TEXT NOT NULL DEFAULT '',
    status          SMALLINT NOT NULL DEFAULT 0 CHECK (status IN (0,1,2)),  -- 0待审 1通过 2拒绝
    handled_by      BIGINT REFERENCES users(id) ON DELETE SET NULL,
    handled_at      TIMESTAMPTZ,
    decide_note     TEXT,                               -- 审批备注（拒绝理由会随邀请信发出）
    invite_code     TEXT,                               -- 通过时生成的邀请码（与 invites 联动）
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_applications_status ON applications (status) WHERE status = 0;

INSERT INTO site_settings (name, value, grp)
VALUES ('application_signup', 'off', 'ops')
ON CONFLICT (name) DO NOTHING;
INSERT INTO settings_meta (name, type, label_zh, label_en, hint, options, group_key, card_order)
VALUES ('application_signup', 'enum', '申请制入站', 'Application signups',
        'off=关闭（缺省）；open=开放申请通道（/apply 提交举证，管理员后台人审，通过自动发邀请码）。',
        '{"options":[{"value":"off","label":"关闭"},{"value":"open","label":"开放"}]}'::jsonb,
        'ops', 3)
ON CONFLICT (name) DO NOTHING;

-- ⑤ staff_panel 挂「入站申请」入口（users 分区；admin 可见）
INSERT INTO staff_panel_entries (panel, name, url, info, sort, section, min_class, tab_key) VALUES
('admin', '入站申请', '/admin?tool=applications', '申请制入站人审（通过自动发绑定邮箱邀请码）', 63, 'users', 92, 'applications')
ON CONFLICT DO NOTHING;
