-- 0405 seedbox_effective 与 domain::Flags::from_profile 拉平（P-426b）
--
-- 背景：0380 建的这张视图是**反作弊侧唯一的读取口**
-- （worker seeding.rs:72 乘 bonus_factor、process_event.rs:123 读 speed_relax），
-- 而「标志位到底给不给」的成文口径写在 apps/api/src/domain/seedbox/mod.rs 的
-- `Flags::from_profile`：
--   · hnr_exempt（免 H&R）与 bonus_factor（保种倍率）是**强承诺**，
--     只有人工审核背书（approved）才算，自动识别（detected）一律不给；
--   · speed_relax 反过来：approved **或** detected 都给
--     （检测只放宽「要不要记作弊线索」，不放宽计费，误判成本低）。
-- 旧视图三处都不符：
--   1) hnr_exempt = COALESCE(hnr_exempt,false) —— 没过 approved 闸；
--   2) bonus_factor = COALESCE(bonus_factor,1.0) —— 没过 approved 闸；
--   3) speed_relax = anticheat_speed_relax —— 少了 `OR detected`。
-- 前两处在今天被写入侧兜住了（review() 落库时写的是 approve && ...，
-- P-426a 又补上了驳回清零），所以现网读不到脏值；但视图是**执法侧**的定义，
-- 它不该依赖调用方守规矩——一旦 ASN 自动识别（detected）落地，
-- 历史脏行或被驳回行留下的倍率就会绕过「人工背书」这道门直接生效。
-- 第 3 条是反方向：asn 识别出的盒子本该免速度告警，旧视图不给 ⇒ 口径比
-- domain 更严，用户申报的既得利益被 silently 收回。
--
-- 本迁移把视图改成与 `from_profile` 字面同一条尺（含 GREATEST(...,1.0) 钳位）。
-- 配套：worker hr_snapshot.rs 原先直读基表 seedbox_profiles.hnr_exempt，
-- 一并改读视图，免得两处口径各自漂移。

CREATE OR REPLACE VIEW seedbox_effective AS
SELECT
    p.user_id,
    p.approved OR p.detected AS is_seedbox,
    p.approved AS user_declared,
    p.detected AS auto_detected,
    (p.approved AND COALESCE(p.hnr_exempt, false)) AS hnr_exempt,
    CASE
        WHEN p.approved THEN GREATEST(COALESCE(p.bonus_factor, 1.0), 1.0)
        ELSE 1.0
    END AS bonus_factor,
    (COALESCE(p.anticheat_speed_relax, false) OR p.detected) AS speed_relax
FROM seedbox_profiles p
WHERE p.approved OR p.detected;

COMMENT ON VIEW seedbox_effective IS
    'seedbox 生效标志位（反作弊唯一读取口）。口径与 '
    'domain::Flags::from_profile 同步：hnr_exempt/bonus_factor 仅人工审核 '
    '(approved) 给；speed_relax 由 approved 或 ASN 自动识别 (detected) 给。'
    '见 0405。';
