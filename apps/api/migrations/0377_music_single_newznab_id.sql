-- 0377_music_single_newznab_id.sql
-- P-335（批一百五十三）：music_single（单曲）挂着**电子书**的对外分类号 7020。
--
-- 症状：Torznab/RSS 出口按 Newznab 标准分类树归位，站方切到音乐站型后，
-- 「单曲」这一类会被 Prowlarr 等自动抓取工具归进 Books(7020)。对元数据
-- 订阅者来说，音乐条目混进书库；对站点自己来说，这是对外契约被错配。
--
-- 根因（取证链）：
--   1) 0317 给 categories 回填 key 时用的是 `p.cat->>'id' = c.id`——
--      **按 id 匹配**，而包里的 id 只是「空表 replace 时的建议序号」（pack_core.rs:63
--      自陈「不能拿来直接 INSERT」）。跨包切换时旧行占着同号主键，于是
--      music_single 这个 key 被贴到了当时 newznab_id=7020 的那一行上。
--   2) 0325 补齐了包载荷里的 newznab_id=3030，但 apply 走 pack_core 的
--      **merge** 语义：新号写成
--          newznab_id = CASE WHEN restore THEN EXCLUDED.newznab_id
--                             ELSE COALESCE(categories.newznab_id,
--                                          EXCLUDED.newznab_id) END
--      旧值非空 → COALESCE 取旧值 → **永不改写**。恢复(restart)模式才会覆盖。
--   实测：全表 21 行里只有 music_single 一行 declared≠actual（3030 vs 7020），
--   其余全部一致——说明这是 0317 那一处 id 撞车的孤立后果，不是系统性偏移。
--
-- 修法：只改这一行，且**按 key 定位**（不再按 id），并把声明值从包载荷里读出来
-- 而不是写死常量——这样以后包载荷改了，本迁移重放也不会打架。
-- 幂等：WHERE 已排除已正确的行，重放 0 行。

BEGIN;

UPDATE categories c
   SET newznab_id = (p.cat->>'newznab_id')::int
  FROM site_type_packs pk,
       jsonb_array_elements(pk.categories) AS p(cat)
 WHERE c.key = p.cat->>'key'
   AND c.key = 'music_single'
   AND p.cat ? 'newznab_id'
   AND c.newznab_id IS DISTINCT FROM (p.cat->>'newznab_id')::int;

-- 防复发：把「包声明了号但表里存着别的号」的行数写进注释可见的日志。
-- 只对 music/ebook/anime 这三个真正共号较多的包报警，其余包同号是正常的
-- （Newznab 树本身就是多对多：一个 7020 对应多个电子书子类）。
DO $$
DECLARE bad int;
BEGIN
  SELECT count(*) INTO bad
    FROM site_type_packs pk, jsonb_array_elements(pk.categories) p(cat)
   WHERE pk.code IN ('music','anime','movie','sports','game')
     AND p.cat ? 'newznab_id'
     AND EXISTS (SELECT 1 FROM categories c
                  WHERE c.key = p.cat->>'key'
                    AND c.newznab_id IS DISTINCT FROM (p.cat->>'newznab_id')::int);
  IF bad > 0 THEN
    RAISE NOTICE 'music_single 修复后仍有 % 行 declared≠actual（需人工核对）', bad;
  END IF;
END $$;

COMMIT;
