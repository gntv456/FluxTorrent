-- 0444: 制作组（team）去掉语义位 官方/自制/转载，补本站官组 ZmWeb/FT。
--
-- 背景（2026-10-31 站长定调）：0443 把 team 收敛为 PT 官组后缀时保留了
-- 「官方/自制/转载」三个通用语义位；站长反馈制作组下拉里只该是组名，
-- 语义位会被误选成「制作组 = 官方」。另外本站需要 ZmWeb / FT 两个缩写。
--
-- 引用体检（2026-10-31 现网）：三行删除前引用均为 0。仍按 0443 的顺序
-- 先清 torrent_sections 再删字典行（该表对 section_dict 是 CASCADE）。
-- 幂等：DELETE 按名；INSERT 走 WHERE NOT EXISTS；包同步可重放。

-- ① 删语义位（先清引用，再删字典行）
DELETE FROM torrent_sections
 WHERE kind = 'team' AND dict_id IN (SELECT id FROM section_dict
   WHERE kind = 'team' AND name IN ('官方', '自制', '转载'));

DELETE FROM section_dict
 WHERE kind = 'team' AND name IN ('官方', '自制', '转载');

-- ② 补本站官组缩写（排在 HX 系之后、WiKi 之前）
INSERT INTO section_dict (kind, name, sort)
SELECT 'team', v.n, v.s FROM (VALUES
  ('ZmWeb', 8), ('FT', 9)
) AS v(n, s)
WHERE NOT EXISTS (SELECT 1 FROM section_dict d
                   WHERE d.kind = 'team' AND d.name = v.n);

-- ③ 站型包同步（重放/切站型不得回灌旧词表）
UPDATE site_type_packs p
   SET sections = jsonb_set(
         p.sections, '{dict,team}',
         (SELECT jsonb_agg(d.name ORDER BY d.sort, d.id)
            FROM section_dict d WHERE d.kind = 'team'),
         TRUE)
 WHERE p.sections ? 'dict';
