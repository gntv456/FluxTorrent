-- 0236_email_verify.sql — C7-#4：注册后邮件激活通道（真实发信版）
--
-- 背景：0208 把 email_verify 从 registration_mode 枚举移除（旧实现不发信不拦
-- 登录=空壳）。本迁移回补**真实实现**：registration_mode 第三档 email_verify，
-- 注册成功 → 发验证信 → 未验证登录拦截 → 点信内链接激活。邀请制站不启用。
--
-- 设计：token 一次性（SHA3-256 落库，明文只在邮件里）；48h 过期；验证成功
-- 行保留（审计「何时激活」），重复验证幂等。不占用 users.status（那是
-- 禁言/封禁/软删的领域值），未验证态用 users.email_verified_at IS NULL 表达，
-- 仅当 registration_mode=email_verify 时登录链路才检查该列。

-- 1) users 补激活时间列（NULL = 未验证；存量用户视为已验证——回填 now()）
ALTER TABLE users ADD COLUMN IF NOT EXISTS email_verified_at timestamptz;
UPDATE users SET email_verified_at = created_at WHERE email_verified_at IS NULL;

-- 2) 验证 token 表（一次性；48h 过期由消费端判定）
CREATE TABLE IF NOT EXISTS email_verifications (
    id          bigserial PRIMARY KEY,
    user_id     bigint NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    token_hash  text NOT NULL UNIQUE,          -- sha3-256(token)，明文不落库
    expires_at  timestamptz NOT NULL,
    created_at  timestamptz NOT NULL DEFAULT now(),
    verified_at timestamptz                    -- 激活时间（审计保留）
);
CREATE INDEX IF NOT EXISTS email_verifications_user_idx
    ON email_verifications (user_id, id);

-- 3) registration_mode 回补 email_verify 档（值行 0208 已在，只改 meta options）
--    顺序铁律：值行先于登记行（settings_meta FK 指向 site_settings）。
INSERT INTO site_settings (name, value, grp)
VALUES ('registration_mode', 'invite_only', 'basic')
ON CONFLICT (name) DO NOTHING;

UPDATE settings_meta
SET options = '{"options":["invite_only","open","email_verify"]}'::jsonb,
    hint = COALESCE(hint, '') ||
        '；email_verify=开放注册+邮件激活：注册后发验证信，未验证账号登录被拦',
    updated_at = now()
WHERE name = 'registration_mode';
