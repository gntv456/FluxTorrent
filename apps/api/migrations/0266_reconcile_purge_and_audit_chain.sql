-- 0266 深度体验测试修复批（2026-10-02 二轮）：
--   ① P1 对账误报根治：删除用户时清零 spark_balance 并补 user_purge 流水；
--      demo 物理删除路径同步清账。历史孤儿流水（demo 用户物理删除残留）回填对冲。
--   ② P2 审计哈希链从空壳变实装：BEFORE INSERT 触发器串链，存量 0001 起回填。

-- ============ ① 对账：删用户动账纪律 ============

-- 1a. delete_user_cascade（Rust 侧同一事务语义，此处建函数供迁移回填与将来复用）：
--     软删前若余额非零，补一条 user_purge 负流水并把余额清零——对账两侧同时归零。
CREATE OR REPLACE FUNCTION user_purge_ledger(uid bigint) RETURNS integer
LANGUAGE plpgsql AS $$
DECLARE
    bal bigint;
    inserted integer := 0;
BEGIN
    SELECT spark_balance INTO bal FROM users WHERE id = uid AND status = 3;
    IF bal IS NOT NULL AND bal <> 0 THEN
        INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, idempotency_key, balance_after)
        VALUES (nextval('spark_ledger_id_seq'), uid, -bal, 'user_purge', 'user', 'user-purge-' || uid, 0);
        UPDATE users SET spark_balance = 0 WHERE id = uid;
        inserted := 1;
    END IF;
    RETURN inserted;
END $$;

-- 1b. 存量回填：软删用户（status=3）残留余额 → 补 purge 流水清零。
--     幂等：idempotency_key 已存在则跳过（唯一索引 idx_spark_idem）。
DO $$
DECLARE
    r record;
BEGIN
    FOR r IN SELECT id, spark_balance AS bal FROM users
             WHERE status = 3 AND spark_balance <> 0
    LOOP
        BEGIN
            INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, idempotency_key, balance_after)
            VALUES (nextval('spark_ledger_id_seq'), r.id, -r.bal, 'user_purge', 'user', 'user-purge-' || r.id, 0);
        EXCEPTION WHEN unique_violation THEN NULL; -- 已回填过
        END;
        UPDATE users SET spark_balance = 0 WHERE id = r.id;
    END LOOP;
END $$;

-- 1c. 存量孤儿流水对冲：物理删除（demo purge）残留的 52,374 无主流水，
--     补一条孤儿对冲行（kind='orphan_offset'）使流水侧合计回到与余额侧一致。
--     对账口径是「流水合计 vs 余额合计」两侧全站求和：user_purge 已让两侧同减
--     1488899；孤儿流水只在流水侧多 52374，补 -52374 一行即两侧相等。
--     只补差额一行，不改历史行（账本纪律：只追加，不篡改）；
--     用户余额不动（对冲行 balance_after 仅记录当时 root 余额，作快照）。
DO $$
DECLARE
    orphan_sum bigint;
    already bigint;
BEGIN
    SELECT COALESCE(sum(l.amount), 0) INTO orphan_sum
    FROM spark_ledger l LEFT JOIN users u ON u.id = l.user_id
    WHERE u.id IS NULL;
    SELECT count(*) INTO already
    FROM spark_ledger WHERE kind = 'orphan_offset' AND idempotency_key = 'orphan-offset-20261002';
    IF orphan_sum <> 0 AND already = 0 THEN
        INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, idempotency_key, balance_after)
        VALUES (nextval('spark_ledger_id_seq'), 1, -orphan_sum, 'orphan_offset', 'system', 'orphan-offset-20261002',
                (SELECT spark_balance FROM users WHERE id = 1));
    END IF;
END $$;

-- 1d. demo 物理删除路径同步清账（purge_demo_data 在删除 users 前补 purge 流水）：
CREATE OR REPLACE FUNCTION purge_demo_data() RETURNS TABLE(kind text, removed bigint)
LANGUAGE plpgsql AS $$
BEGIN
    -- 删前结账：@demo.local 账号余额补 user_purge 流水（对账两侧同减，防再产孤儿）
    INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, idempotency_key, balance_after)
    SELECT nextval('spark_ledger_id_seq'), id, -spark_balance, 'user_purge', 'user', 'user-purge-' || id, 0
    FROM users WHERE email LIKE '%@demo.local' AND spark_balance <> 0
    ON CONFLICT DO NOTHING;
    RETURN QUERY
    SELECT 'users'::text, count(*)::bigint FROM users WHERE email LIKE '%@demo.local';
    DELETE FROM users WHERE email LIKE '%@demo.local';
    RETURN QUERY
    SELECT 'torrents'::text, count(*)::bigint FROM torrents WHERE title LIKE '[demo]%';
    DELETE FROM torrents WHERE title LIKE '[demo]%';
    RETURN QUERY
    SELECT 'comments'::text, count(*)::bigint FROM comments c
    WHERE NOT EXISTS (SELECT 1 FROM torrents t WHERE t.id = c.torrent_id);
    RETURN QUERY
    SELECT 'invites'::text, count(*)::bigint FROM invites i
    WHERE NOT EXISTS (SELECT 1 FROM users u WHERE u.id = i.inviter_id);
    RETURN QUERY
    SELECT 'forum_posts'::text, count(*)::bigint FROM forum_posts p
    WHERE NOT EXISTS (SELECT 1 FROM users u WHERE u.id = p.user_id);
END $$;

-- ============ ② 审计哈希链实装（prev_hash/self_hash 自 0001 起为空壳）============

-- 链式哈希：self_hash = SHA256(prev_hash || id || actor_id || action || coalesce(ref) || created_at)
-- pgcrypto 的 digest() 已在依赖（0001 起 citext/pg_trgm 同批，pgcrypto 已装）。
-- 审计行只 INSERT 不 UPDATE/DELETE（历史无此路径；触发器同时拦 UPDATE/DELETE 防篡改）。
CREATE OR REPLACE FUNCTION audit_log_chain() RETURNS trigger
LANGUAGE plpgsql AS $$
DECLARE
    prev bytea;
BEGIN
    IF TG_OP = 'DELETE' THEN
        RAISE EXCEPTION 'audit_log 行不可删除（防篡改链）';
    END IF;
    IF TG_OP = 'UPDATE' THEN
        RAISE EXCEPTION 'audit_log 行不可更新（防篡改链）';
    END IF;
    SELECT self_hash INTO prev FROM audit_log ORDER BY id DESC LIMIT 1;
    IF prev IS NULL THEN
        prev := decode(repeat('00', 32), 'hex'); -- 创世行：全零前驱
    END IF;
    IF NEW.prev_hash IS DISTINCT FROM prev THEN
        NEW.prev_hash := prev;
    END IF;
    NEW.self_hash := digest(
        NEW.prev_hash ||
        NEW.id::text::bytea ||
        COALESCE(NEW.actor_id::text, 'null')::bytea ||
        NEW.action::bytea ||
        COALESCE(NEW.ref::text, 'null')::bytea ||
        to_char(NEW.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS.US')::bytea,
        'sha256');
    RETURN NEW;
END $$;

DROP TRIGGER IF EXISTS trg_audit_log_chain ON audit_log;
CREATE TRIGGER trg_audit_log_chain
    BEFORE INSERT OR UPDATE OR DELETE ON audit_log
    FOR EACH ROW EXECUTE FUNCTION audit_log_chain();

-- 存量回填（按 id 顺序串链；纯 SQL 循环，万行级可接受，一次性）。
-- 回填须绕过防篡改触发器：先 DROP 再回填再重建。
DROP TRIGGER IF EXISTS trg_audit_log_chain ON audit_log;

DO $$
DECLARE
    r record;
    prev bytea := decode(repeat('00', 32), 'hex');
    h bytea;
BEGIN
    IF EXISTS (SELECT 1 FROM audit_log WHERE self_hash IS NOT NULL) THEN
        RETURN; -- 幂等：已回填过
    END IF;
    FOR r IN SELECT id, actor_id, action, COALESCE(ref::text, 'null') AS ref,
                    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD HH24:MI:SS.US') AS ts
             FROM audit_log ORDER BY id
    LOOP
        h := digest(prev ||
                    r.id::text::bytea ||
                    COALESCE(r.actor_id::text, 'null')::bytea ||
                    r.action::bytea ||
                    r.ref::bytea ||
                    r.ts::bytea, 'sha256');
        UPDATE audit_log SET prev_hash = prev, self_hash = h WHERE id = r.id;
        prev := h;
    END LOOP;
END $$;

DROP TRIGGER IF EXISTS trg_audit_log_chain ON audit_log;
CREATE TRIGGER trg_audit_log_chain
    BEFORE INSERT OR UPDATE OR DELETE ON audit_log
    FOR EACH ROW EXECUTE FUNCTION audit_log_chain();

-- ============ ④ promotions 悬挂——核实为误报，无需迁移 ============
-- 复核结论：promotions.torrent_id 自 0001 起即 ON DELETE CASCADE；现存库零悬挂
-- （体验测试看到的"残留"是软删 approval_status=3 的回收站语义，种子行仍在，
-- 属有意保留——恢复种子时促销随回）。物理删种（demo purge/直接 SQL）走级联，无泄漏。
