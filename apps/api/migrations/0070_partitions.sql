-- 0070 流水表分区落地：存量从 default 拆到月分区
-- 背景：0001 建了 RANGE 分区结构（注释写明"自动建分区由 worker 负责"），但 worker 从未实现，
--       全部数据堆在 *_default 分区里，分区裁剪完全失效。
-- 本迁移：按「先搬空 default → 建月分区 → 回灌（自动路由进新分区）」的顺序处理
--       traffic_ledger(window_start) / spark_ledger(created_at) / posts(created_at) 三表 2026 年数据。
--       （PG 创建新分区时若 default 分区含落入新边界的行会直接报错，必须先清空该月范围）
-- 运行时预建未来分区由 worker 的 ensure_partitions job 每日负责（当月至 +2 月）。

DO $$
DECLARE
  tbl text;
  m date;
  col text;
  staged boolean;
BEGIN
  FOREACH tbl IN ARRAY ARRAY['traffic_ledger', 'spark_ledger', 'posts'] LOOP
    col := CASE tbl WHEN 'traffic_ledger' THEN 'window_start' ELSE 'created_at' END;
    FOR m IN SELECT generate_series('2026-01-01'::date, '2026-12-01'::date, interval '1 month')::date LOOP
      -- ① 从 default 暂存该月行（无该月数据则跳过搬迁）
      EXECUTE format('CREATE TEMP TABLE _pt_stage ON COMMIT DROP AS SELECT * FROM %I_default WHERE %I >= %L AND %I < %L',
                     tbl, col, m, col, m + interval '1 month');
      SELECT count(*) > 0 INTO staged FROM _pt_stage;
      IF staged THEN
        EXECUTE format('DELETE FROM %I_default WHERE %I >= %L AND %I < %L', tbl, col, m, col, m + interval '1 month');
      END IF;
      -- ② 建月分区（default 该月已清空，不会触发 moving-rows 报错）
      EXECUTE format('CREATE TABLE IF NOT EXISTS %I_%s PARTITION OF %I FOR VALUES FROM (%L) TO (%L)',
                     tbl, to_char(m, 'YYYY_MM'), tbl, m, m + interval '1 month');
      -- ③ 回灌（INSERT 走父表，自动路由进刚建的月分区）
      IF staged THEN
        EXECUTE format('INSERT INTO %I SELECT * FROM _pt_stage', tbl);
      END IF;
      DROP TABLE _pt_stage;
    END LOOP;
  END LOOP;
END $$;
