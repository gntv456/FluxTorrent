-- 0184：内容包登记表补 payload 列（二审 R11 配套）。
--
-- rules 包回滚需要知道「包写过哪些键」，只删包内键、不动管理员手工新增的
-- rule_% 键——payload 存进登记表作为回滚边界。存量行 payload 落 NULL
-- （旧包回滚按旧口径全删，仅影响 2 个银行利率键，可接受）。

ALTER TABLE content_packs ADD COLUMN IF NOT EXISTS payload JSONB;
