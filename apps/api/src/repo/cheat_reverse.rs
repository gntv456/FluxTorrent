//! 申诉推翻后的反向解除（0369 M3）的 SQL 唯一居所（FRAME §2 下沉）：
//! 自 gaps_http/cheat_appeal_reverse.rs 整体并入（该文件是事务编排体，
//! SQL 与编排不可分割）。与 sanction.rs 的下达动作一一对应、方向相反。
//!
//! 反向项（按该案 `sanction` JSON 的 kind 派发）：
//!   ban/full_ban → 清 cheat_banned_until（full_ban 另恢复 status 2→0）
//!   warn         → 清 warned_until/warned_reason
//!   reclaim_up>0 → traffic_ledger 正向流水 + users.uploaded 补回
//! 另无论何种 kind 都：案件 sanction.state=reversed、本案证据标
//! false_positive（规则准确率统计口径）、案件备注留痕。

use sqlx::{Postgres, Transaction};

use crate::errors::{DomainError, DomainResult};

/// 下达侧的回收（P-403）：在调用方事务内扣上传量，返回**实际扣掉的字节数**。
///
/// 流水记实际值而非请求值：快照列是 `GREATEST(uploaded - 请求值, 0)`，夹到 0
/// 之后实际只扣掉 `uploaded` 那么多。旧写法把请求值写进 traffic_ledger，于是
/// `sum(ledger)` 与 `users.uploaded` 永久撕裂；而下面的反向解除按同一枚请求值
/// 回补 ⇒ 虚报数大于现有上传量的案件被推翻后，会员白得差额（分享率、等级、
/// 成就全读这一列）。同仓定例见 `admin_http/user_ops.rs` 的 P0 错账注释。
pub async fn apply_reclaim_tx(
    tx: &mut Transaction<'_, Postgres>,
    uid: i64,
    case_id: i64,
    operator_id: i64,
    requested: i64,
) -> DomainResult<i64> {
    if requested <= 0 {
        return Ok(0);
    }
    let up_now: i64 = sqlx::query_scalar(
        "SELECT uploaded FROM users WHERE id = $1 FOR UPDATE",
    )
    .bind(uid)
    .fetch_one(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let applied = requested.min(up_now.max(0));
    if applied <= 0 {
        return Ok(0);
    }
    let sql_ledger = "INSERT INTO traffic_ledger (id, user_id, torrent_id, \
        delta_up, delta_down, window_start, reason, operator_id) \
        VALUES (nextval('traffic_ledger_id_seq'), $1, 0, $2, 0, \
        now(), $3, $4)";
    sqlx::query(sql_ledger)
        .bind(uid)
        .bind(-applied)
        .bind(format!("cheat reclaim case #{case_id}"))
        .bind(operator_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    sqlx::query(
        "UPDATE users SET uploaded = GREATEST(uploaded - $1, 0) WHERE id = $2",
    )
    .bind(applied)
    .bind(uid)
    .execute(&mut **tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(applied)
}

/// 执行反向解除（调用方持有事务与 commit）；返回已执行项的说明
/// （供通知正文与响应体）。
pub async fn reverse_sanction(
    tx: &mut Transaction<'_, Postgres>,
    uid: i64,
    sanction: Option<&serde_json::Value>,
    case_id: Option<i64>,
    appeal_id: i64,
    actor_id: i64,
    note: &str,
) -> DomainResult<Vec<String>> {
    let kind = sanction
        .and_then(|s| s.get("kind"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    // P-403：只回补**实际扣掉的那个数**（`reclaim_applied`）。旧写法读请求值
    // `reclaim_up`，而扣减侧被 `GREATEST(...,0)` 夹过 ⇒ 虚报多于现有上传量的
    // 案件被推翻时，会员白得差额。缺 `reclaim_applied` 的老案件仍按请求值走
    // （那些案的扣减侧本来就是请求值，账目自洽，不能凭空改口径）。
    let r_up = sanction
        .and_then(|s| s.get("reclaim_applied"))
        .and_then(|v| v.as_i64())
        .or_else(|| {
            sanction
                .and_then(|s| s.get("reclaim_up"))
                .and_then(|v| v.as_i64())
        })
        .unwrap_or(0)
        .max(0);
    let mut reversed: Vec<String> = Vec::new();
    // ① 停权 / 全站封禁
    if matches!(kind, "ban" | "full_ban") {
        sqlx::query("UPDATE users SET cheat_banned_until = NULL WHERE id = $1")
            .bind(uid)
            .execute(&mut **tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        reversed.push("停权已解除".into());
        if kind == "full_ban" {
            let n = sqlx::query(
                "UPDATE users SET status = 0 WHERE id = $1 AND status = 2",
            )
            .bind(uid)
            .execute(&mut **tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
            if n > 0 {
                reversed.push("全站封禁已解除".into());
            }
        }
    }
    // ② 警告
    if kind == "warn" {
        sqlx::query(
            "UPDATE users SET warned_until = NULL, warned_reason = NULL \
             WHERE id = $1",
        )
        .bind(uid)
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        reversed.push("警告已清除".into());
    }
    // ③ 回收流量回补
    if r_up > 0 {
        sqlx::query(
            "INSERT INTO traffic_ledger \
               (id, user_id, torrent_id, delta_up, delta_down, \
                window_start, reason, operator_id) \
             VALUES (nextval('traffic_ledger_id_seq'), $1, 0, $2, 0, \
                     now(), $3, $4)",
        )
        .bind(uid)
        .bind(r_up)
        .bind(format!("cheat appeal overturn #{appeal_id}"))
        .bind(actor_id)
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        sqlx::query("UPDATE users SET uploaded = uploaded + $1 WHERE id = $2")
            .bind(r_up)
            .bind(uid)
            .execute(&mut **tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        reversed.push(format!("回补上传 {r_up} 字节"));
    }
    // ④ 案件标 reversed（读改写，避免 jsonb_set 父路径陷阱）
    if let Some(cid) = case_id {
        if let Some(sj) = sanction {
            let mut sj = sj.clone();
            sj["state"] = serde_json::json!("reversed");
            sj["reversed_by"] = serde_json::json!(actor_id);
            sj["reversed_at"] = serde_json::json!(chrono::Utc::now());
            sj["reversed_appeal"] = serde_json::json!(appeal_id);
            sqlx::query("UPDATE reports SET sanction = $1 WHERE id = $2")
                .bind(&sj)
                .bind(cid)
                .execute(&mut **tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        }
        // ⑤ 本案证据标误判
        sqlx::query(
            "UPDATE cheat_events SET false_positive = TRUE WHERE case_id = $1",
        )
        .bind(cid)
        .execute(&mut **tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let body = format!(
            "申诉 #{appeal_id} 复核结论：推翻（overturned）。{}。说明：{note}",
            if reversed.is_empty() {
                "无自动反向项".to_string()
            } else {
                reversed.join("；")
            }
        );
        super::cheat_appeals::insert_case_note(tx, cid, actor_id, body)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    Ok(reversed)
}
