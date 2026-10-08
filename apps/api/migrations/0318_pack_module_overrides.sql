-- 0318_pack_module_overrides.sql
-- 站型成熟度批（批次 1 · H4）：模块开关的手工覆盖位。
--
-- 现状：apply 无条件把包里 31 个 module_* 全部写一遍（pack_core.rs），
-- 站长手工关掉的模块（「我只想做音乐站，别把游戏区打开」）在下次切站型时
-- 被无声复位。tagline 已有「站长自定义值保留」守卫（0145），module_* 缺同款。
--
-- 方案：site_settings 增加零侵入的登记表 `pack_module_overrides`：
--   module_key → 唯一登记行。任何「站长通过设置面手工写 module_*」的路径
--   在落库时顺手 UPSERT 一行；站型包 apply 撞见登记行就跳过该键
--   （包值与现值相同也照登记——覆盖语义以「人来过」为准，不猜意图）。
--   回滚（restore 快照重放）不受限：快照是站长状态的忠实还原。
--
-- 为什么不用 site_settings 加列：那张表被 settings 包导入/导出按 (name,value)
-- 全量搬运，加列会破坏十处既有载荷契约；独立小表零波及。

BEGIN;

CREATE TABLE IF NOT EXISTS pack_module_overrides (
    module_key TEXT PRIMARY KEY,          -- 不带 module_ 前缀的裸键（如 games）
    set_by INT REFERENCES users(id) ON DELETE SET NULL,
    set_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE pack_module_overrides IS
    '站长手工设置过的模块开关登记：站型包 apply 跳过这些键（H4 覆盖语义）';

COMMIT;
