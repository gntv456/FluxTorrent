-- 0075 补充（在 0075_groups_notices.sql 基础上；该文件已含 ①②③④，此处为函数部分）
-- 通知偏好判定函数：notice_prefs->key = false 才跳过，缺省（null/无键）= 发送。
-- SQL 侧统一入口，worker 与 API 共用，防两处漂移（与 user_can 同款纪律）。
CREATE OR REPLACE FUNCTION u_notice_enabled(p_user_id BIGINT, p_kind TEXT)
RETURNS BOOLEAN LANGUAGE sql STABLE AS $$
    SELECT COALESCE((u.notice_prefs ->> p_kind)::boolean, TRUE)
    FROM users u WHERE u.id = p_user_id
$$;
