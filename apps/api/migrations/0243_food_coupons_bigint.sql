-- 口粮券列型与代码口径对齐。0239 把 users.food_coupons 建成 integer(INT4)，
-- 但 linkage.rs 全程按 i64 解码，sqlx 不做 INT4→INT8 的位宽放宽，线上
-- /games/linkage/status 直接 500：
--   mismatched types; Rust type `i64` (as SQL type `INT8`) is not compatible with SQL type `INT4`
-- 选改列而不是改代码：券余额与魔力/火花同量级，全站余额列一律 bigint，
-- 这里跟着 bigint 才是口径一致，而不是再开一个 int4 余额的特例。
ALTER TABLE users ALTER COLUMN food_coupons TYPE bigint USING food_coupons::bigint;

-- 落库即断言列型，防止将来有人重建 0239 时又退回 integer 而无人发现。
DO $$
DECLARE t text;
BEGIN
    SELECT data_type INTO t
      FROM information_schema.columns
     WHERE table_name = 'users' AND column_name = 'food_coupons';
    IF t IS DISTINCT FROM 'bigint' THEN
        RAISE EXCEPTION 'users.food_coupons 应为 bigint，实为 %', t;
    END IF;
END $$;
