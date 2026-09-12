-- 0049 银行二期（2026-09-12）：定期每日结息模式 + 结息健康游标 + 站点概览支撑
-- settle_mode：'maturity' = 到期一次性还本付息（默认，旧口径）；'daily' = 每日结息发到余额，到期只还本。
-- 选择 daily 后 interest 列转为「全期理论利息」参考值，实发以 paid_interest 累计为准。

ALTER TABLE bank_deposits
  ADD COLUMN IF NOT EXISTS settle_mode TEXT NOT NULL DEFAULT 'maturity'
    CHECK (settle_mode IN ('maturity', 'daily')),
  ADD COLUMN IF NOT EXISTS paid_interest BIGINT NOT NULL DEFAULT 0,
  ADD COLUMN IF NOT EXISTS last_interest_date DATE;

CREATE INDEX IF NOT EXISTS idx_bank_deposits_interest_due
  ON bank_deposits (last_interest_date)
  WHERE status = 0 AND settle_mode = 'daily';

-- 结息调度健康游标：worker 每次成功跑完 bank_daily upsert 当日记录，页面读它展示「结息状态」。
CREATE TABLE IF NOT EXISTS bank_settle_runs (
  run_date DATE PRIMARY KEY,
  demand_rows INT NOT NULL DEFAULT 0,
  fixed_rows INT NOT NULL DEFAULT 0,
  loan_rows INT NOT NULL DEFAULT 0,
  deduct_rows INT NOT NULL DEFAULT 0,
  finished_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 结息模式开关 + 健康展示开关（后台可配；默认仍为到期一次性口径）
INSERT INTO site_settings (name, value) VALUES
  ('bank_fixed_settle_mode', 'maturity')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, group_key, card_order, visible, min_class)
VALUES
  ('bank_fixed_settle_mode', 'enum', '定期结息模式（maturity=到期一次 / daily=每日结息）', 'Fixed settle mode', 'economy', 62, true, 60)
ON CONFLICT (name) DO NOTHING;

UPDATE settings_meta SET options = '{"maturity": "到期一次性还本付息", "daily": "每日结息，到期还本"}'::jsonb
WHERE name = 'bank_fixed_settle_mode';
