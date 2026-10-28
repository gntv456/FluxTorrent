-- 0430: 审计哈希链并发断裂修复（E-1）。
--
-- 现象：链校验 first_broken_id=5466，全链共 11 处 prev_hash ≠ 前行 self_hash。
-- 成因：0266 的触发器 audit_log_chain() 在 BEFORE INSERT 里
--   `SELECT self_hash FROM audit_log ORDER BY id DESC LIMIT 1`
-- 读「当前已提交的最后一行」——两个并发事务同时插入时都读到同一个前驱，
-- 后提交者 prev_hash 指向倒数第二行，链在此分叉（复现：双连接同毫秒
-- 插入，新行 prev 全部指到同一前驱，第二行断链；历史断点 5466/9534/
-- 9563/17981/21703/23463/30650/30653/30656/30661 全部是毫秒级连插）。
--
-- 修复分两步：
--   ① 新触发器函数：BEFORE INSERT 段用事务级咨询锁
--      pg_advisory_xact_lock(910718) 串行化「读末行+定前驱」，
--      并发插入排队执行，各自读到已提交的最新前驱。锁随事务结束
--      自动释放；audit_log 行插入本就毫秒级，串行化点不构成瓶颈。
--   ② 存量断链重接：11 处历史断点是并发写入的既成事实，行内容本身
--      未被篡改（self_hash 与内容重算全部一致）。按 id 序把断点行的
--      prev_hash 重接为前行 self_hash 并重算 self_hash（内容判据不变），
--      链恢复连续。修复动作自身落一行审计记录。

-- ① 新触发器：咨询锁串行化链前驱读取
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
    PERFORM pg_advisory_xact_lock(910718);
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

-- ② 存量断链重接：绕过防篡改触发器（迁移以表主身份执行），逐行对齐
--    prev_hash = 前行 self_hash（首行=全零创世），并重算 self_hash。
ALTER TABLE audit_log DISABLE TRIGGER trg_audit_log_chain;

DO $$
DECLARE
    r RECORD;
    prev_h bytea := decode(repeat('00',32),'hex');
    new_self bytea;
    changed int := 0;
BEGIN
    FOR r IN SELECT id, prev_hash, self_hash, actor_id, action, ref, created_at
             FROM audit_log ORDER BY id
    LOOP
        IF r.prev_hash IS DISTINCT FROM prev_h THEN
            new_self := digest(
                prev_h ||
                r.id::text::bytea ||
                COALESCE(r.actor_id::text,'null')::bytea ||
                r.action::bytea ||
                COALESCE(r.ref::text,'null')::bytea ||
                to_char(r.created_at AT TIME ZONE 'UTC','YYYY-MM-DD HH24:MI:SS.US')::bytea,
                'sha256');
            UPDATE audit_log
               SET prev_hash = prev_h, self_hash = new_self
             WHERE id = r.id;
            prev_h := new_self;
            changed := changed + 1;
        ELSE
            prev_h := r.self_hash;
        END IF;
    END LOOP;
    RAISE NOTICE 'audit chain relink: % rows fixed', changed;
END $$;

ALTER TABLE audit_log ENABLE TRIGGER trg_audit_log_chain;

-- ③ 修复动作留痕（走正常插入通道——新触发器生效后写入）
INSERT INTO audit_log (id, actor_id, action, ref)
VALUES (nextval('audit_log_id_seq'), 1, 'audit_chain_relink',
        '{"migration": "0430", "reason": "E-1 concurrent-insert chain fork repair"}'::jsonb);
