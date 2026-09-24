-- 0169: 生态商店 M3 —— 规则包（受限表达式）设置键（策划案 §4.2 / §8 M3）
--
-- 规则键命名：rule_<域>_<名>。值为受限表达式（rules_engine lint 白名单：
-- 算术/比较/min/max，变量白名单按规则声明）。空 = 用内置硬编码默认
-- （T3 缺省=现状：存量站零感知）。求值失败/越域回落默认值并告警（A4）。
--
-- 首批两键（银行利率，变量 term_days）：
--   rule_bank_term_rate     年化利率，域 [0,1]，默认阶梯 7→1% 30→3% 90→6% 180→10% 365→18%
--   rule_bank_loan_daily    贷款日利率（小数），域 [0,0.01]，默认阶梯 7→8bp 30→12bp ... 365→22bp
-- 规则包（kind=rules）经内容包导入路径落这些键：lint 通过才落库（预检与确认两态都校验）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('rule_bank_term_rate', '', '定期年化利率公式（变量 term_days，空=内置阶梯）', 'module'),
('rule_bank_loan_daily', '', '贷款日利率公式（变量 term_days，空=内置阶梯）', 'module')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, group_key, card_order) VALUES
('rule_bank_term_rate', 'text', '定期年化利率公式（变量 term_days，空=内置阶梯）', 'Term rate formula', 'module', 96),
('rule_bank_loan_daily', 'text', '贷款日利率公式（变量 term_days，空=内置阶梯）', 'Loan daily rate formula', 'module', 97)
ON CONFLICT (name) DO UPDATE
  SET label_zh = EXCLUDED.label_zh, label_en = EXCLUDED.label_en,
      group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;

-- 登记表 kind 枚举扩展：rules（0167 只允许 taxonomy/theme）
ALTER TABLE content_packs DROP CONSTRAINT IF EXISTS content_packs_kind_check;
ALTER TABLE content_packs ADD CONSTRAINT content_packs_kind_check
  CHECK (kind IN ('taxonomy', 'theme', 'rules'));
