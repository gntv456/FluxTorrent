-- 0200：删掉 modules.is_on —— 模块开关从此只有一个真值 site_settings.module_<key>
--
-- 起因：四审 0196 落地时把后台面板过滤写成
--   `module_key IN (SELECT key FROM modules WHERE is_on)`
-- 看着合理，读的却是**注册表种子值**：站长在后台关模块改的是 site_settings，
-- is_on 永远不变 ⇒ 面板条目不消失（同类误读几乎必然会再发生，除非把那列删掉）。
--
-- 删列前核实：代码里唯一访问 modules 表的语句是 require_known_module 的
--   `SELECT EXISTS(SELECT 1 FROM modules WHERE key = $1)`，不碰 is_on；
--   pg_proc / pg_views 里没有任何函数体或视图引用它；
--   读不到开关行时的回落走 Rust 侧 default_on()（general 中立矩阵，0178 定）。
-- 历史迁移（0107 建表、0178/0179 翻默认、0197 加键、0198 补开关行）都排在本迁移之前，
-- 它们读写 is_on 时列还在，因此不改写任何历史文件；空库按序执行同样成立。

DO $$
DECLARE
    missing int;
BEGIN
    SELECT count(*) INTO missing
    FROM modules m
    WHERE NOT EXISTS (
        SELECT 1 FROM site_settings s WHERE s.name = 'module_' || m.key
    );
    IF missing > 0 THEN
        RAISE NOTICE '有 % 个模块键没有 site_settings 开关行：删列后按 default_on() 回落',
            missing;
    END IF;
END $$;

ALTER TABLE modules DROP COLUMN IF EXISTS is_on;
