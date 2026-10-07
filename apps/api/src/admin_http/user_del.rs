use actix_web::{delete, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

/// 单事务级联删除一个（已封禁）用户：覆盖全部 NO ACTION 引用列（83 处实测）。
/// 由 `DELETE /admin/users/{id}` 与批量 `deletedisabled`（http.rs）共用同一权威实现。
/// 任一步失败整体回滚；返回 anyhow::Result 便于批量调用方收集逐户失败原因。
pub async fn delete_user_cascade(
    db: &sqlx::PgPool,
    uid: i64,
) -> anyhow::Result<()> {
    let mut tx = db.begin().await?;
    // ── 软删化（账本纪律与风控追溯，0097）─────────────────────────────
    // 旧版物理 DELETE spark_ledger / login_events / hr_* / leak_events / snatches：
    // 违背「一切动账经 ledger 流水」的自家纪律（全局对账视图失真），且作弊者删号
    // 重注册后无历史证据可查。改为：
    //   1) users 行保留并匿名化（status=3 软删态；用户名/邮箱随机改写释放唯一索引；
    //      passkey 作废防 announce 冒用；pass_hash 置不可登录值）+ 撤销全部 JWT。
    //   2) 账本/风控证据表不再删除：spark_ledger、login_events、hr_snapshots、
    //      hr_violations、leak_events、snatches、traffic_ledger（cheat_events 本无 FK）。
    //   3) 社交/娱乐数据维持物理清理（无人引用、无需留痕）。
    let softened: usize = sqlx::query(
        r#"
        UPDATE users SET
            username = 'deleted-' || id::text || '-' || substr(md5(random()::text), 1, 8),
            email = 'deleted-' || id::text || '-' || substr(md5(random()::text), 1, 8) || '@deleted.invalid',
            pass_hash = '!', passkey = 'deleted-' || id::text,
            title = NULL, avatar_url = NULL, status = 3
        WHERE id = $1 AND status >= 2
        "#,
    )
    .bind(uid)
    .execute(&mut *tx)
    .await
    .map_err(|e| anyhow::anyhow!(format!("uid {uid}: {e}")))?
    .rows_affected()
    .try_into()
    .unwrap_or(0);
    if softened == 0 {
        // 竞态：账号已被恢复/删除——按无行处理，幂等返回
        tx.commit().await?;
        return Ok(());
    }
    // ── 动账纪律（0266）：软删前清余额并补 user_purge 负流水 ──────────
    // 否则 spark_balance 残留在快照侧而无对应流水，reconcile_diff_alert
    // 每 6h 对账告警永久误报（流水外余额变动源）。
    sqlx::query(
        r#"
        INSERT INTO spark_ledger
            (id, user_id, amount, kind, ref_type,
             idempotency_key, balance_after)
        SELECT nextval('spark_ledger_id_seq'), $1, -spark_balance,
               'user_purge', 'user', 'user-purge-' || $1, 0
        FROM users WHERE id = $1 AND spark_balance <> 0
        ON CONFLICT DO NOTHING
        "#,
    )
    .bind(uid)
    .execute(&mut *tx)
    .await
    .map_err(|e| anyhow::anyhow!(format!("uid {uid}: purge ledger: {e}")))?;
    sqlx::query("UPDATE users SET spark_balance = 0 WHERE id = $1")
        .bind(uid)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            anyhow::anyhow!(format!("uid {uid}: zero balance: {e}"))
        })?;
    sqlx::query(
                "INSERT INTO token_revocations (user_id, nbf) VALUES ($1, \
         EXTRACT(EPOCH FROM now())::bigint) ON CONFLICT (user_id) DO UPDATE SET \
         nbf = GREATEST(token_revocations.nbf, EXCLUDED.nbf)",
    )
    .bind(uid)
    .execute(&mut *tx)
    .await
    .map_err(|e| anyhow::anyhow!(format!("uid {uid}: {e}")))?;
    for sql in [
        // —— 本人拥有的业务数据（删；账本/风控证据除外，见函数头） ——
        "DELETE FROM attendance WHERE user_id = $1",
        "DELETE FROM task_claims WHERE user_id = $1",
        "DELETE FROM bank_demand_accounts WHERE user_id = $1",
        "DELETE FROM bank_deposits WHERE user_id = $1",
        "DELETE FROM bank_loans WHERE user_id = $1",
        "DELETE FROM bank_interest_records WHERE user_id = $1",
        "DELETE FROM messages WHERE sender_id = $1 OR receiver_id = $1",
        "DELETE FROM pmboxes WHERE user_id = $1",
        "DELETE FROM message_flood WHERE user_id = $1",
        "DELETE FROM staffmessages WHERE user_id = $1",
        "DELETE FROM comments WHERE user_id = $1",
        "DELETE FROM posts WHERE user_id = $1",
        "DELETE FROM topics WHERE user_id = $1",
        "DELETE FROM user_medals WHERE user_id = $1",
        "DELETE FROM user_roles WHERE user_id = $1",
        "DELETE FROM user_permissions WHERE user_id = $1",
        "DELETE FROM user_dressups WHERE user_id = $1",
        "DELETE FROM user_vouchers WHERE user_id = $1",
        "DELETE FROM bookmarks WHERE user_id = $1",
        "DELETE FROM api_tokens WHERE user_id = $1",
        "DELETE FROM password_resets WHERE user_id = $1",
        "DELETE FROM invites WHERE inviter_id = $1 OR used_by = $1",
        "DELETE FROM invite_quota WHERE user_id = $1",
        "DELETE FROM friendships WHERE user_id = $1 OR friend_id = $1",
        "DELETE FROM thanks WHERE user_id = $1",
        "DELETE FROM farm_plots WHERE user_id = $1",
        "DELETE FROM farm_harvests WHERE user_id = $1",
        "DELETE FROM fun_items WHERE user_id = $1",
        "DELETE FROM fun_item_votes WHERE user_id = $1",
        "DELETE FROM fun_votes WHERE user_id = $1",
        "DELETE FROM gomoku_games WHERE black_id = $1 OR white_id = $1",
        "DELETE FROM contest_entries WHERE user_id = $1",
        "DELETE FROM offers WHERE user_id = $1",
        "DELETE FROM offer_votes WHERE user_id = $1",
        "DELETE FROM requests WHERE user_id = $1",
        "DELETE FROM resub_uses WHERE user_id = $1",
        "DELETE FROM resurrections WHERE user_id = $1",
        "DELETE FROM subtitles WHERE user_id = $1",
        "DELETE FROM seed_milestones WHERE user_id = $1",
        "DELETE FROM jixiao_claims WHERE user_id = $1",
        "DELETE FROM appeals WHERE user_id = $1",
        "DELETE FROM download_keys WHERE user_id = $1",
        "DELETE FROM shop_orders WHERE user_id = $1",
        "DELETE FROM pool_donations WHERE user_id = $1",
        "DELETE FROM funding_contribs WHERE user_id = $1",
        "DELETE FROM push_subscriptions WHERE user_id = $1",
        "DELETE FROM reports WHERE reporter_id = $1",
        // —— 他方操作留痕列（置空，保留记录本身） ——
        "UPDATE torrents SET owner_id = NULL WHERE owner_id = $1",
        // 注：audit_log.actor_id **不能置空**。0266 的审计哈希链触发器拒绝任何
        // audit_log UPDATE（`NEW.self_hash <> OLD.self_hash` 即 RAISE），
        // 一句置空会让整条删号事务失败 ⇒ 「凡是产生过审计行的账号都删不掉」
        // （实测 DELETE /admin/users/{id} 回 400「audit_log 中不可篡改，留痕保留」）。
        // 这里刻意不动 audit_log：actor_id 继续指向该账号，而账号已被墓碑化成
        // `deleted-<id>-<hash>`，归属信息不丢、链也不断。
        "UPDATE announcements SET author_id = NULL WHERE author_id = $1",
        "UPDATE appeals SET handled_by = NULL WHERE handled_by = $1",
        "UPDATE posts SET edited_by = NULL WHERE edited_by = $1",
        "UPDATE fundings SET creator_id = NULL WHERE creator_id = $1",
        "UPDATE hr_snapshots SET pardoned_by = NULL WHERE pardoned_by = $1",
        "UPDATE hr_violations SET resolved_by = NULL WHERE resolved_by = $1",
        "UPDATE leak_events SET resolved_by = NULL WHERE resolved_by = $1",
        "UPDATE reports SET claimed_by = NULL, \
         handled_by = NULL WHERE claimed_by = $1 OR handled_by = $1",
        "UPDATE staffmessages SET answered_by = NULL, \
         assigned_to = NULL WHERE answered_by = $1 OR assigned_to = $1",
        "UPDATE seed_preserve SET claimed_by = NULL WHERE claimed_by = $1",
        "UPDATE agent_rules SET created_by = NULL WHERE created_by = $1",
        "UPDATE email_bans SET created_by = NULL WHERE created_by = $1",
        "UPDATE forum_mods SET created_by = NULL WHERE created_by = $1",
        "UPDATE friend_links SET applied_by = NULL WHERE applied_by = $1",
        "UPDATE fun_polls SET created_by = NULL WHERE created_by = $1",
        "UPDATE ip_bans SET banned_by = NULL WHERE banned_by = $1",
        "UPDATE mass_mails SET sent_by = NULL WHERE sent_by = $1",
        "UPDATE promotions SET created_by = NULL WHERE created_by = $1",
        "UPDATE rules_revisions SET edited_by = NULL WHERE edited_by = $1",
        "UPDATE sticky_promotions SET created_by = NULL WHERE created_by = $1",
        "UPDATE torrent_groups SET created_by = NULL WHERE created_by = $1",
        "UPDATE torrent_operation_logs SET operator_id = NULL WHERE \
         operator_id = $1",
        "UPDATE user_modify_logs SET modifier = NULL WHERE modifier = $1",
        "UPDATE username_change_logs SET operator = NULL WHERE operator = $1",
        "UPDATE users SET invited_by = NULL WHERE invited_by = $1",
    ] {
        sqlx::query(sql)
            .bind(uid)
            .execute(&mut *tx)
            .await
            .map_err(|e| anyhow::anyhow!(format!("uid {uid}: {e}")))?;
    }
    tx.commit().await?;
    Ok(())
}

/// 详情页删除用户（参考站用户详情「删除」口径）：仅 sysop，且要求先封禁（防误删活跃账号）
#[delete("/admin/users/{id}")]
async fn user_admin_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::USER_DELETE_DISABLED,
    )
    .await?;
    let uid = path.into_inner();
    if uid == auth.id {
        return Err(DomainError::Validation("不能删除自己".into()));
    }
    // 三轮审计 P1-1（2026-10-07）：补 ensure_outranks——其余全部用户写端点
    // 都有严格大于护栏，唯独删除缺席：被封 99 的账号可被另一个 99 墓碑化
    // （user_roles/user_permissions/messages 连带销毁，申诉凭据湮灭）。
    super::guard::ensure_outranks(&state.repo.db, auth.class_id, uid).await?;
    let status: Option<i16> =
        sqlx::query_scalar("SELECT status FROM users WHERE id = $1")
            .bind(uid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    match status {
        None => return Err(DomainError::NotFound(uid)),
        Some(s) if s < 2 => {
            return Err(DomainError::Validation(
                "仅封禁状态的账号可删除，请先封禁".into(),
            ));
        }
        _ => {}
    }
    crate::admin_http::delete_user_cascade(&state.repo.db, uid)
        .await
        .map_err(|e| DomainError::Validation(e.to_string()))?;
    state
        .repo
        .audit(Some(auth.id), "user.delete", Some(uid))
        .await;
    Ok(ok(serde_json::json!({ "deleted": uid })))
}
