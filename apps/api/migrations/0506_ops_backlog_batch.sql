-- 0506 运营体检二批（余量台账清偿）：
-- contests.metric 赛制参数化：uploads=窗口内过审种数（0503 既有口径，
-- 缺省零漂移）| downloads=窗口内完成下载数 | seedtime=窗口内做种秒数
-- 增量。旧行回填 'uploads' 保持行为不变。

ALTER TABLE contests ADD COLUMN IF NOT EXISTS metric TEXT
    NOT NULL DEFAULT 'uploads';
ALTER TABLE contests DROP CONSTRAINT IF EXISTS contests_metric_check;
ALTER TABLE contests ADD CONSTRAINT contests_metric_check
    CHECK (metric IN ('uploads', 'downloads', 'seedtime'));
COMMENT ON COLUMN contests.metric IS
  '计分赛制：uploads=过审种数 downloads=完成下载数 seedtime=做种秒增量';
