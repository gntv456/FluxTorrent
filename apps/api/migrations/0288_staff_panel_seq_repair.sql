-- 0288_staff_panel_seq_repair.sql
-- 干净安装会让迁移链**当场失败**，且「等级规则」导航项永久消失（P1）。
--
-- 【症状】在全新空库按顺序跑迁移，0342 报
--   duplicate key value violates unique constraint "staff_panel_entries_pkey"
--   DETAIL: Key (id)=(106) already exists
-- 0342 之后的所有迁移都不会执行（sqlx 的 run 是单链，前一条失败即中止）。
-- 已部署库不受影响：当年 0342 跑在真实库上时 id 106 恰好空着，侥幸撞过去了。
-- 所以线上一直没暴露——这是「只有重建库才会撞上」的那类坑。
--
-- 【根因】0235 手工钉死了 id：
--   INSERT INTO staff_panel_entries (id, ...) VALUES (106, 'admin','抽卡运营', ...)
-- SERIAL 的序列不会因为这种「显式 id 的 INSERT」而前进。实测（0341 结束的库上）：
--   max(id) = 106 而 last_value = 105  →  nextval() = 106
-- 与 0235 那行正面相撞。0344 的注释其实已经记下了同一个病灶的痕迹
-- （「0339 的 trumps 与 0342 的 classes 两条导航项自落地以来从未出现过」），
-- 但当时只把空串 module_key 归一，没碰序列，真病灶被漏掉了。
--
-- 【为什么是 0288 这个号】本条必须在 0342 之前执行才有意义——放在链尾
-- （比如 0388）的话，0342 先失败、链就断了，它自己永远跑不到。
-- sqlx 按 version 升序应用并跳过已应用的版本，因此插在中间即可：
--   * 存量库：0288 未应用 → 照样执行一次；本条幂等且是纯修复，跑了无害；
--   * 干净库：在 0342 之前把序列归位，0342 拿到 107，冲突消失。
-- 选 288 是因为它在 git 历史里从未被任何迁移用过（0017~0387 的空档），
-- 不与既有号撞车、也不会让任何已应用环境冒出 VersionMissing。
-- 先例见 scripts/align_migration_renumber_0127_0128.sql：0127 挪到 0141
-- 用的就是同一个「历史空号」手法。
--
-- 【为什么不改 0235】迁移是账本，改已落库文件的字节会撞 sqlx checksum，
-- 站点下次启动直接起不来（`migration 235 was previously applied but has
-- been modified`）。修法是另记一笔，与 0343/0344 同一处理方式。

BEGIN;

-- ① 根治：把序列推到 max(id) 之后。这才是让「之后任何导航 INSERT 都拿不到
--    已占用 id」成立的那一步；只补 0342 那一行的话，下一条手写 id 的迁移
--    会把同样的坑再踩一遍。
--    GREATEST(...,1) 与既有 setval 惯例一致（0017/0033/0035/0063 同写法）：
--    空表时序列本就该从 1 起步。
SELECT setval(
    'staff_panel_entries_id_seq',
    GREATEST((SELECT max(id) FROM staff_panel_entries), 1),
    true
);

-- ② 兜底：把 0342 因为冲突而没插上的「等级规则」行补回来。
--    存量库本来就有这行（当年侥幸跑过去了），WHERE NOT EXISTS 保证零副作用；
--    干净库则由本条补齐，0342/0343 在其后跑成 no-op。
--    文案取 0343 的终态（0343 只 UPDATE，没处理「行压根没插上」的情况）。
INSERT INTO staff_panel_entries
    (panel, name, url, info, sort, section, min_class, tab_key,
     module_key, perm_key)
SELECT 'admin', '等级规则', '/admin?tool=classes',
       '成长档升降级阈值与晋升奖励（贵宾/职务档只读）',
       (SELECT COALESCE(max(sort), 0) + 1 FROM staff_panel_entries
          WHERE section = 'users'),
       'users', 93, 'classes', NULL, 'sitepacks.manage'
 WHERE NOT EXISTS (
     SELECT 1 FROM staff_panel_entries WHERE tab_key = 'classes'
 );

COMMIT;
