-- 0080 审计修复批次一（P0）：schema 漂移矫正 + 经济幂等硬约束
-- 背景：2026-09-14 全链路审计（_doc/audit-2026-09-14/00-缺陷总清单.md）

-- ① [P0-1/P0-9] snatches.connectable 列类型漂移矫正：
--    0032 先建 BOOLEAN，0071 想改成 SMALLINT(-1/0/1) 但 ADD COLUMN IF NOT EXISTS 静默 no-op，
--    worker 按 SMALLINT 语义绑参 → announce 计费链路整体死锁、做种收益停发。
--    存量 boolean 统一映射：true→1（历史口径"可连接"）、false→0。
ALTER TABLE snatches ALTER COLUMN connectable DROP DEFAULT;
ALTER TABLE snatches
  ALTER COLUMN connectable TYPE smallint
  USING CASE WHEN connectable IS NULL THEN NULL
             WHEN connectable THEN 1
             ELSE 0 END;
ALTER TABLE snatches ALTER COLUMN connectable SET DEFAULT 1;
COMMENT ON COLUMN snatches.connectable IS 'tracker 回连抽样：-1 未测 / 0 不可达 / 1 可达（0071 语义，0080 起真正生效）';

-- ② [P0-4] spark_ledger 幂等键硬约束：
--    此前仅普通索引，worker 侧 4 处 INSERT ... WHERE NOT EXISTS + 独立 UPDATE 的路径
--    在调度重叠/双实例下可双插双加余额。存量键经查无重复，可直接转唯一。
-- 注：spark_ledger 是 RANGE(created_at) 分区表，分区表上的唯一索引必须包含分区键。
DROP INDEX IF EXISTS idx_spark_idem;
CREATE UNIQUE INDEX idx_spark_idem ON spark_ledger (idempotency_key, created_at)
  WHERE idempotency_key IS NOT NULL;
-- 逻辑唯一性由触发器在写入侧强制（幂等键跨分区重复时抛错）：
CREATE OR REPLACE FUNCTION spark_ledger_idem_guard() RETURNS trigger AS $$
BEGIN
  IF NEW.idempotency_key IS NOT NULL AND EXISTS (
    SELECT 1 FROM spark_ledger l
    WHERE l.idempotency_key = NEW.idempotency_key AND l.created_at < NEW.created_at + interval '1 second' AND l.created_at > NEW.created_at - interval '365 days'
  ) THEN
    RAISE EXCEPTION 'duplicate spark_ledger idempotency_key: %', NEW.idempotency_key;
  END IF;
  RETURN NEW;
END $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS trg_spark_idem ON spark_ledger;
CREATE TRIGGER trg_spark_idem BEFORE INSERT ON spark_ledger
  FOR EACH ROW WHEN (NEW.idempotency_key IS NOT NULL)
  EXECUTE FUNCTION spark_ledger_idem_guard();

-- ③ [P0-2] announce_url 站点设定纠偏：8080 双重 /announce → tracker 根地址。
--    build_torrent_bytes 拼接 {base}/announce/{passkey}，故 base 必须是 tracker 根。
UPDATE site_settings SET value = 'http://127.0.0.1:7070', updated_at = now()
 WHERE name = 'announce_url' AND value LIKE '%/announce';

-- ④ [P1-5] staff（class 90/93/99）补 torrent.see_banned 权限：
--    此前仅 uploader 职务持有，审核员经列表（include_unapproved）看不到待审种子。
INSERT INTO role_permissions (role_type, role_key, permission_key, granted)
SELECT 'class', c, 'torrent.see_banned', true
FROM (VALUES ('90'), ('93'), ('99')) AS t(c)
ON CONFLICT DO NOTHING;
