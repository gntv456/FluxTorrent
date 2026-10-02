-- 清理 ZT 规模化压测数据。
-- PostgreSQL 不支持 DELETE ... CASCADE（MySQL 语法）。这里用 DO 块：
-- 反复遍历所有指向 users / torrents 的单列外键，删除引用「合成根行」的记录；
-- 遇到外键冲突（下层尚未清完）则本轮跳过，下一轮再试，直到一轮零删除。
-- 排除 torrents / users 自身，最后显式按「先子后父」删根行。
-- 安全性：仅命中 username LIKE 'ztscale_%' / name LIKE '[ZTS]%' 的合成行及其依赖。
DO $$
DECLARE r record; n bigint; total bigint;
BEGIN
  LOOP
    total := 0;
    FOR r IN
      SELECT c.conrelid::regclass AS child,
             a.attname            AS col,
             c.confrelid::regclass AS parent
      FROM pg_constraint c
      JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = c.conkey[1]
      WHERE c.contype = 'f'
        AND c.confrelid IN ('users'::regclass, 'torrents'::regclass)
        AND array_length(c.conkey, 1) = 1
        AND c.conrelid NOT IN ('users'::regclass, 'torrents'::regclass)
    LOOP
      BEGIN
        IF r.parent = 'users'::regclass THEN
          EXECUTE format(
            'DELETE FROM %s WHERE %I IN (SELECT id FROM users WHERE username LIKE ''ztscale_%%'')',
            r.child, r.col);
        ELSE
          EXECUTE format(
            'DELETE FROM %s WHERE %I IN (SELECT id FROM torrents WHERE name LIKE ''[ZTS]%%'')',
            r.child, r.col);
        END IF;
        GET DIAGNOSTICS n = ROW_COUNT;
        total := total + n;
        IF n > 0 THEN RAISE NOTICE '清理 %.% : % 行', r.child, r.col, n; END IF;
      EXCEPTION WHEN foreign_key_violation THEN
        NULL;  -- 下层未清完，下轮再试
      END;
    END LOOP;
    EXIT WHEN total = 0;
  END LOOP;
END $$;

DELETE FROM snatches WHERE user_id IN (SELECT id FROM users WHERE username LIKE 'ztscale_%');
DELETE FROM torrents WHERE name LIKE '[ZTS]%';
DELETE FROM users    WHERE username LIKE 'ztscale_%';

ANALYZE users; ANALYZE torrents; ANALYZE snatches;
SELECT 'users' t, count(*) FROM users
UNION ALL SELECT 'torrents', count(*) FROM torrents
UNION ALL SELECT 'snatches', count(*) FROM snatches
UNION ALL SELECT 'seed_milestones', count(*) FROM seed_milestones
UNION ALL SELECT 'hr_snapshots', count(*) FROM hr_snapshots;
