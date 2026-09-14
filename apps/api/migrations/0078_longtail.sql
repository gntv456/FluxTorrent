-- 0078 v3 长尾批次（对照 v3 §27-13/15/16/17/18/20/22 ⏳ 项清零）：
--   A) 定向众筹免费（HDBits Featured 口径）：某种子社区凑火花 → 达标自动挂 free×时长
--   B) 泄露者检测（U3D LeakerController 简化离线版）：首发下载时窗+来路聚合，staff 复核
--   C) 论坛已读跟踪（NP readposts 口径）：user/topic → 已读至楼层
--   D) 工单体系（U3D Ticket）：staffmessages 工单化（状态/优先级/指派）
--   E) Refundable（U3D 口径）：促销档 refundable——按做种时长线性退还下载量
--   F) 赠送税（Gazelle 奖池税口径）：礼物/转账抽 5% 入站免池（回收通道，函数实现）

-- A) 定向众筹
CREATE TABLE IF NOT EXISTS fundings (
  id BIGSERIAL PRIMARY KEY,
  torrent_id BIGINT NOT NULL UNIQUE REFERENCES torrents(id) ON DELETE CASCADE,
  creator_id BIGINT NOT NULL REFERENCES users(id),
  goal BIGINT NOT NULL,                       -- 火花目标
  hours INT NOT NULL DEFAULT 168,             -- 达标后 free 时长（默认 7 天）
  raised BIGINT NOT NULL DEFAULT 0,           -- 实筹（含税后入账）
  status SMALLINT NOT NULL DEFAULT 0,         -- 0=进行中 1=达标已挂促销 2=到期未达标退款 3=管理员关闭
  ends_at TIMESTAMPTZ NOT NULL,
  promoted_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS funding_contribs (
  id BIGSERIAL PRIMARY KEY,
  funding_id BIGINT NOT NULL REFERENCES fundings(id) ON DELETE CASCADE,
  user_id BIGINT NOT NULL REFERENCES users(id),
  amount BIGINT NOT NULL,                     -- 用户实付（税前）
  tax BIGINT NOT NULL DEFAULT 0,              -- 赠送税入池部分（F 项联动）
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  UNIQUE (funding_id, user_id)                -- 一人一项目可追投：追投走 UPDATE amount
);
CREATE INDEX IF NOT EXISTS idx_fundings_open ON fundings (status, ends_at);

-- B) 泄露者检测事件表（worker 离线扫描写入；只报告不处罚）
CREATE TABLE IF NOT EXISTS leak_events (
  id BIGSERIAL PRIMARY KEY,
  kind TEXT NOT NULL,                         -- 'first_dl_far' 首发 IP 远离 / 'passkey_multi_ip' passkey 多地使用
  user_id BIGINT NOT NULL REFERENCES users(id),
  torrent_id BIGINT REFERENCES torrents(id) ON DELETE CASCADE,
  detail JSONB NOT NULL DEFAULT '{}',         -- 证据（IP/距离/时间窗/agent）
  score SMALLINT NOT NULL DEFAULT 0,          -- 风险分（0-100，展示排序用）
  resolved SMALLINT NOT NULL DEFAULT 0,       -- 0=待复核 1=已确认泄露 2=误报
  resolved_by BIGINT REFERENCES users(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX IF NOT EXISTS idx_leak_open ON leak_events (resolved, created_at DESC);

-- C) 论坛已读（NP readposts：user_id+topic_id → 已读到的楼层；行=有新帖）
CREATE TABLE IF NOT EXISTS topic_reads (
  user_id BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  topic_id BIGINT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  last_post_id BIGINT NOT NULL DEFAULT 0,     -- 已读至该帖（0=仅标记）
  read_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  PRIMARY KEY (user_id, topic_id)
);

-- D) 工单（U3D Ticket 口径，挂在既有 staffmessages 上补流转字段）
ALTER TABLE staffmessages ADD COLUMN IF NOT EXISTS priority SMALLINT NOT NULL DEFAULT 0;   -- 0低1中2高3紧急
ALTER TABLE staffmessages ADD COLUMN IF NOT EXISTS assigned_to BIGINT REFERENCES users(id);
ALTER TABLE staffmessages ADD COLUMN IF NOT EXISTS ticket_status SMALLINT NOT NULL DEFAULT 0; -- 0=新 1=处理中 2=已答复待确认 3=关闭
-- 既有 answered=1 的历史件映射为关闭（幂等：仅迁移时执行一次）
UPDATE staffmessages SET ticket_status = 3 WHERE answered = 1 AND ticket_status = 0;
CREATE INDEX IF NOT EXISTS idx_staffmsg_ticket ON staffmessages (ticket_status, priority DESC, id DESC);

-- E) Refundable：促销枚举加档（ billed 口径见 worker：refund 促销期间下载按做种时长线性退还 ）
ALTER TYPE promotion_kind_enum ADD VALUE IF NOT EXISTS 'refundable';

-- F) 赠送税（site_settings 可调：gift_tax_bp 缺省 500 = 5%；0=免税）
INSERT INTO site_settings (name, value) VALUES ('gift_tax_bp', '500')
ON CONFLICT (name) DO NOTHING;

-- 众筹税/礼物税入池统一记 pool_donations（month 站点时区），账目进 v_spark_flow_monthly 的 burned 口径
