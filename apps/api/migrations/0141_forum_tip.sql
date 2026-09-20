-- 0127 论坛 Phase2（第四片）：打赏（tip）
--
-- 调研结论：站内没有红包/打赏表（唯一 grep 命中是 staff-tools 的无关 tooltip 文案），
-- 但「魔力转移」范式已有三处先例可复刻：medal_gift（spend 全额 + gift_tax_bp 入站免池）、
-- magic-pool/donate（捐赠）、jgg（spend/earn 幂等对）。
--
-- 刻意取舍：本轮只做「楼层打赏」，不做策划案原案的红包（拼手气瓜分/等额/剩余退回）——
-- 红包需要「部分领取 + 过期退回 + 并发抢」三套状态机，复杂度接近抽奖但使用频率远低；
-- 打赏是单向即时转移，零状态机。先把高频路径做实，红包留待有真实需求再上。
--
-- 打赏与点赞同位（挂在楼层上），但走经济通道：spend(打赏人) + earn(作者) 同额对冲，
-- 与 gift_tax 口径对齐——论坛打赏不抽税（礼物税针对勋章交易；打赏是内容激励，
-- 抽税会抑制互动，与发帖 +2 的激励方向冲突）。

CREATE TABLE IF NOT EXISTS post_tips (
  id BIGSERIAL PRIMARY KEY,
  post_id BIGINT NOT NULL,
  topic_id BIGINT NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
  -- 打赏人/收款人（收款人即楼层作者，落列冗余存一份便于聚合查询）
  from_user BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  to_user BIGINT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  spark BIGINT NOT NULL CHECK (spark > 0),
  -- 附言（可选，≤50 字）
  note TEXT NOT NULL DEFAULT '',
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
-- post_id 无外键（posts 是分区表，与 post_likes 同处置）；topic_id 级联兜底清楼
CREATE INDEX IF NOT EXISTS idx_post_tips_post ON post_tips (post_id);
CREATE INDEX IF NOT EXISTS idx_post_tips_to_user ON post_tips (to_user);
-- 「我收到的打赏」聚合（个人页/消息通知用）
CREATE INDEX IF NOT EXISTS idx_post_tips_from_user ON post_tips (from_user);
