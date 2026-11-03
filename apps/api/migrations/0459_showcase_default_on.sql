-- 0459: module_showcase 缺省翻转 no → yes。
--
-- 背景（2026-11-03 全站深测）：showcase 模块键挂的是首页「最新资源」海报墙
-- （apps/web page.tsx showcaseOn），而 home_sections 排版默认**包含** latest
-- （in_default=true）。两层配置自相矛盾：缺省关 showcase 时，全新安装/未改
-- 过开关的站点首页空着一大块，看起来像坏站。
--
-- 历史语义：showcase 最初是「主页展示权」（街机奖励 cosmetic，0244 目录仍在，
-- 不受影响）；0178 把它划入「教育/重度娱乐中立矩阵」一并缺省关，是语义误挂
-- ——首页核心资源列表不属于重度娱乐。0209 让键接真（海报墙受控）后矛盾暴露。
--
-- 修法：三处同步——
--   1) api modules.rs DEFAULT_OFF 移出 showcase（代码缺省）；
--   2) worker locks.rs module_on 兜底清单同步移出；
--   3) 本迁移把存量值守卫式翻回 yes（站长后来显式关过的实例：2026-10-25
--      0178 之后未再动过的 no 才翻，改过 timestamp 的尊重站长）。
-- 要关海报墙的正路 = 后台排版编辑把 latest 移出 home_layout。

-- 存量守卫式翻转：仅翻「值仍为 no 且时间戳还是 0178 批量落库时刻」的行。
-- 0178 之后的显式修改会更新 updated_at，被 WHERE 挡住不动。
UPDATE site_settings
   SET value = 'yes', updated_at = now()
 WHERE name = 'module_showcase'
   AND value = 'no'
   AND updated_at = '2026-10-25 18:05:08.240708+00';

-- 站型包矩阵：对齐「首页资源列表是核心面」的口径——除 education（教育站
-- 0178 原矩阵 showcase=false，站长切包显式选的形态）外，其余包统一为开。
UPDATE site_type_packs
   SET modules = jsonb_set(modules, '{showcase}', 'true'::jsonb)
 WHERE code IN ('general','anime','ebook','game','lossless','music',
                'software','sports')
   AND modules->>'showcase' = 'false';
