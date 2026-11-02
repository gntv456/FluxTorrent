-- 0457: 电子书包「杂志 / 漫画」的对外科号回到 7010 / 7030。
--
-- 0455 重写站型包 payload 时，把这两行连同其它电子书分类一起写成了
-- 7020（Books/EBook），而 categories 表里仍是 0325 起的 7010 / 7030：
-- 包声明与表实存从此分叉。危害在下一步——按包重物化（切站型、保存包、
-- 内容包导入都走这条路）会把表覆盖成包里的 7020，于是文学·社科·科技·
-- 教辅·杂志·漫画六支挤进同一个对外号，Prowlarr/Calibre 侧
-- `?cat=7010`（Magazines）与 `?cat=7030`（Comics）直接归零。
-- 表侧现值就是对的，这里只把包声明改回来，不动表。

UPDATE site_type_packs
   SET categories = (
         SELECT jsonb_agg(
                  CASE
                    WHEN c->>'key' = 'ebook_mag'
                      THEN jsonb_set(c, '{newznab_id}', '7010')
                    WHEN c->>'key' = 'ebook_comic'
                      THEN jsonb_set(c, '{newznab_id}', '7030')
                    ELSE c
                  END
                  ORDER BY (c->>'sort')::int)
           FROM jsonb_array_elements(categories) AS c)
 WHERE code = 'ebook';
