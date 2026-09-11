-- 0035 捐赠中心（馒头 donate 口径：储值钱包 + 套餐订购 + VIP）
-- 套餐档位（plan_type: quota / upload / vip）
CREATE TABLE IF NOT EXISTS donation_plans (
    id          SERIAL PRIMARY KEY,
    plan_type   TEXT NOT NULL CHECK (plan_type IN ('quota', 'upload', 'vip')),
    title       TEXT NOT NULL,           -- 如「10 个片单额度」「100 GB 上传量」「30 天」
    reward      TEXT,                    -- 附赠说明（如「获赠邀请 1」）
    price_usd   NUMERIC(10,2) NOT NULL,
    sort        INT NOT NULL DEFAULT 0,
    enabled     BOOLEAN NOT NULL DEFAULT true
);

INSERT INTO donation_plans (id, plan_type, title, reward, price_usd, sort)
SELECT * FROM (VALUES
  (1, 'quota',  '10 个片单额度', NULL, 10.00, 1),
  (2, 'upload', '100 GB 上传量', NULL, 20.00, 2),
  (3, 'upload', '500 GB 上传量', NULL, 50.00, 3),
  (4, 'vip',    '30 天',   '获赠邀请 1', 30.00, 4),
  (5, 'vip',    '180 天',  '获赠邀请 1', 160.00, 5),
  (6, 'vip',    '终身用户', '获赠邀请 1', 520.00, 6)
) AS seed(id, plan_type, title, reward, price_usd, sort)
WHERE NOT EXISTS (SELECT 1 FROM donation_plans);
SELECT setval('donation_plans_id_seq', GREATEST((SELECT max(id) FROM donation_plans), 1));

-- 充值/消费流水（kind: topup / order）
CREATE TABLE IF NOT EXISTS donation_ledger (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    kind        TEXT NOT NULL CHECK (kind IN ('topup', 'order')),
    amount_usd  NUMERIC(10,2) NOT NULL,        -- topup 正 / order 负
    balance_after NUMERIC(10,2) NOT NULL,
    plan_id     INT REFERENCES donation_plans(id),
    note        TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 用户钱包 + VIP（users 扩列）
ALTER TABLE users
    ADD COLUMN IF NOT EXISTS wallet_usd NUMERIC(10,2) NOT NULL DEFAULT 0,
    ADD COLUMN IF NOT EXISTS vip_until TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS quota_extra INT NOT NULL DEFAULT 0;
