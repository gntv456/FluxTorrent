-- 0417 求种加价（VIP 深测 F1）：requests_bids 表 + 列表真数据底座。
--
-- 背景：求种列表的 latest_bounty / bids 两列此前是兜底伪数据
-- （requests.rs 注释自认「加价不存在，回 bounty 就是两列恒等伪数据」）。
-- 本迁移建 bids 明细表；requests.bounty 维持「冻结总额」口径不变——
-- 建单冻结 + 后续加价都往里累加，应种交付时按总额一次性转移
-- （fulfill 事务已按 bounty>0 全额支付，加价入 bounty 后无需改 fulfill）。
--
-- 幂等：CREATE TABLE IF NOT EXISTS + 注释；无数据搬迁，重跑安全。

CREATE TABLE IF NOT EXISTS request_bids (
  id          BIGSERIAL PRIMARY KEY,
  request_id  BIGINT NOT NULL REFERENCES requests(id) ON DELETE CASCADE,
  -- 加价人（含楼主续押；他人加价进悬赏池、应种交付时一并归应种人）
  user_id     BIGINT NOT NULL REFERENCES users(id),
  amount      BIGINT NOT NULL CHECK (amount > 0),
  -- 幂等键（防网络重试双扣）：uid 前缀 + 客户端键，同建单口径
  idem_key    TEXT,
  created_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- 列表页取「最新一笔加价」与「加价人次」的两条查询路径
CREATE INDEX IF NOT EXISTS request_bids_req_idx
  ON request_bids(request_id, id DESC);

-- 幂等键唯一（NULL 不受限——无客户端键的老客户端走 request_id 兜底键）
CREATE UNIQUE INDEX IF NOT EXISTS request_bids_idem_uq
  ON request_bids(idem_key) WHERE idem_key IS NOT NULL;

COMMENT ON TABLE request_bids IS
  '求种加价明细：楼主续押与他人加价共用；requests.bounty 为冻结总额（建单+加价累加），应种时全额转移';
