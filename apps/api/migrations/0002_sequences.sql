-- 补充分区表 ID 序列（0001 中 BIGINT 手动 id 需要序列供给 nextval）
CREATE SEQUENCE IF NOT EXISTS spark_ledger_id_seq;
CREATE SEQUENCE IF NOT EXISTS traffic_ledger_id_seq;
CREATE SEQUENCE IF NOT EXISTS posts_id_seq;
CREATE SEQUENCE IF NOT EXISTS audit_log_id_seq;
