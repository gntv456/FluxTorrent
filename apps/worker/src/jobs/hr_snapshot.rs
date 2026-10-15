//! H&R 快照建表（自 hr.rs 抽出，守 300 行门禁）。
//!
//! 0380 seedbox 豁免也在这条 SQL 里：盒子用户经人工审核拿到 `hnr_exempt`
//! 后不再建快照——他们的流量来自上传而非做种，H&R 记不到这份功。
//! 语义边界见 `domain/seedbox` 模块头：只豁免「因不可探测被误判」那一类，
//! 幽灵做种、流量虚报仍照常检测。

use sqlx::PgPool;

use super::hr::SiteDefaults;

pub(super) async fn build_snapshots(
    db: &PgPool,
    d: &SiteDefaults,
) -> anyhow::Result<u64> {
    // 1) 为新完成的下载建快照（幂等）；免费窗口内完成的不建快照（豁免）
    let n = sqlx::query(
        r#"
        INSERT INTO hr_snapshots
            (user_id, torrent_id, required_seconds, deadline)
        SELECT s.user_id, s.torrent_id,
               COALESCE((t.hr_policy->>'seed_hours')::int, $2::int)
                   * 3600,
               s.completed_at + make_interval(
                   days => COALESCE(
                       (t.hr_policy->>'days')::int, $3::int))
        FROM snatches s
        JOIN torrents t ON t.id = s.torrent_id
        WHERE s.completed_at
          > now() - make_interval(hours => $1::int)
          -- 0286：只对「当前对外可见」的种子建义务。此前无此判定，
          -- 待审/被拒/回收站里的种子同样给会员记 H&R 责任（且删种会连带
          -- 级联掉证据，等于一边误判一边免单）。
          AND t.approval_status = 1
          AND NOT EXISTS (SELECT 1 FROM hr_snapshots h
                           WHERE h.user_id = s.user_id
                             AND h.torrent_id = s.torrent_id)
          AND COALESCE(t.hr_policy->>'enabled', 'true')::boolean
          -- 0072 buffer 豁免（U3D hitrun.buffer 口径）：下载量不足种子 10% 视为误触/秒删，不计 H&R
          -- 与券核销同阈值（≈10.4%）：10%~10.4% 区间不再误判
      AND s.downloaded > t.size * 104 / 1000
          AND NOT EXISTS (
              SELECT 1 FROM promotions p
              WHERE (p.torrent_id = s.torrent_id
                     OR (p.torrent_id IS NULL AND (
                         p.scope = 'global'
                         OR (p.scope = 'official' AND t.official_tag)
                         OR (p.scope = 'non_official' AND NOT t.official_tag)
                         OR (p.scope = 'category'
                             AND t.category_id = p.category_id))))
                AND p.starts_at <= s.completed_at AND p.ends_at > s.completed_at
                AND p.kind IN ('free', 'x2free')
          )
          -- 保种员 / VIP 持 hr.exempt 权限 → 免除 H&R，不建快照
          AND NOT user_can(s.user_id, 'hr.exempt')
          -- 0380 seedbox：经人工审核拿到 hnr_exempt 的盒子用户同样免建快照
          --（他们的流量来自上传而非做种，H&R 记不到这份功）。
          -- 语义边界见 domain/seedbox 模块头：只豁免「因不可探测被误判」，
          -- 幽灵做种、流量虚报仍照常检测。
          AND NOT EXISTS (
              SELECT 1 FROM seedbox_profiles sp
              WHERE sp.user_id = s.user_id AND sp.hnr_exempt
          )
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(d.lookback_h as i32)
    .bind(d.seed_hours as i32)
    .bind(d.days as i32)
    .execute(db)
    .await?
    .rows_affected();
    Ok(n)
}
