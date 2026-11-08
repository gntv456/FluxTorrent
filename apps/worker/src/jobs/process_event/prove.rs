//! 判据证据取数（判据本体在 [`super::seeding_gate`]，这里只管查库）。
//! 从 process_event.rs 按域拆出（行数棘轮）；2026-11-09 辅种通路批把
//! 佐证查询与本批新增的 uploaded_total / has_demand 两个证据位收拢到这里。

/// 一次 announce 的站点侧证据（均为已入账口径，不接受客户端自报累计）。
pub(super) struct Proof {
    /// 被 corr_room 约束过的佐证总量（xreport 管道；线上尚无客户端接入，恒 None）
    pub(super) corroborated: Option<i64>,
    /// 该用户在本种的终身已入账上传（snatches.uploaded，速率钳后记账）
    pub(super) uploaded_total: i64,
    /// 本种是否发生过真实下载行为（存在 downloaded>0 的 snatch）
    pub(super) has_demand: bool,
}

pub(super) async fn gather(
    exec: &mut sqlx::PgConnection,
    user: i64,
    torrent_id: i64,
) -> anyhow::Result<Proof> {
    let corroborated: Option<i64> = sqlx::query_scalar(
        "SELECT bytes FROM upload_corroborated \
         WHERE uploader_id = $1 AND torrent_id = $2",
    )
    .bind(user)
    .bind(torrent_id)
    .fetch_optional(&mut *exec)
    .await?;
    let uploaded_total: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT uploaded FROM snatches \
          WHERE user_id = $1 AND torrent_id = $2), 0)",
    )
    .bind(user)
    .bind(torrent_id)
    .fetch_one(&mut *exec)
    .await?;
    // 需求门槛走 idx_snatches_torrent 单次探测；downloaded>0 覆盖
    // 「有人在下还没完」的活种，completed_at 兜住零倍率/零下载完成的边角
    let has_demand: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM snatches \
          WHERE torrent_id = $1 \
            AND (downloaded > 0 OR completed_at IS NOT NULL))",
    )
    .bind(torrent_id)
    .fetch_one(&mut *exec)
    .await?;
    Ok(Proof {
        corroborated,
        uploaded_total,
        has_demand,
    })
}
