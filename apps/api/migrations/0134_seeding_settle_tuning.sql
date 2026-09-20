-- 0134 做种结算的两处收尾：① 结算 JOIN 的部分索引；② 函数注释澄清（ln 与 log10 等价）
--
-- 背景（0133 落地后的自查）：
--   1) 每小时结算扫 `snatches WHERE seeding`，而 snatches 只有主键 (user_id, torrent_id)，
--      做种行占比很小却要全表扫。加**部分索引**只覆盖做种行 —— 索引体积小、announce 写入代价低。
--   2) 0133 的函数体用的是 `ln(...)/ln(1+vol_base)`，注释里写的是 log10 —— 两者比值完全等价
--      （换底公式），但字面不一致会让后人怀疑口径漂移。这里用 COMMENT 说明清楚。
--   3) 另记录调用注意：`seeding_params()` 是 STABLE 函数，调用侧务必
--      `WITH p AS MATERIALIZED (SELECT * FROM seeding_params())`，否则优化器可能把它内联进
--      nestloop 内层 → 每颗种子读 8 次 site_settings。

CREATE INDEX IF NOT EXISTS idx_snatches_seeding ON snatches (user_id) WHERE seeding;

COMMENT ON FUNCTION seeding_torrent_bonus(
    bigint, int, double precision, int, double precision, double precision, double precision, double precision, double precision
) IS '单颗种子的做种收益加成系数（纯函数）。
公式 =（种子档位分 + 个人做种时长档位分）× 体积对数饱和因子 × 稀有度加成 × 标定系数。
体积因子实现用 ln(1+size_GB)/ln(1+vol_base)——与 log10 比值完全等价（换底公式），非口径差异。
档位：种子维度 濒危2.0/高龄1.5/老1.0/大体积0.75/中体积0.5/日常0.25（第一命中优先）；
个人时长 ≥1年2.0/6-12月1.0/3-6月0.75/1-3月0.5。参数全部来自 site_settings seeding_*（见 seeding_params()）。';

COMMENT ON FUNCTION seeding_hourly(
    double precision, bigint, double precision, double precision, bigint, boolean
) IS 'Σ加成 → 每小时魔力。底薪不参与 arctan 压缩曲线；donor 整笔（含底薪）乘 donor_mult。
渐近上限 = base + cap（atan 开区间，实际取不到，故实际恒 < base+cap）。';

COMMENT ON FUNCTION seeding_params() IS '做种收益参数行（从 site_settings 的 seeding_* 键读取）。
调用侧应使用 WITH p AS MATERIALIZED (SELECT * FROM seeding_params())，避免被内联后逐行求值。';
