-- 0206：站型包携带术语段（0205 的配套，四审 L2「另存快照丢载荷」的同族预防）。
--
-- 0193/0197 那一批已经证明：站型包的每一段载荷都必须**显式**被快照采集与 apply
-- 覆盖，漏一段就是「站长自建的东西另存一次就没」。术语表是站长自建内容，
-- 所以它进包的代价必须和 sections/tags/classes/economy/metadata 完全一致。
--
-- 口径沿用 pack_core 的约定：**NULL = 该包未声明这一维度，apply 时不动**；
-- 数组（含空数组）= 显式声明，apply 时全量覆盖。空数组是有意义的声明
-- （「本站就是要清空术语」），不能与 NULL 混用。
ALTER TABLE site_type_packs
    ADD COLUMN IF NOT EXISTS terms JSONB;

COMMENT ON COLUMN site_type_packs.terms IS
    '术语规则数组 [{canonical,replacement,sort}]；NULL = 本包不声明术语';
