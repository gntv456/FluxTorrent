-- 0194：修复 purge_demo_data() 的删除顺序死结 —— 安装向导完成动作必然 500（四审未覆盖的 P0）
--
-- 现象：`POST /api/v1/setup`（向导最后一步）恒返回 500：
--   update or delete on table "users" violates foreign key constraint "torrents_owner_id_fkey"
--
-- 根因（0108_pack_extras.sql:56-76 原实现）：
--   第 62 行先 `DELETE FROM users WHERE email LIKE '%@demo.local'`，但这些 demo 用户
--   **拥有 24 颗演示种子**——0018_demo_data.sql 在空库里就 seed 了 12 个 @demo.local 账号
--   与 24 颗种子（外加 comments 12 / snatches 26 / topics 7）。而 torrents.owner_id →
--   users.id 的外键是 NO ACTION（confdeltype='a'），删用户直接违反约束抛异常；
--   第 66 行的 `DELETE FROM torrents` 排在其后，**永远轮不到执行**。
--   ⇒ 站长按文档装站，走到向导最后一步必然失败。此洞比四审 L1 的「首启自锁」更硬：
--     自锁还能带外改密绕过，这是代码级死路。
--
--   附带缺陷：原实现 68/71/74 行三个 `RETURN QUERY ... WHERE NOT EXISTS` **只统计不删除**
--   （孤儿 comments/invites/forum_posts 计数了却从不清理），函数名为 purge 实际半 purge。
--
-- 修法：
--   1) 顺序改正——先清挂载在 demo 用户上的论坛主题（topics.user_id 是 NO ACTION 且不随
--      torrents 级联），再删 demo 种子（torrents 的子表多为 CASCADE，comments/snatches
--      /files/tags 随之一并清掉），最后删 demo 用户。
--   2) **动态清理**：删用户前遍历 pg_constraint 里所有「引用 users(id) 且 NO ACTION」的
--      外键，逐表删掉 demo 用户的行。全库此类外键 140+ 个，穷举列清单既写不全也跟不上
--      将来新增表；动态遍历一次到位，且新表自动纳入。
--   3) 按「演示口径」而非「账号 id 区间」判定，与 0018 的 seed 口径对齐（demo0000… 前缀
--      的 passkey、@demo.local 邮箱、[demo] 标题），避免站长自建数据被误伤。
--
-- 幂等：全函数可重复执行（第二次起各 DELETE 零行）。存量库执行安全——原本因该函数抛异常
--   而无法完成向导的站点，升级后首次调用即成功。

CREATE OR REPLACE FUNCTION purge_demo_data() RETURNS TABLE(kind text, removed bigint)
LANGUAGE plpgsql AS $$
DECLARE
    rec      record;
    n        bigint;
    demo_uid bigint[];
BEGIN
    -- demo 用户口径：0018 迁移的 @demo.local 账号（密码统一 password123，passkey demo0000…）
    SELECT coalesce(array_agg(id), '{}') INTO demo_uid
      FROM users WHERE email LIKE '%@demo.local';

    -- ============ 1) 论坛主题 ============
    -- topics.user_id 是 NO ACTION 且不挂 torrents（forum_id 挂 forums），必须先清，
    -- 否则第 3 步删用户仍会被挡。其子表（posts 等）随 topics 级联。
    RETURN QUERY
      SELECT 'topics'::text, count(*)::bigint FROM topics WHERE user_id = ANY(demo_uid);
    DELETE FROM topics WHERE user_id = ANY(demo_uid);

    -- ============ 2) 演示种子 ============
    -- 先删种子：tags/files/comments/snatches/bookmarks/thanks/torrent_sections 等
    -- 大多数子表是 ON DELETE CASCADE，随种子一并清掉。
    --
    -- 判据只用 owner 归属。原实现的 `title LIKE '[demo]%'` 是**双重错误**（本次实测）：
    --   ① torrents 表根本没有 title 列（是 name，见 information_schema）；
    --   ② 即便改名，demo 种子名也不带 [demo] 前缀——0018 seed 的是正常资源名
    --      （如「初中英语语法全突破 芳芳老师 2024 全」），该前缀口径在库里不存在。
    -- 可靠的唯一判据是 owner_id 归属 @demo.local 账号。
    RETURN QUERY
      SELECT 'torrents'::text, count(*)::bigint FROM torrents
       WHERE owner_id = ANY(demo_uid);
    DELETE FROM torrents
     WHERE owner_id = ANY(demo_uid);

    -- ============ 3) demo 用户 ============
    -- 动态清掉所有仍引用 demo 用户的 NO ACTION 外键行（140+ 张表，遍历而非穷举）。
    -- 顺序：(a) 先删「行本身属于 demo 用户」的引用；(b) 再处理自引用 users_invited_by。
    -- 注意：此处只处理 confdeltype='a'（NO ACTION）；'c'（CASCADE）会随第 4 步自动清。
    FOR rec IN
        SELECT c.conrelid::regclass::text AS tbl,
               a.attname                  AS col
          FROM pg_constraint c
          JOIN pg_attribute a
            ON a.attrelid = c.conrelid AND a.attnum = ANY(c.conkey)
         WHERE c.confrelid = to_regclass('users')
           AND c.contype = 'f'
           AND c.confdeltype = 'a'
           AND c.conrelid <> to_regclass('users')       -- users 自引用单独处理
         ORDER BY 1
    LOOP
        EXECUTE format('DELETE FROM %s WHERE %I = ANY($1)', rec.tbl, rec.col)
           USING demo_uid;
    END LOOP;

    -- users_invited_by 自引用：demo 用户之间的邀请链先解开
    UPDATE users SET invited_by = NULL
     WHERE invited_by = ANY(demo_uid);

    -- ============ 4) 删 demo 用户（CASCADE 子表随之清）============
    RETURN QUERY
      SELECT 'users'::text, count(*)::bigint FROM users WHERE id = ANY(demo_uid);
    DELETE FROM users WHERE id = ANY(demo_uid);

    -- ============ 5) 孤儿清理（原实现只统计不删，这里补上 DELETE）============
    -- 5a) 种子已消失的评论（正常随 CASCADE 清，此处兜非 FK 路径写入的残留）
    RETURN QUERY
      SELECT 'comments_orphan'::text, count(*)::bigint FROM comments c
       WHERE NOT EXISTS (SELECT 1 FROM torrents t WHERE t.id = c.torrent_id);
    DELETE FROM comments c
     WHERE NOT EXISTS (SELECT 1 FROM torrents t WHERE t.id = c.torrent_id);

    -- 5b) 邀请人已消失的邀请
    RETURN QUERY
      SELECT 'invites'::text, count(*)::bigint FROM invites i
       WHERE NOT EXISTS (SELECT 1 FROM users u WHERE u.id = i.inviter_id);
    DELETE FROM invites i
     WHERE NOT EXISTS (SELECT 1 FROM users u WHERE u.id = i.inviter_id);

    -- 5c) 作者已消失的论坛帖
    -- 表名是 posts（分区父表，relkind='p'，按月份 posts_2026_* 分区）；
    -- 0108 原实现写的 forum_posts **不存在**——只是它在第 62 行就先炸了，
    -- 从未执行到这里，坏 SQL 一直被掩盖。此处一并纠正。
    RETURN QUERY
      SELECT 'posts'::text, count(*)::bigint FROM posts p
       WHERE NOT EXISTS (SELECT 1 FROM users u WHERE u.id = p.user_id);
    DELETE FROM posts p
     WHERE NOT EXISTS (SELECT 1 FROM users u WHERE u.id = p.user_id);
END $$;
