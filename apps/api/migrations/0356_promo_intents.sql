-- 0356：促销 intent/快照改造。
-- 背景：人工促销此前只有 promotions 结果行——「这个种子为什么是这个倍率」
--       在直挂/删除后无从追溯（promo_set 直写 + freeleech_delete 物理删除）。
-- 方案：促销意图（promo_intents）先落库，审批通过才写 promotions；
--       管理组直挂（freeleech_set 现状保留）也同步落一条 approved intent 留痕。
--       审批时点对规则字段打 sha3-256 快照（snapshot_hash），事后可校验未篡改。
--
-- scope 用 TEXT + CHECK 而非复用 promotion_scope 枚举：
--   0044 教训——新增枚举值不能在添加它的同一事务中被引用；
--   intent 是审批队列数据，与 promotions 的执行口径解耦更稳
--   （值域约束与 promotions.scope::text 完全一致）。

CREATE TABLE promo_intents (
  id BIGSERIAL PRIMARY KEY,
  scope TEXT NOT NULL
    CHECK (scope IN ('torrent','global','official','non_official','category')),
  torrent_id BIGINT REFERENCES torrents(id) ON DELETE CASCADE,
  category_id BIGINT REFERENCES categories(id),
  kind TEXT NOT NULL REFERENCES promo_kinds(kind),
  starts_at TIMESTAMPTZ NOT NULL,
  ends_at TIMESTAMPTZ NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending','approved','rejected','cancelled','expired')),
  requested_by BIGINT NOT NULL REFERENCES users(id),
  decided_by BIGINT REFERENCES users(id),
  decided_at TIMESTAMPTZ,
  decide_note TEXT,
  snapshot_hash TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
  CHECK ((scope = 'torrent') = (torrent_id IS NOT NULL)),
  CHECK ((scope = 'category') = (category_id IS NOT NULL))
);

-- 待审队列按状态局部索引（审批面板只扫 pending）
CREATE INDEX idx_promo_intents_status
  ON promo_intents (status) WHERE status = 'pending';
