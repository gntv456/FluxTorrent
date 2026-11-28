-- 0530 娱乐屋门槛键收口：三个迟插键拉平写权 + 老虎机门槛标签矫正（2026-11-28）
--
-- ① 写权拉平：0490 的九键赶上了 0493 的 min_class 99→98 一次性 UPDATE，
--    但 0514（打地鼠）/ 0517（水果机）/ 0521（斗地主）在 0493 之后才插入，
--    INSERT 列表未写 min_class，停在缺省 99 → 站长号（98）在后台只读、
--    保存整批被拒（settings 三闸第三闸）。对齐 0493 口径拉平到 98。
--
-- ② 标签矫正：老虎机上线后复用旧 capsule 池键 games_minclass_capsule
--    （流水/成就/图鉴不断档，前端 games-registry badge='capsule'），
--    但 settings_meta 标签仍叫「扭蛋等级门槛」，扭蛋入口已退役——
--    后台对着清单找不到「老虎机等级门槛」。只改文案，键名与值不动
--    （改键名会断 /games 概览与 capsule 硬闸的取数路径）。

UPDATE settings_meta SET min_class = 98, updated_at = now()
WHERE name IN ('games_minclass_mole', 'games_minclass_fruit',
               'games_minclass_doudizhu')
  AND min_class = 99;

UPDATE settings_meta
SET label_zh = '老虎机等级门槛',
    label_en = 'Slot min class',
    updated_at = now()
WHERE name = 'games_minclass_capsule';
