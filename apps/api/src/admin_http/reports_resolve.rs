//! 举报处置（批三十八从 reports.rs 拆出，300 行门禁）。
//!
//! 拆的是 resolve 这一个 handler：它同时管状态机、给举报人回信、字幕举报
//! 的奖惩动账三件事，注释越写越厚就把父文件顶到门禁外——按既有口径
//! 「拆文件」而不是放宽基线。claim/release/queue 仍留在 reports.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::guard::staff;

/// 处置举报：action = act(已处置) | dismiss(驳回)；结果自动 PM 通知举报人
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResolveReq {
    report_id: i64,
    #[serde(default)]
    action: Option<String>, // act / dismiss，缺省 act
    #[serde(default)]
    note: String,
}

#[post("/admin/reports/resolve")]
pub async fn report_resolve(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<ResolveReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    // P-243：结案/驳回会改他人工单状态并发私信
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::REPORTS_HANDLE,
    )
    .await?;
    let action = body.action.as_deref().unwrap_or("act");
    if !["act", "dismiss"].contains(&action) {
        return Err(DomainError::Validation("action 需为 act/dismiss".into()));
    }
    // 审计修复（P-133）：备注直接拼进发给举报人的私信正文，此前**没有上限**——
    // 实测 5000 字整段塞进去（一条私信撑满收件箱，且夹带链接/长文可当传播面）。
    // 按字判（同 P-128 的口径），并且必须在改状态之前拒，否则「先结案再报错」
    // 会把工单卡成已处理却没人知道结论。
    let note = body.note.trim();
    if note.chars().count() > 500 {
        return Err(DomainError::Validation(
            "备注最长 500 字（会原样发给举报人）".into(),
        ));
    }
    // 取举报人与对象（供 PM）
    let info: Option<(i64, String, i64, Option<i64>)> = sqlx::query_as(
        "SELECT reporter_id, ref_type, \
         ref_id, claimed_by FROM reports WHERE id = $1 AND status IN (0, 2)",
    )
    .bind(body.report_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((reporter_id, ref_type, ref_id, claimed_by)) = info else {
        return Err(DomainError::Validation("举报不存在或已处理".into()));
    };
    // P-475：作弊案件的结案口是 POST /admin/cheat-cases/{id}/resolve——那边判
    // CHEAT_SANCTIONS、要处置说明、校验 sanction 档位、比高低（ensure_outranks），
    // 并顺带解除证据停发。本口只判 REPORTS_HANDLE，从这边点「处理」会把案件状态
    // 写成 1 而跳过以上全部。改状态之前点名拒。
    if ref_type == "cheat" {
        return Err(DomainError::Validation(
            super::reports::CHEAT_GUARD.into(),
        ));
    }
    // 认领互斥（0285）：resolve 此前无视 claimed_by，任何人可关掉别人正在办的工单。
    if claimed_by.is_some() && claimed_by != Some(auth.id) {
        return Err(DomainError::Validation(
            "该举报已被其他管理员认领，请先由认领人释放".into(),
        ));
    }
    sqlx::query(
        "UPDATE reports SET status = 1, handled_by = $1, handled_at = now() \
         WHERE id = $2 AND status IN (0, 2)",
    )
    .bind(auth.id)
    .bind(body.report_id)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // PM 通知举报人
    let subject = if action == "act" {
        "您的举报已处置"
    } else {
        "您的举报已审阅（未处置）"
    };
    let mut pm = format!(
        "您举报的对象（{} #{ref_id}）已被管理组审阅。\n\n结果：{}",
        ref_type,
        if action == "act" {
            "已按规则处置"
        } else {
            "经核实未违反规则，予以驳回"
        }
    );
    if !note.is_empty() {
        pm.push_str("\n\n管理备注：");
        pm.push_str(note);
    }
    // 审计修复（P-135）：旧写法是 `let _ = sqlx::query(INSERT INTO messages…)`
    // ——回信写失败被整个吞掉：工单已标「已处理」、版主看到 200，举报人却永远
    // 收不到结论，事后也无从发现。结案本身是事实，不为一条私信反转，
    // 但必须**留痕 + 如实回报**（面板/闸门看 `pm_sent`）。
    let pm_sent = match sqlx::query(
        "INSERT INTO messages (sender_id, receiver_id, subject, body, kind) \
         VALUES ($1, $2, $3, $4, 'report_resolved')",
    )
    .bind(auth.id)
    .bind(reporter_id)
    .bind(subject)
    .bind(&pm)
    .execute(&state.repo.db)
    .await
    {
        Ok(r) => r.rows_affected() == 1,
        Err(e) => {
            tracing::warn!(
                %e,
                report = body.report_id,
                "举报结论私信写入失败（工单已结案，举报人收不到）"
            );
            false
        }
    };
    // 字幕举报奖惩闭环（0146 P1-8，NP 口径：成立 → 举报者 +50、上传者 −100、
    // 删字幕；dismiss 不动账）。幂等键锚定举报单：重复处理不会二次动账。
    if action == "act" && ref_type == "subtitle" {
        let uploader: Option<i64> = sqlx::query_scalar(
            "SELECT user_id FROM subtitles WHERE id = $1 AND deleted_at \
             IS NULL",
        )
        .bind(ref_id)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let n = sqlx::query(
            "UPDATE subtitles SET deleted_at = now(), moderated_by = $1, \
             moderated_at = now() WHERE id = $2 AND deleted_at IS NULL",
        )
        .bind(auth.id)
        .bind(ref_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
        if n > 0 {
            let rid = body.report_id;
            let reward_idem = format!("subtitle-report-reward:{rid}");
            crate::economy_http::earn_spark(
                &state.repo.db,
                reporter_id,
                50,
                "subtitle_report",
                &reward_idem,
            )
            .await?;
            if let Some(uploader) = uploader {
                if uploader != reporter_id {
                    let fine_idem =
                        format!("subtitle-report-fine:{}", body.report_id);
                    // P-415b：「罚金失败不阻断结案」是口径，但不阻断 ≠ 不留痕。
                    // 旧写法 `let _ =` 把 DB 故障、幂等冲突一起吞掉，站方事后根本
                    // 看不出这 100 点扣没扣。余额不足是预期内的（扣不满就算了），
                    // 其余一律出声（P-323 / P-328 同口径）。
                    match crate::economy_http::spend_spark(
                        &state.repo.db,
                        uploader,
                        100,
                        "subtitle_violation",
                        &fine_idem,
                        "subtitle",
                        ref_id,
                    )
                    .await
                    {
                        Ok(_) => {}
                        Err(DomainError::InsufficientSpark) => {}
                        Err(e) => tracing::warn!(
                            err = %e,
                            report = rid,
                            user = uploader,
                            "字幕举报罚金扣款失败（结案继续，罚金未落账）"
                        ),
                    }
                }
            }
        }
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "report.resolve",
            Some(body.report_id),
            Some(body.note.trim()),
            Some(serde_json::json!({"action": action})),
        )
        .await;
    Ok(ok(
        serde_json::json!({ "resolved": body.report_id, "action": action,
                            "pm_sent": pm_sent }),
    ))
}
