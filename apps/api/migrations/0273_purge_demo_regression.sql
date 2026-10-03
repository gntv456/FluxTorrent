-- 0273：修复 0266 对 purge_demo_data() 的整体回退 —— 空库装机向导又变必然 500。
--
-- 事故链：
--   0194 修过一次同款 500（先删种子再删用户 + 动态清 NO ACTION 外键）；
--   0266 做「demo 删除前补 user_purge 结账流水」时，CREATE OR REPLACE 整个函数
--   **抄回了 0108 的旧函数体**（连 `title LIKE '[demo]%'` 这个 0194 已纠正的双重
--   错误都在——torrents 列是 name 不是 title，且 demo 种子名本就不带前缀），
--   0194 的顺序修复被静默整体覆盖。本地 dev 库当时已跑完 0194 且装机 done，
--   purge 不再被调用，所以四轮深测全没踩到；e2e 空库装站闸门一跑即复现。
--
-- 修法：以 0194 的函数体为准（正确顺序 + 动态外键清理），在**最前**插入 0266
--   要的 user_purge 结账流水（删用户前把余额清账，对账两侧同减）。
--   结账口径与 0266 逐字一致：kind=user_purge、幂等键 user-purge-<id>。
--
-- 幂等：全函数可重复执行（第二次起各 DELETE 零行、结账 ON CONFLICT DO NOTHING）。
--   已按 0266 记账的存量库重放安全。

CREATE OR REPLACE FUNCTION purge_demo_data() RETURNS TABLE(kind text, removed bigint)
LANGUAGE plpgsql AS $$
DECLARE
    rec      record;
    demo_uid bigint[];
BEGIN
    -- demo 用户口径：0018 迁移的 @demo.local 账号
    SELECT coalesce(array_agg(id), '{}') INTO demo_uid
      FROM users WHERE email LIKE '%@demo.local';

    -- 删前结账（0266 口径保留）：@demo.local 账号余额补 user_purge 流水
    INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type,
                              idempotency_key, balance_after)
    SELECT nextval('spark_ledger_id_seq'), id, -spark_balance,
           'user_purge', 'user', 'user-purge-' || id, 0
      FROM users
     WHERE email LIKE '%@demo.local' AND spark_balance <> 0
    ON CONFLICT DO NOTHING;

    -- ============ 1) 论坛主题（topics.user_id 是 NO ACTION 且不挂种子）============
    RETURN QUERY
      SELECT 'topics'::text, count(*)::bigint FROM topics
       WHERE user_id = ANY(demo_uid);
    DELETE FROM topics WHERE user_id = ANY(demo_uid);

    -- ============ 2) 演示种子（判据=owner 归属；name 无 [demo] 前缀，0194 实测）============
    RETURN QUERY
      SELECT 'torrents'::text, count(*)::bigint FROM torrents
       WHERE owner_id = ANY(demo_uid);
    DELETE FROM torrents
     WHERE owner_id = ANY(demo_uid);

    -- ============ 3) 动态清引用 demo 用户的 NO ACTION 外键行（140+ 表，遍历不穷举）============
    FOR rec IN
        SELECT c.conrelid::regclass::text AS tbl,
               a.attname                  AS col
          FROM pg_constraint c
          JOIN pg_attribute a
            ON a.attrelid = c.conrelid AND a.attnum = ANY(c.conkey)
         WHERE c.confrelid = to_regclass('users')
           AND c.contype = 'f'
           AND c.confdeltype = 'a'
           AND c.conrelid <> to_regclass('users')
         ORDER BY 1
    LOOP
        EXECUTE format('DELETE FROM %s WHERE %I = ANY($1)', rec.tbl, rec.col)
           USING demo_uid;
    END LOOP;

    UPDATE users SET invited_by = NULL
     WHERE invited_by = ANY(demo_uid);

    -- ============ 4) 删 demo 用户（CASCADE 子表随之清）============
    RETURN QUERY
      SELECT 'users'::text, count(*)::bigint FROM users WHERE id = ANY(demo_uid);
    DELETE FROM users WHERE id = ANY(demo_uid);

    -- ============ 5) 孤儿清理（统计 + 删除）============
    RETURN QUERY
      SELECT 'comments_orphan'::text, count(*)::bigint FROM comments c
       WHERE NOT EXISTS (SELECT 1 FROM torrents t WHERE t.id = c.torrent_id);
    DELETE FROM comments c
     WHERE NOT EXISTS (SELECT 1 FROM torrents t WHERE t.id = c.torrent_id);

    RETURN QUERY
      SELECT 'invites'::text, count(*)::bigint FROM invites i
       WHERE NOT EXISTS (SELECT 1 FROM users u WHERE u.id = i.inviter_id);
    DELETE FROM invites i
     WHERE NOT EXISTS (SELECT 1 FROM users u WHERE u.id = i.inviter_id);

    -- 表名是 posts（分区父表）；0108 写的 forum_posts 不存在
    RETURN QUERY
      SELECT 'posts'::text, count(*)::bigint FROM posts p
       WHERE NOT EXISTS (SELECT 1 FROM users u WHERE u.id = p.user_id);
    DELETE FROM posts p
     WHERE NOT EXISTS (SELECT 1 FROM users u WHERE u.id = p.user_id);
END $$;
