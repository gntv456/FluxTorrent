//! 免费券使用（0073）。
//! 从 economy_http.rs 按域拆出。

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

/// 用券：把一张未用未过期的券绑定到种子（CAS 防并发双用）。
/// 生效在 worker 计费侧：该种该用户的下载增量按 0 计（free）/上下行均 0 计（neutral），
/// 当累计下载超过种子大小 4% 时核销（Gazelle slop 口径——防买了券只下 1% 就转移给别人用）。
#[post("/me/vouchers/use")]
async fn voucher_use(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<VoucherUseReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    // temp_invite 券（0204 修死库存）：核销即发一枚邀请码，天数取
    // invite_admin_ttl_days（30 天口径），不绑种子。其余券种维持绑种子语义。
    let vkind: Option<String> = sqlx::query_scalar(
        "SELECT kind FROM user_vouchers WHERE id = $1 AND user_id = $2 \
         AND used_torrent_id IS NULL AND used_at IS NULL AND expires_at > now()",
    )
    .bind(body.voucher_id)
    .bind(auth.id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    match vkind.as_deref() {
        Some("temp_invite") => {
            let ttl_days = invite_admin_ttl_days(&state.repo.db).await;
            let code = crate::domain::new_invite_code();
            let expires = chrono::Utc::now() + chrono::Duration::days(ttl_days);
            // P-407：旧顺序是「CAS 销券（autocommit）→ 再发邀请码」。发码那步
            // 一旦失败（DB 抖动/约束），券已经是 used_at=now()，而这条 CAS 只认
            // used_at IS NULL ⇒ 重试被「券不存在、已使用或已过期」挡回：会员手里
            // 既没有券也没有码，那张券花出去的火花永久蒸发。同型口径见 P-364
            // （钱到账才落标记）与刚修的 P-406（撤销的 CAS 与退款同事务）。
            // 现在两条写进同一事务：任何一步失败 ⇒ 整体回滚，券与码一个都不动。
            let mut tx = state
                .repo
                .db
                .begin()
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            let id = issue_invite_tx(&mut tx, auth.id, &code, expires).await?;
            let n = sqlx::query(
                "UPDATE user_vouchers SET used_at = now() WHERE id = $1 \
                 AND user_id = $2 AND used_at IS NULL \
                 AND used_torrent_id IS NULL AND expires_at > now()",
            )
            .bind(body.voucher_id)
            .bind(auth.id)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
            if n == 0 {
                // 券在这瞬间被另一路用掉/过期：回滚，不把刚生成的码留在库里
                return Err(DomainError::Validation(
                    super::vouchers::why_unusable(
                        &state.repo.db,
                        body.voucher_id,
                        auth.id,
                    )
                    .await,
                ));
            }
            tx.commit()
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            state
                .repo
                .audit(Some(auth.id), "voucher.temp_invite", Some(id))
                .await;
            return Ok(ok(serde_json::json!({
                "voucher_id": body.voucher_id,
                "invite_id": id,
                "code": code,
                "expires_at": expires.to_rfc3339(),
            })));
        }
        None => {
            return Err(DomainError::Validation(
                super::vouchers::why_unusable(
                    &state.repo.db,
                    body.voucher_id,
                    auth.id,
                )
                .await,
            ));
        }
        // 0211 H2：绑种子语义只属于免费/中性券——rename_card 等其它券种
        // 误入此口 = 花钱买空气（绑上种子后既不生效也无消费出口），显式拒绝。
        Some(k) if k != "free" && k != "neutral" => {
            return Err(DomainError::Validation(format!(
                "该券类型（{k}）不能绑定种子；请在对应的入口使用"
            )));
        }
        _ => {}
    }
    // 绑定即生效但不置 used_at：worker 计费侧以「used_torrent_id 已绑定 + used_at IS NULL」
    // 判定生效中的券，下载量过阈值后由核销语句置 used_at。旧版绑定时就写 used_at，
    // 导致券永远不被计费侧匹配（用户花钱买的权益确定性为 0，真实资损）。
    let n = sqlx::query(
        "UPDATE user_vouchers SET used_torrent_id = $3 \
         WHERE id = $1 AND user_id = $2 AND used_torrent_id IS NULL AND used_at IS NULL AND expires_at > now()",
    )
    .bind(body.voucher_id)
    .bind(auth.id)
    .bind(body.torrent_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        // P-459：CAS 没打中不等于「用过或过期」——最常见的真因是这张券
        // 已经绑定在某张种子上生效中（面板还把它列在「可用券」里）。
        return Err(DomainError::Validation(
            super::vouchers::why_unusable(
                &state.repo.db,
                body.voucher_id,
                auth.id,
            )
            .await,
        ));
    }
    state
        .repo
        .audit(Some(auth.id), "voucher.use", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({
        "voucher_id": body.voucher_id,
        "torrent_id": body.torrent_id,
        "note": "已对该种子生效；下载量超过种子大小 4% 后自动核销"
    })))
}

/// 事务内发一枚邀请码（P-407）。
///
/// 与 `Repo::issue_invite` 是同一条 INSERT：之所以在本地再写一遍，是券核销必须
/// 与发码同事务，而 `repo/mod.rs` 已压在行数基线上（多一行就红）。
/// 动任何一侧请同步另一侧——只有本口走事务版。
async fn issue_invite_tx(
    tx: &mut sqlx::PgTransaction<'_>,
    inviter_id: i64,
    code: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> DomainResult<i64> {
    Ok(sqlx::query_scalar::<_, i64>(
        "INSERT INTO invites (inviter_id, code, expires_at) \
         VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(inviter_id)
    .bind(code)
    .bind(expires_at)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?)
}

/// 管理端直发邀请码 TTL（0204 配置化）：site_settings.invite_admin_ttl_days，
/// 缺省 30，clamp 1..=365。temp_invite 券核销与 admin 直发同口径。
pub async fn invite_admin_ttl_days(db: &sqlx::PgPool) -> i64 {
    sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'invite_admin_ttl_days'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .and_then(|v: String| v.parse().ok())
    .map(|v: i64| v.clamp(1, 365))
    .unwrap_or(30)
}

// ============ 我的火花（M11） ============

#[get("/me/spark")]
async fn my_spark(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let balance: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1")
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
    // 收益因子说明（设计稿：1.03x/5x/0.1x 可点击展开）—— 由做种状态推导
    let seeding_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM snatches WHERE user_id = $1 AND seeding",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    // 与 worker 同口径的僵尸阈值：max(2h, 2×announce_interval)（详见 worker 的 stale_peer_threshold_secs）
    let announce: i64 = sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'announce_interval'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .and_then(|v| v.parse::<i64>().ok())
    .map(|v| v.clamp(60, 86400))
    .unwrap_or(1800);
    let stale_secs = (announce * 2).max(7200);
    // 收益构成：按规则名分组（种子档位 + 做种时长档位两类），与 worker 结算同源。
    // 0133 后新增「做种时长」维度 —— 玩家能看到"我为什么拿这么多"，这是相对 NP 的体验优势。
    let my_rules: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT rule, count(*) FROM (
            SELECT CASE
                     WHEN t.seeders <= 1 AND t.times_completed >= 3 THEN '濒危保种'
                     WHEN now() - t.created_at > interval '365 days' THEN '高龄种'
                     WHEN now() - t.created_at > interval '180 days' THEN '老种'
                     WHEN t.size >= 107374182400 THEN '大体积种'
                     WHEN t.size >= 26843545600 THEN '中体积种'
                     ELSE '日常种'
                   END AS rule
            FROM snatches s JOIN torrents t ON t.id = s.torrent_id
            WHERE s.user_id = $1 AND s.seeding
              AND NOT (s.connectable = 0 AND s.uploaded = 0)
              AND s.last_seen_at > now() - ($2::bigint * interval '1 second')
            UNION ALL
            SELECT CASE
                     WHEN s.seeded_seconds >= 31536000 THEN '传奇做种者（≥1年）'
                     WHEN s.seeded_seconds >= 15552000 THEN '资深做种者（6-12月）'
                     WHEN s.seeded_seconds >= 7776000 THEN '稳定做种者（3-6月）'
                     WHEN s.seeded_seconds >= 2592000 THEN '新晋做种者（1-3月）'
                     ELSE '新做种（<1月）'
                   END AS rule
            FROM snatches s
            WHERE s.user_id = $1 AND s.seeding
              AND NOT (s.connectable = 0 AND s.uploaded = 0)
              AND s.last_seen_at > now() - ($2::bigint * interval '1 second')
        ) x GROUP BY rule ORDER BY count(*) DESC
        "#,
    )
    .bind(auth.id)
    .bind(stale_secs)
    .fetch_all(&state.repo.db)
    .await
    .unwrap_or_default();
    // 真实预估（与 worker 结算同一套 DB 函数 + 同一套过滤，迁移 0133）：
    // 不再是 "10 + 颗数×2" 的粗估。
    let hourly_estimate: i64 = sqlx::query_scalar(
        r#"
        WITH p AS MATERIALIZED (SELECT * FROM seeding_params()),
        s AS (
            SELECT sum(seeding_torrent_bonus(
                       t.size, t.seeders,
                       (EXTRACT(EPOCH FROM (now() - t.created_at)) / 86400.0)::double precision,
                       t.times_completed,
                       (GREATEST(sn.seeded_seconds, 0) / 3600.0)::double precision,
                       p.vol_base, p.rarity_k, p.rarity_exp, p.scale)) AS bonus_raw,
                   bool_or(donor_privileged(u.id)) AS donor
            FROM snatches sn
            JOIN torrents t ON t.id = sn.torrent_id
            JOIN users u ON u.id = sn.user_id
            CROSS JOIN p
            WHERE sn.user_id = $1 AND sn.seeding
              AND NOT (sn.connectable = 0 AND sn.uploaded = 0)
              AND sn.last_seen_at > now() - ($2::bigint * interval '1 second')
        )
        SELECT seeding_hourly(COALESCE(bonus_raw, 0), p.base, p.cap, p.curve_k, p.donor_mult,
                              COALESCE(donor, FALSE))
        FROM s CROSS JOIN p
        "#,
    )
    .bind(auth.id)
    .bind(stale_secs)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    // 结构化系数表（0417 F6）：档位/系数从 seeding_params()（站长可调）取真值，
    // 阈值常量与 worker 结算 CASE 分支同源。formula_note 保留（人话全文），
    // tiers 供前端渲染表格——此前只有一行长文，资深用户没法算账。
    let (base, cap, curve_k, vol_base, rarity_k, scale, donor_mult): (
        f64, f64, f64, f64, f64, f64, f64,
    ) = sqlx::query_as(
        "SELECT base::float8, cap::float8, curve_k::float8, \
                vol_base::float8, rarity_k::float8, scale::float8, \
                donor_mult::float8 FROM seeding_params()",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or((10.0, 150.0, 0.12, 50.0, 0.6, 0.4, 2.0));
    let tiers = serde_json::json!({
        "formula": {
            "base_per_hour": base,
            "cap_per_hour": cap,
            "curve_k": curve_k,
            "scale": scale,
            "donor_mult": donor_mult,
            "note": "每小时 = 底薪 + (2/π)·封顶·atan(Σ加成 × 曲线系数)，捐赠者整笔翻倍",
        },
        "torrent_tier": [
            { "name": "濒危保种", "score": 2.0, "cond": "做种者≤1 且 完成≥3" },
            { "name": "高龄种",   "score": 1.5, "cond": "种龄 > 365 天" },
            { "name": "老种",     "score": 1.0, "cond": "种龄 > 180 天" },
            { "name": "大体积种", "score": 0.75, "cond": "体积 ≥ 100GB" },
            { "name": "中体积种", "score": 0.5,  "cond": "体积 ≥ 25GB" },
            { "name": "日常种",   "score": 0.25, "cond": "其余" },
        ],
        "seed_hours_tier": [
            { "name": "≥1年",  "score": 2.0 },
            { "name": "6-12月", "score": 1.0 },
            { "name": "3-6月", "score": 0.75 },
            { "name": "1-3月", "score": 0.5 },
        ],
        "volume": {
            "vol_base": vol_base,
            "note": "对数饱和：≥50GB 记满分，1GB 约 0.18",
        },
        "rarity": {
            "k": rarity_k,
            "note": "独苗 ×1.6、人多趋近 ×1.0",
        },
    });
    Ok(ok(serde_json::json!({
        "balance": balance,
        "seeding_count": seeding_count,
        "hourly_estimate": hourly_estimate,
        // 兼容旧消费点（my-spark pill 渲染）：保持 [规则名, 颗数] 数组形态
        "reward_rules": my_rules,
        "tiers": tiers,
        "formula_note": "每小时 = 底薪 + (2/π)·封顶·atan(Σ加成 × 曲线系数)，捐赠者整笔翻倍。每颗种子加成 =（种子档位分 + 做种时长档位分）× 体积因子 × 稀有度加成 × 标定系数；种子档位：濒危保种 2.0 / 高龄种 1.5 / 老种 1.0 / 大体积 0.75 / 中体积 0.5 / 日常 0.25；做种时长档位：≥1年 +2.0 / 6-12月 +1.0 / 3-6月 +0.75 / 1-3月 +0.5；体积按对数饱和（≥50GB 记满分，1GB 约 0.18）；稀有度独苗 ×1.6、人多趋近 ×1.0。",
    })))
}

#[derive(Deserialize)]
pub(super) struct VoucherUseReq {
    pub(super) voucher_id: i64,
    /// 免费券/中性券绑种子必填；temp_invite 券核销不消费此字段（可传 0）
    #[serde(default)]
    pub(super) torrent_id: i64,
}
