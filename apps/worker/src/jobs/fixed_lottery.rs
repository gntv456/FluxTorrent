//! 固定抽签 worker 侧（0520）：到期开奖扫描 + 履约重试扫描。
//!
//! 与 api 侧 services::fixed_lottery::draw 共用同一套 CAS 协议——这里只做
//! 「哪些活动到期了」的扫描，开奖本体在 api 侧 service（含 seed/排序/落子）。
//! worker 直接连库，走 repo 等价 SQL；fulfill_one 逻辑复制自 services
//! （worker 与 api 是两个 crate，共用代码走 include 惯例在本仓太重，
//! 两处 60 行内的小逻辑允许镜像，改动须同步）。

use sqlx::PgPool;

/// 到期/达标扫描（60s tick）。返回处理的活动数。
pub(crate) async fn fixed_lottery_settle_due(
    db: &PgPool,
) -> anyhow::Result<u64> {
    // scheduled：到点；threshold：保底时间到（达标触发由 api 报名口同步做，
    // 这里只兜「报名瞬间并发不足、后慢慢攒够但没人再报名」的边缘——阈值
    // 达标扫描每分钟补一刀，不漏）
    let due: Vec<i64> = sqlx::query_scalar(
        "SELECT id FROM fixed_lottery_campaigns \
         WHERE status = 'open' AND ( \
           (trigger_type = 'scheduled' \
            AND scheduled_draw_at IS NOT NULL \
            AND scheduled_draw_at <= now()) \
           OR (trigger_type = 'threshold' \
               AND fallback_draw_at IS NOT NULL \
               AND fallback_draw_at <= now()) \
           OR (trigger_type = 'threshold' \
               AND threshold_count IS NOT NULL \
               AND threshold_count >= 1 \
               AND (SELECT count(*) FROM fixed_lottery_participants p \
                    WHERE p.campaign_id \
                      = fixed_lottery_campaigns.id) >= threshold_count) \
         ) LIMIT 50",
    )
    .fetch_all(db)
    .await?;
    let mut done = 0u64;
    for cid in due {
        match api_draw(db, cid, None).await {
            Ok((false, _)) => done += 1,
            Ok((true, _)) => {} // 已被并发（手动/报名触发）抢先
            Err(e) => {
                tracing::warn!(campaign_id = cid, error = %e,
                    "fixed_lottery settle failed");
            }
        }
    }
    Ok(done)
}

/// 履约重试扫描（60s tick）：pending/failed 且退避到点的行。
pub(crate) async fn fixed_lottery_fulfill_due(
    db: &PgPool,
) -> anyhow::Result<u64> {
    let due: Vec<i64> = sqlx::query_scalar(
        "SELECT winner_id FROM fixed_lottery_fulfillments \
         WHERE status IN ('pending', 'failed') \
           AND (next_retry_at IS NULL OR next_retry_at <= now()) \
         LIMIT 100",
    )
    .fetch_all(db)
    .await?;
    let mut done = 0u64;
    for wid in due {
        match super::fixed_lottery_fulfill::fulfill_one(db, wid).await {
            Ok(_) => done += 1,
            Err(e) => {
                tracing::warn!(winner_id = wid, error = %e,
                    "fixed_lottery fulfill failed");
            }
        }
    }
    // 领取期限扫挂在同一 60s tick：原函数没有任何调用点＝期限形同虚设。
    if let Err(e) =
        super::fixed_lottery_claim_expire::fixed_lottery_claim_expire(db).await
    {
        tracing::warn!(error = %e, "fixed_lottery claim expire failed");
    }
    Ok(done)
}

/// 开奖（镜像 services::fixed_lottery::draw 的 CAS+落子；worker 无 actix
/// 依赖，audit 走库直写）。
async fn api_draw(
    db: &PgPool,
    cid: i64,
    operator: Option<i64>,
) -> anyhow::Result<(bool, usize)> {
    let status: Option<String> = sqlx::query_scalar(
        "SELECT status FROM fixed_lottery_campaigns WHERE id = $1",
    )
    .bind(cid)
    .fetch_optional(db)
    .await?
    .flatten();
    match status.as_deref() {
        Some("drawn") => return Ok((true, 0)),
        Some("open") => {}
        _ => anyhow::bail!("campaign {cid} not drawable"),
    }
    let mut tx = db.begin().await?;
    // CAS 首步持锁；候选集随后在同一事务内冻结，报名口 FOR SHARE 互斥。
    let n = sqlx::query(
        "UPDATE fixed_lottery_campaigns \
         SET status = 'drawn', drawn_at = now(), drawn_by = $2 \
         WHERE id = $1 AND status = 'open'",
    )
    .bind(cid)
    .bind(operator)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        tx.rollback().await.ok();
        return Ok((true, 0));
    }
    let prizes: Vec<(i64, i32, String)> = sqlx::query_as(
        "SELECT id, quantity, name FROM fixed_lottery_prizes \
         WHERE campaign_id = $1 ORDER BY sort_order, id",
    )
    .bind(cid)
    .fetch_all(&mut *tx)
    .await?;
    if prizes.is_empty() {
        anyhow::bail!("campaign {cid} has no prizes");
    }
    let uids: Vec<i64> =
        super::fixed_lottery_repo::eligible_uids_tx(&mut tx, cid).await?;
    // seed 走 PG 侧 CSPRNG（pgcrypto，0165 已装）
    let seed: String =
        sqlx::query_scalar("SELECT encode(gen_random_bytes(32), 'hex')")
            .fetch_one(&mut *tx)
            .await?;
    let sel = selection(cid, &seed, &uids);
    let slot_total: usize = prizes.iter().map(|p| p.1.max(0) as usize).sum();
    let mut rows: Vec<(i64, i32, i64, i64, String, String)> = Vec::new();
    let mut pos = 0usize;
    'outer: for (pid, qty, pname) in &prizes {
        for slot_no in 1..=*qty {
            let Some((uid, key_hex)) = sel.get(pos) else {
                break 'outer;
            };
            pos += 1;
            let uname: String =
                sqlx::query_scalar("SELECT username FROM users WHERE id = $1")
                    .bind(uid)
                    .fetch_one(&mut *tx)
                    .await?;
            rows.push((*pid, slot_no, pos as i64, *uid, uname, pname.clone()));
            let _ = key_hex;
        }
    }
    let meta = serde_json::json!({
        "algorithm": "hmac-sha256-bottom-k-v1",
        "seed": seed,
        "seed_hash": seed_hash(&seed),
        "candidate_snapshot_hash": snapshot_hash(&uids),
        "candidate_count": uids.len(),
        "prize_slot_count": slot_total,
        "winner_count": rows.len(),
        "voided": uids.is_empty(),
        "trigger": if operator.is_some() { "manual" } else { "auto" },
    });
    for (pid, sno, p, uid, ref uname, ref pname) in &rows {
        let key = flx_key(&seed, cid, *uid);
        sqlx::query(
            "INSERT INTO fixed_lottery_winners \
             (campaign_id, prize_id, user_id, username_snapshot, \
              prize_name_snapshot, slot_no, draw_position, selection_key, \
              status, claim_deadline_at) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'pending', \
                 CASE WHEN EXISTS ( \
                      SELECT 1 FROM fixed_lottery_prizes p \
                      WHERE p.id = $2 AND p.reward_kind = 'custom') \
                 THEN now() + make_interval(days => ( \
                      SELECT c.claim_default_days \
                      FROM fixed_lottery_campaigns c WHERE c.id = $1)) \
                 ELSE NULL END) \
             ON CONFLICT (campaign_id, user_id) DO NOTHING",
        )
        .bind(cid)
        .bind(pid)
        .bind(uid)
        .bind(uname)
        .bind(pname)
        .bind(sno)
        .bind(p)
        .bind(key)
        .execute(&mut *tx)
        .await?;
        sqlx::query(
            "INSERT INTO fixed_lottery_fulfillments (winner_id, status) \
             SELECT id, 'pending' FROM fixed_lottery_winners \
             WHERE campaign_id = $1 AND user_id = $2 \
             ON CONFLICT (winner_id) DO NOTHING",
        )
        .bind(cid)
        .bind(uid)
        .execute(&mut *tx)
        .await?;
    }
    sqlx::query(
        "UPDATE fixed_lottery_campaigns SET draw_meta = $2 WHERE id = $1",
    )
    .bind(cid)
    .bind(&meta)
    .execute(&mut *tx)
    .await?;
    // 与 api 侧 audit_detail 同构：meta 并进 ref jsonb（audit_log 无 detail 列，
    // 列结构 = actor_id/action/ref/ip/prev_hash/self_hash，哈希链由触发器维护）
    let mut ref_v = meta.clone();
    if let Some(obj) = ref_v.as_object_mut() {
        obj.insert("id".into(), serde_json::json!(cid));
    }
    sqlx::query(
        "INSERT INTO audit_log (id, actor_id, action, ref) \
         VALUES (nextval('audit_log_id_seq'), $1, 'lottery.drawn', $2)",
    )
    .bind(operator)
    .bind(&ref_v)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((false, rows.len()))
}

fn seed_hash(seed_hex: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(seed_hex.as_bytes());
    h.finalize().iter().map(|x| format!("{x:02x}")).collect()
}

fn flx_key(seed_hex: &str, cid: i64, uid: i64) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type H = Hmac<Sha256>;
    let mut mac = H::new_from_slice(seed_hex.as_bytes()).expect("hmac key");
    mac.update(cid.to_string().as_bytes());
    mac.update(b":");
    mac.update(uid.to_string().as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

fn selection(cid: i64, seed: &str, uids: &[i64]) -> Vec<(i64, String)> {
    let mut v: Vec<(i64, String)> =
        uids.iter().map(|&u| (u, flx_key(seed, cid, u))).collect();
    v.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)));
    v
}

fn snapshot_hash(uids: &[i64]) -> String {
    use sha2::{Digest, Sha256};
    let mut s = uids.to_vec();
    s.sort_unstable();
    let mut h = Sha256::new();
    for u in s {
        h.update(u.to_string().as_bytes());
        h.update(b"\n");
    }
    h.finalize().iter().map(|x| format!("{x:02x}")).collect()
}
