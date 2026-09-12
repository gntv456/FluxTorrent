-- 0048 银行全功能对齐火花银行（2026-09-12）
-- 新增：活期账户（复利结息）、贷款（计息/一次性结清/逾期罚息/自动扣款）、利息明细表。
-- 定期沿用 bank_deposits，补 type 列区分活期/定期（活期走独立账户表，此列留作口径对齐）。
-- 口径说明：站内利率统一为「日利率万分比」模型（daily_rate_bp bigint，1bp = 0.01%/日），
--           整数火花防浮点误差，与 spark_ledger 原语一致。

CREATE TABLE IF NOT EXISTS bank_demand_accounts (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL UNIQUE REFERENCES users(id),
  balance BIGINT NOT NULL DEFAULT 0 CHECK (balance >= 0),
  daily_rate_bp INT NOT NULL DEFAULT 1,          -- 活期日利率（万分比，默认 0.01%/日）
  last_interest_date DATE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_bank_demand_interest_due
  ON bank_demand_accounts (last_interest_date) WHERE balance > 0;

CREATE TABLE IF NOT EXISTS bank_loans (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),
  amount BIGINT NOT NULL,                        -- 放款本金
  daily_rate_bp INT NOT NULL,                    -- 计息日利率（万分比）
  penalty_rate_bp INT NOT NULL DEFAULT 50,       -- 逾期罚息日利率（万分比，默认 0.50%/日）
  term_days INT NOT NULL,
  remaining BIGINT NOT NULL,                     -- 剩余本金（罚息并入）
  accrued_interest BIGINT NOT NULL DEFAULT 0,    -- 已计提未结清利息（不并本金）
  status TEXT NOT NULL DEFAULT 'active',         -- active | paid | defaulted
  due_at TIMESTAMPTZ NOT NULL,
  last_interest_date DATE,
  paid_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_bank_loans_user ON bank_loans (user_id);
CREATE INDEX IF NOT EXISTS idx_bank_loans_status_due ON bank_loans (status, due_at);
CREATE UNIQUE INDEX IF NOT EXISTS uq_bank_loans_one_active_per_user
  ON bank_loans (user_id) WHERE status = 'active';

CREATE TABLE IF NOT EXISTS bank_interest_records (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),
  kind TEXT NOT NULL,                            -- demand | fixed | loan | loan_penalty
  reference_id BIGINT NOT NULL,                  -- 活期账户 id / bank_deposits id / bank_loans id
  amount BIGINT NOT NULL,                        -- 正=应收利息入账，负=利息支出
  rate_bp INT NOT NULL,
  calc_date DATE NOT NULL DEFAULT (CURRENT_DATE),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_bank_interest_user_date ON bank_interest_records (user_id, calc_date DESC);
CREATE INDEX IF NOT EXISTS idx_bank_interest_ref ON bank_interest_records (kind, reference_id);

-- 定期存款补支取手续费与计息游标（每日结息口径预留；当前仍到期一次性还本付息）
ALTER TABLE bank_deposits
  ADD COLUMN IF NOT EXISTS penalty BIGINT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS withdrawn_at TIMESTAMPTZ;

-- 站点设置：银行参数（走 settings_meta 已有渲染管线；visible=false 仅程序读）
INSERT INTO site_settings (name, value) VALUES
  ('bank_enabled', 'true'),
  ('bank_min_deposit', '100'),            -- 定期单笔最小（火花）
  ('bank_max_deposit', '1000000'),        -- 定期单笔最大（0=不限）
  ('bank_min_demand', '100'),             -- 活期单笔最小
  ('bank_loan_ratio', '100'),             -- 最大可贷 = 时魔/小时 × 系数 + 常数
  ('bank_loan_ratio_constant', '1000'),
  ('bank_min_loan', '100'),
  ('bank_demand_rate_bp', '1'),           -- 活期 0.01%/日
  ('bank_penalty_rate_bp', '50'),         -- 定期提前支取 0.50%（万分比）
  ('bank_overdue_penalty_bp', '50'),      -- 贷款逾期罚息 0.50%/日
  ('bank_auto_deduct_days', '7'),
  ('bank_allow_negative', 'false')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, group_key, card_order, visible, min_class)
VALUES
  ('bank_enabled', 'yesno', '启用银行系统', 'Enable bank', 'economy', 50, true, 60),
  ('bank_min_deposit', 'number', '定期单笔最小金额', 'Min fixed deposit', 'economy', 51, true, 60),
  ('bank_max_deposit', 'number', '定期单笔最大金额（0 不限）', 'Max fixed deposit', 'economy', 52, true, 60),
  ('bank_min_demand', 'number', '活期单笔最小金额', 'Min demand deposit', 'economy', 53, true, 60),
  ('bank_loan_ratio', 'number', '贷款额度系数（×时魔/小时）', 'Loan ratio', 'economy', 54, true, 60),
  ('bank_loan_ratio_constant', 'number', '贷款额度常数', 'Loan constant', 'economy', 55, true, 60),
  ('bank_min_loan', 'number', '贷款最小金额', 'Min loan', 'economy', 56, true, 60),
  ('bank_demand_rate_bp', 'number', '活期日利率（万分比）', 'Demand daily rate bp', 'economy', 57, true, 60),
  ('bank_penalty_rate_bp', 'number', '定期提前支取手续费（万分比）', 'Early withdrawal penalty bp', 'economy', 58, true, 60),
  ('bank_overdue_penalty_bp', 'number', '贷款逾期罚息日利率（万分比）', 'Overdue penalty bp', 'economy', 59, true, 60),
  ('bank_auto_deduct_days', 'number', '逾期自动扣款天数', 'Auto deduct days', 'economy', 60, true, 60),
  ('bank_allow_negative', 'yesno', '逾期扣款允许负余额', 'Allow negative balance', 'economy', 61, true, 60)
ON CONFLICT (name) DO NOTHING;
