//! 种子裁决用例（FRAME-PLAN-01 F2 示范抽离，0288「双入口纪律」的根治）。
//!
//! 单条裁决 `admin_http::review::review_decide` 与批量裁决
//! `admin_http::review_batch::review_batch` 原先各自维护同一套副作用，
//! 两处 SQL 靠人工同步（0288 纪律）迟早漂移——本文件是这套副作用的唯一
//! 居所：SQL 自两路由**原样搬移**（行为零变，diff 可审），路由层只做参数
//! 适配（单条口映射为错误响应、批量口映射为 skipped 条目）与循环调用。
//!
//! 副作用清单（FRAME §4.3「副作用清单化」：新增副作用必须改这份清单，
//! 防漏发）：
//!   1. torrents 状态翻转（1=过 / 2=拒；过审落 approved_at，拒落拒因/备注）；
//!   2. users 连击/被拒计数 + 发布信用分（0075/0077/0355，P2-1 过审清零）；
//!   3. 过审副作用：自动促销 + 组/厂牌/字幕组订阅推送
//!      （admin_http::review_side_effects，0288 已共用的那份）；
//!   4. torrent_operation_logs 留痕（失败 WARN 不阻断裁决）；
//!   5. 审核结果通知：站内信 + 尽力邮件（admin_http::review_notify）。
//! 不在本函数（口径不同，留在路由层）：审计 action 命名（单条
//! `review.approve`·逐条 / 批量 `review.approve.batch`·汇总一次）、
//! tracker 白名单与列表缓存代际推进（单条逐次 / 批量命中才汇总一次）。

use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

/// 单条裁决结果：路由层据此映射为错误（单条口）或 skipped 条目（批量口）。
pub(crate) enum DecideOutcome {
    /// 种子不存在（owner 查无）
    NotFound,
    /// 操作者本人发布的种子（利益冲突，0285 实测：class 91 发布员可审并
    /// 放行自己的种）
    SelfConflict,
    /// 不在待审状态（approval_status <> 0，含被并发裁决抢先处理）
    NotPending,
    /// 裁决已落库且上面清单里的副作用全部执行完毕
    Done,
}

/// 裁决一个种子的全部副作用（状态更新/计数/信用分/副作用调度/留痕/通知）。
///
/// `reason` 传原始串，函数内部按原口径取用：deny_note 存 trim 后的值
/// （两入口原本就都存 trim 后），留痕 detail 与通知回退文案按传入原样
/// （单条口传 body.reason、批量口传已 trim 的 reason——与抽离前逐字节
/// 一致）。`batch` 只影响留痕 detail 是否带 `"batch": true` 标记。
#[allow(clippy::too_many_arguments)]
pub(crate) async fn decide_one(
    state: &AppState,
    operator_id: i64,
    torrent_id: i64,
    approve: bool,
    reason: &str,
    deny_reason_id: Option<i64>,
    batch: bool,
    by_auto: bool,
) -> DomainResult<DecideOutcome> {
    let db = &state.repo.db;
    // 利益冲突预检（0285）：两入口原本就都先查 owner——查无即「种子不存在」
    let owner_id: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(torrent_id)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner_id else {
        return Ok(DecideOutcome::NotFound);
    };
    if owner == operator_id {
        return Ok(DecideOutcome::SelfConflict);
    }
    // 1=已过 2=被拒。过审写 approved_at（审计 2026-10-08 P1-1：该字段此前
    // 在审核链路从不落值，worker 的泄露检测与社交队小时窗都以
    // `approved_at IS NOT NULL` 为前提 ⇒ 全部常规过审种恒空转）。
    // 状态机判定收 domain（FRAME §7 第 6 条）：0→1/2 合法性与下面 WHERE
    // 谓词同源，先在此显式判定（非法组合此处不可能出现——from 恒为 0，
    // 断言防未来有人改 from 时绕过矩阵）。
    let to = if approve {
        crate::domain::torrent_status::APPROVED
    } else {
        crate::domain::torrent_status::DENIED
    };
    debug_assert!(crate::domain::torrent_status::can_transition(
        crate::domain::torrent_status::PENDING,
        to,
    ));
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = $2, \
            approved_at = CASE WHEN $2::smallint = 1 THEN now() ELSE NULL END, \
            deny_reason_id = $3, deny_note = $4 \
         WHERE id = $1 AND approval_status = 0",
    )
    .bind(torrent_id)
    .bind(to)
    .bind(if approve { None } else { deny_reason_id })
    .bind(if approve {
        None
    } else {
        Some(reason.trim().to_string())
    })
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .rows_affected();
    if n == 0 {
        return Ok(DecideOutcome::NotPending);
    }
    // 详情对象缓存：审核态翻转必须立即失效——过审种此前不进共享缓存
    // （aggregate.rs 只缓存 approval_status=1），但被拒那一刻是「从 1 的
    // 世界里消失」，列表/tracker 白名单都在刷新，详情缓存不跟着清就会
    // 出现「列表已不见、直达详情仍全量可读」最长 30 秒的劈叉。
    crate::torrent_http::invalidate_tdetail_cache(state, torrent_id).await;
    // 0075 连击 +1/被拒清零；0077 deny_count 累计（P2-1 过审清零）；
    // 0355 信用分独立累积：+1（上限 5）/ -2（下限 0）。
    let _ = sqlx::query(
        "UPDATE users u SET approve_streak = CASE WHEN $2 THEN \
         u.approve_streak + 1 ELSE 0 END, \
         deny_count = CASE WHEN $2 THEN 0 ELSE u.deny_count + 1 END, \
         publish_credits = GREATEST(0, LEAST(5, u.publish_credits \
         + CASE WHEN $2 THEN 1 ELSE -2 END)) FROM torrents t \
         WHERE t.id = $1 AND u.id = t.owner_id",
    )
    .bind(torrent_id)
    .bind(approve)
    .execute(db)
    .await;
    // 过审副作用（自动促销 + 组/厂牌/字幕组订阅推送）：单条与批量共用
    if approve {
        crate::admin_http::review_side_effects::apply_approval_side_effects(
            db,
            torrent_id,
            operator_id,
        )
        .await;
    }
    // 种子操作记录：留痕失败不阻断裁决，但必须报出来（与 postpone/resume
    // 同形；单条口原口径 WARN，批量口原先静默——统一为 WARN，只多日志）
    let action = if approve { "approve" } else { "reject" };
    let detail = if batch {
        serde_json::json!({
            "reason": reason,
            "deny_reason_id": deny_reason_id,
            "batch": true,
        })
    } else {
        serde_json::json!({
            "reason": reason,
            "deny_reason_id": deny_reason_id,
        })
    };
    if let Err(e) = sqlx::query(
        "INSERT INTO torrent_operation_logs \
         (torrent_id, operator_id, action, detail) \
         VALUES ($1, $2, $3, $4::jsonb)",
    )
    .bind(torrent_id)
    .bind(operator_id)
    .bind(action)
    .bind(detail.to_string())
    .execute(db)
    .await
    {
        tracing::warn!(%e, torrent = torrent_id, "审核留痕写入失败");
    }
    // U2 §12.5 审核结果通知：取数与发送在 review_notify.rs（0285 拆出并修列名）。
    // by_auto 贯通到通知层：自动裁决才用「自动」模板主题/限频/outbox，
    // 人工裁决即便选了 91-96 段拒因也如实按人工口径通知（2026-10-21 实测修）。
    crate::admin_http::review_notify::notify_review_result(
        state, torrent_id, approve, reason, by_auto,
    )
    .await;
    Ok(DecideOutcome::Done)
}
