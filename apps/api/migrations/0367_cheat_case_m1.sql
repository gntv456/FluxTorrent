-- 0367_cheat_case_m1.sql：反作弊案件管理 M1（对标 PT Studio anticheat 运营层）
-- 策略「长在」：案件长在 reports 表（ref_type='cheat'），证据长在 cheat_events，
-- 不建平行体系。迁移全部幂等，可重跑。

-- ① reports 扩展案件列（举报表复用为案件队列，列对举报行无副作用）
ALTER TABLE reports ADD COLUMN IF NOT EXISTS priority smallint NOT NULL DEFAULT 0;
ALTER TABLE reports ADD COLUMN IF NOT EXISTS risk_score int;
ALTER TABLE reports ADD COLUMN IF NOT EXISTS evidence_ref text;
ALTER TABLE reports ADD COLUMN IF NOT EXISTS sla_breached boolean NOT NULL DEFAULT FALSE;

-- ② 举报去重索引排除 cheat 类型：案件与举报去重语义不同（举报按
--    (reporter, 目标) 去重；案件按「同一被调查对象至多一个进行中」去重）。
--    旧索引 uq_reports_open_per_reporter (ref_type, ref_id, reporter_id)
--    WHERE status = ANY(ARRAY[0,2]) 对 cheat 行的去重键无意义，先删后建。
--    注意：http/reports_http.rs 的 ON CONFLICT 推断谓词必须同步加
--    ref_type <> 'cheat'，否则 PG 推断失败（索引谓词与 ON CONFLICT 不匹配）。
DROP INDEX IF EXISTS uq_reports_open_per_reporter;
CREATE UNIQUE INDEX IF NOT EXISTS uq_reports_open_per_reporter
    ON reports (ref_type, ref_id, reporter_id)
    WHERE status = ANY(ARRAY[0, 2]) AND ref_type <> 'cheat';

-- ③ cheat 案件唯一：同一 ref_id（被开案用户）至多一个进行中案件
CREATE UNIQUE INDEX IF NOT EXISTS uq_cheat_case_open
    ON reports (ref_id)
    WHERE ref_type = 'cheat' AND status = ANY(ARRAY[0, 2]);

-- ④ 案件队列索引（priority DESC, 等待时长）——部分索引只覆盖 cheat 未结案
CREATE INDEX IF NOT EXISTS idx_reports_cheat_queue
    ON reports (priority DESC, created_at)
    WHERE ref_type = 'cheat' AND status = ANY(ARRAY[0, 2]);

-- ⑤ cheat_events 归案：证据挂到案件（可空——未开案证据暂时游离）
ALTER TABLE cheat_events ADD COLUMN IF NOT EXISTS case_id bigint
    REFERENCES reports(id);
CREATE INDEX IF NOT EXISTS idx_cheat_events_case
    ON cheat_events (case_id) WHERE case_id IS NOT NULL;

-- ⑥ 案件备注时间线：多 staff 协同处理的过程记录（PT Studio FR-ACH-017）。
--    与 audit_detail 分工：audit 是操作审计（不可改的合规流），备注是
--    处理过程（协同载体）。30 天后随案件归档匿名化由 M3 处理。
CREATE TABLE IF NOT EXISTS case_notes (
  id BIGSERIAL PRIMARY KEY,
  case_id BIGINT NOT NULL REFERENCES reports(id) ON DELETE CASCADE,
  author_id BIGINT NOT NULL REFERENCES users(id),
  body TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_case_notes_case ON case_notes (case_id, created_at);
