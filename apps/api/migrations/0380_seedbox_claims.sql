-- 0380 seedbox 盒子申报（PT Studio FR-SBOX-001~011 对标）
--
-- 现状与问题：反作弊有 connectable 四档（0071），其中 -2 SUSPECT 是
-- 「加密/CGNAT 无法探测」时的**猜测**——它缓解误伤，但猜不准：
--   * 真盒子用户被当成可疑 → 速度规则被收紧 → 投诉无出口；
--   * 真做假种的人自称盒子 → 若无人工审核即可骗到豁免。
-- 本迁移把「猜」升级为「用户自证 + 管理员审核 + 系统辅助识别」：
--
--   1. 用户申报 CIDR（多段），提交后 pending；
--   2. 管理员审核 → approved（可信）/ rejected（驳回，带理由）；
--   3. 系统从 announce 流量里识别数据中心 ASN → detected（仅提示，不自动处罚）；
--   4. approved/detected 的用户带 seedbox 标志位，供反作弊读：
--      hnr_exempt（H&R 豁免）/ bonus_factor（保种倍率加成）
--      / anticheat_speed_relax（速度规则放宽）。
--
-- 【安全边界】标志位只**放宽**不**加重`：approved 也不豁免其它欺诈
-- （幽灵做种、流量虚报），只对「因不可探测而被误判」这一类开窗。
-- detected 永不自动处罚——ASN 抽样天然有误判，必须人工确认。

-- ① 申报单：一人一单，可多次提交（保留历史供审计）
CREATE TABLE IF NOT EXISTS seedbox_claims (
    id          BIGSERIAL PRIMARY KEY,
    user_id     BIGINT      NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    cidrs       TEXT[]      NOT NULL,
    -- pending=待审 / approved=通过 / rejected=驳回
    status      TEXT        NOT NULL DEFAULT 'pending'
                            CHECK (status IN ('pending', 'approved', 'rejected')),
    reviewer_id BIGINT,
    review_note TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    reviewed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_seedbox_claims_status
    ON seedbox_claims (status, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_seedbox_claims_user
    ON seedbox_claims (user_id, created_at DESC);

COMMENT ON TABLE seedbox_claims IS
    'seedbox 盒子申报。approved 是用户自证通道（人工审核），detected 记在 seedbox_profiles。两者都不自动处罚，只放宽「因不可探测被误判」那一类规则。';

-- ② 用户画像：标志位的唯一读取入口（反作弊只读这里，不直接碰申报单）
CREATE TABLE IF NOT EXISTS seedbox_profiles (
    user_id     BIGINT      PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    -- 用户自证且已审核通过（可信）
    approved    BOOLEAN     NOT NULL DEFAULT false,
    -- 系统从 ASN 抽样识别（仅提示，可能误判）
    detected    BOOLEAN     NOT NULL DEFAULT false,
    -- 检测到的数据中心 ASN 列表（JSONB 数组，运维可见）
    detected_asns JSONB     NOT NULL DEFAULT '[]'::jsonb,
    -- 反作弊读的三只标志
    hnr_exempt            BOOLEAN NOT NULL DEFAULT false,
    bonus_factor          NUMERIC(6, 2) NOT NULL DEFAULT 1.0,
    anticheat_speed_relax BOOLEAN NOT NULL DEFAULT false,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON COLUMN seedbox_profiles.hnr_exempt IS
    'H&R 豁免：盒子用户做种不计入 H&R 达标（他们的流量来自上传而非做种）。';
COMMENT ON COLUMN seedbox_profiles.bonus_factor IS
    '保种倍率加成（默认 1.0）。盒子保种是站点稀缺资源，值得额外奖励。';
COMMENT ON COLUMN seedbox_profiles.anticheat_speed_relax IS
    '速度规则放宽：加密/CGNAT 环境下速度波动大，不按可疑处理。';
COMMENT ON COLUMN seedbox_profiles.detected IS
    '系统识别（数据中心 ASN 抽样）。**永不自动处罚**——ASN 抽样天然有误判，只作提示与审核参考。';

-- ③ 生效视图：反作弊唯一该读的东西（一行一人，无则视为非盒子）
CREATE OR REPLACE VIEW seedbox_effective AS
SELECT
    p.user_id,
    (p.approved OR p.detected) AS is_seedbox,
    p.approved                    AS user_declared,
    p.detected                    AS auto_detected,
    COALESCE(p.hnr_exempt, false)            AS hnr_exempt,
    COALESCE(p.bonus_factor, 1.0)            AS bonus_factor,
    COALESCE(p.anticheat_speed_relax, false) AS speed_relax
FROM seedbox_profiles p
WHERE p.approved OR p.detected;

COMMENT ON VIEW seedbox_effective IS
    '盒子生效态（approved OR detected）。反作弊/H&R/保种倍率统一读这里，'
    '避免各处各写一套 JOIN 判据导致口径漂移。';

-- ④ 幂等种子：不做默认数据。标志位一律 false，等真实申报。
