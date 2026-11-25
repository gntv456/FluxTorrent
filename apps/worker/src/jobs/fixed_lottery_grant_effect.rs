//! 权益类奖品发放（worker 侧直写版）。自 fixed_lottery_fulfill.rs 拆出
//! 守 300 行门禁；镜像 flux_shared::apply_item_effect 的同五个分支（改动须同步）。

use sqlx::PgPool;

/// 权益类效果（invite/vip/ad_free/rename_card/upload_credit）的 worker 侧
/// 直写版——镜像 flux_shared::apply_item_effect 的同五个分支（改动须同步）。
pub(crate) async fn grant_effect(
    db: &PgPool,
    uid: i64,
    kind: &str,
    cfg: &serde_json::Value,
) -> anyhow::Result<()> {
    match kind {
        "upload_credit" => {
            let gb = cfg.get("gb").and_then(|v| v.as_i64()).unwrap_or(0);
            let bytes = gb * 1024 * 1024 * 1024;
            sqlx::query(
                "INSERT INTO traffic_ledger \
                 (id, user_id, torrent_id, delta_up, delta_down, window_start) \
                 VALUES (nextval('traffic_ledger_id_seq'), \
                      $1, 0, $2, 0, now())",
            )
            .bind(uid)
            .bind(bytes)
            .execute(db)
            .await?;
            sqlx::query(
                "UPDATE users SET uploaded = uploaded + $2 WHERE id = $1",
            )
            .bind(uid)
            .bind(bytes)
            .execute(db)
            .await?;
        }
        "invite" => {
            let code: String = sqlx::query_scalar(
                "SELECT string_agg( 
                     substr('abcdefghjkmnpqrstuvwxyz23456789', g, 1), '') 
                 FROM generate_series(1, 16) g 
                 ORDER BY random()",
            )
            .fetch_one(db)
            .await?;
            sqlx::query(
                "INSERT INTO invites (inviter_id, code, expires_at) \
                 VALUES ($1, $2, now() + interval '72 hours')",
            )
            .bind(uid)
            .bind(&code)
            .execute(db)
            .await?;
        }
        "vip" => {
            let days = cfg.get("days").and_then(|v| v.as_i64()).unwrap_or(7);
            sqlx::query(
                "UPDATE users SET vip_until = \
                    GREATEST(COALESCE(vip_until, now()), now()) \
                    + make_interval(days => $2::int) WHERE id = $1",
            )
            .bind(uid)
            .bind(days as i32)
            .execute(db)
            .await?;
        }
        "ad_free" => {
            let days = cfg.get("days").and_then(|v| v.as_i64()).unwrap_or(15);
            sqlx::query(
                "UPDATE users SET donor_until = \
                    GREATEST(COALESCE(donor_until, now()), now()) \
                    + make_interval(days => $2::int) WHERE id = $1",
            )
            .bind(uid)
            .bind(days as i32)
            .execute(db)
            .await?;
        }
        "rainbow_id" => {
            let item_id =
                cfg.get("item_id").and_then(|v| v.as_i64()).unwrap_or(0);
            sqlx::query(
                "INSERT INTO user_dressups (user_id, item_id, source) \
                 VALUES ($1, $2, 'lottery') \
                 ON CONFLICT (user_id, item_id) DO NOTHING",
            )
            .bind(uid)
            .bind(item_id)
            .execute(db)
            .await?;
        }
        "rename_card" => {
            sqlx::query(
                "INSERT INTO user_vouchers (user_id, kind, source) \
                 VALUES ($1, 'rename_card', 'lottery')",
            )
            .bind(uid)
            .execute(db)
            .await?;
        }
        other => anyhow::bail!("unknown effect kind {other}"),
    }
    Ok(())
}
