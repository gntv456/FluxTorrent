-- 0126 论坛 Phase2（第三片）：抽奖主题（lottery）
--
-- 范式对齐两处既有设施：
--   · games.rs jgg（九宫格）——「spend 票价 → 按权重抽签 → earn 派奖」的经济闭环与幂等键纪律；
--   · 0124 悬赏——发帖托管（spend_spark_tx 同事务）、楼主单向操作的权限口径。
-- 差异：论坛抽奖是「楼主出资、多人参与、定时开奖」——奖品是奖金池（发帖时冻结总奖金），
-- 参与者付票价凑份子（也可能免费），开奖时随机抽 N 名平分或按份分。
--
-- 简化口径（刻意）：奖金池 = winners × prize_per_winner（发帖时整池冻结）；
-- 票价可 0（免费抽奖，纯福利帖）。开奖由 worker 定时扫描（与 exam 周期结算同一模式），
-- 也允许楼主提前手动开奖。

CREATE TABLE IF NOT EXISTS topic_lotteries (
  topic_id BIGINT PRIMARY KEY REFERENCES topics(id) ON DELETE CASCADE,
  -- 中奖名额：开奖时随机抽这么多参与者
  winners INT NOT NULL DEFAULT 1,
  -- 每名中奖人获得的魔力（发帖时冻结 winners × prize_per_winner）
  prize_per_winner BIGINT NOT NULL DEFAULT 0,
  -- 参与票价（0 = 免费参与）
  ticket_spark INT NOT NULL DEFAULT 0,
  -- open=可参与 / drawn=已开奖 / cancelled=楼主取消（退回未开奖资金）
  status TEXT NOT NULL DEFAULT 'open',
  -- 开奖时间（到点 worker 扫描开奖；楼主也可提前手动开）
  draw_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS lottery_entries (
  topic_id BIGINT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  -- 开奖结果（drawn 后回填；未中为 FALSE）
  won BOOLEAN NOT NULL DEFAULT FALSE,
  PRIMARY KEY (topic_id, user_id)   -- 一人一次参与机会
);

CREATE INDEX IF NOT EXISTS idx_lottery_open ON topic_lotteries (status, draw_at) WHERE status = 'open';
CREATE INDEX IF NOT EXISTS idx_lottery_entries_topic ON lottery_entries (topic_id);
