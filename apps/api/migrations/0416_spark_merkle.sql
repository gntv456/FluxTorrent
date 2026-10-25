-- 0416: 每日火花流水结算凭证（Merkle inclusion proof）
--
-- 目的：给会员一份可自行复核的「当天流水确实被结算、事后未被改动」的凭证。
-- 纯哈希审计，不写链、不发币、不引入任何钱包/代币语义（anchoring=none）。
--
-- 表分两层：
--   spark_merkle_days   每天一行树根 + 叶子/人数（公开可读，聚合口径）；
--   spark_merkle_leaves 每一行流水一片叶子（含原文），仅本人可读。
-- 叶子一旦封存不再重算：之后改动 spark_ledger 不会改历史树根，
-- 这正是「可对照」的来源。空日也会封存（root 为约定空值）。
CREATE TABLE IF NOT EXISTS spark_merkle_days (
  day DATE PRIMARY KEY,
  root TEXT NOT NULL,
  leaf_count BIGINT NOT NULL,
  user_count BIGINT NOT NULL,
  built_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS spark_merkle_leaves (
  day DATE NOT NULL REFERENCES spark_merkle_days(day) ON DELETE CASCADE,
  idx BIGINT NOT NULL,
  user_id BIGINT NOT NULL,
  ledger_id BIGINT NOT NULL,
  amount BIGINT NOT NULL,
  kind TEXT NOT NULL,
  created TEXT NOT NULL,
  leaf TEXT NOT NULL,
  preimage TEXT NOT NULL,
  PRIMARY KEY (day, idx)
);

CREATE INDEX IF NOT EXISTS idx_spark_merkle_leaves_user
  ON spark_merkle_leaves (day, user_id);
