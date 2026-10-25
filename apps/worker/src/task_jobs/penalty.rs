//! 考核不达标处置菜单（0397）：解析 fail_penalty JSONB 并逐项执行。
//!
//! 0381 的 `fail_action` 三态（none/restrict/ban）只能单选、且仅多阶段
//! 路径可配。本模块把处置升级为**多选菜单**，并让单段考核（0093 的
//! 新人转正考核就是单段——此前到期只有罚金可扣）也有处置手段。
//! 单项执行在 penalty_ops.rs（300 行门禁拆分）。
//!
//! 【幂等】每项处置有 `(claim_id, kind)` 唯一约束兜底 + 落账表
//! `exam_penalties` 先查后插（worker 重跑/并发双跑都不双罚）。
//! 【审计】每项落 audit_log（`exam.penalty`）+ exam_penalties 明细行，
//! 站长可查「谁因哪个考核被处置了什么、实扣多少」。
//! 【通知】处置内容拼进 settle_fail 的站内信——用户必须知道发生了什么、
//! 找谁申诉，否则处置就是黑箱。
//!
//! ⚠️ tracker 侧 passkey 缓存 60s，封禁/禁下载最坏延迟 1 分钟生效
//! ——与人工处置相同窗口。

use sqlx::PgPool;

use super::penalty_ops::apply_one;
use super::settle::OpenClaim;

/// 一项惩罚配置（fail_penalty 数组元素）。
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub(crate) struct PenaltyItem {
    /// 惩罚种类（菜单键，见 KIND_* 常量）
    pub kind: String,
    /// spark_fine：扣魔力数额 / traffic_fine：扣字节数
    #[serde(default)]
    pub amount: Option<i64>,
    /// traffic_fine：扣哪个量（uploaded/downloaded，缺省 uploaded）
    #[serde(default)]
    pub target: Option<String>,
    /// ban_pages：禁浏览的页面权限位列表（如 ["torrent.upload"]）
    #[serde(default)]
    pub pages: Option<Vec<String>>,
}

pub(crate) const KIND_BAN: &str = "ban";
pub(crate) const KIND_SUSPEND: &str = "suspend";
pub(crate) const KIND_RESTRICT_DOWNLOAD: &str = "restrict_download";
pub(crate) const KIND_MUTE: &str = "mute";
pub(crate) const KIND_BAN_UPLOAD: &str = "ban_upload";
pub(crate) const KIND_BAN_PAGES: &str = "ban_pages";
pub(crate) const KIND_SPARK_FINE: &str = "spark_fine";
pub(crate) const KIND_TRAFFIC_FINE: &str = "traffic_fine";

/// 合法 kind 白名单：菜单外的键是配置错误，静默跳过等于「配了没生效」
/// 的假绿——校验失败要在日志里喊出来。
pub(crate) const KINDS: &[&str] = &[
    KIND_BAN,
    KIND_SUSPEND,
    KIND_RESTRICT_DOWNLOAD,
    KIND_MUTE,
    KIND_BAN_UPLOAD,
    KIND_BAN_PAGES,
    KIND_SPARK_FINE,
    KIND_TRAFFIC_FINE,
];

/// 解析 fail_penalty JSONB。空数组/解析失败返回空 vec（回退旧路径）。
pub(crate) fn parse_penalty(v: serde_json::Value) -> Vec<PenaltyItem> {
    let Ok(items) = serde_json::from_value::<Vec<PenaltyItem>>(v) else {
        return vec![];
    };
    items
        .into_iter()
        .filter(|i| KINDS.contains(&i.kind.as_str()))
        .collect()
}

/// 应用一批处置（多阶段与单段结算共用入口）。
///
/// `stage_seq` 多阶段传实际 seq，单段传 1。
///
/// 返回**站内信通知片段**（每项处置一句）：处置是黑箱就没有意义——
/// 用户必须知道被处置了什么、实扣多少、找谁申诉。
///
/// 【原子性】处置与 exam_penalties 落账在同一事务里：worker 崩在
/// 「已扣、还没记账」的窗口上，重跑会二次扣款（traffic_fine 尤其致命）。
/// 同一个事务 ⇒ 要么全成、要么全不成。
pub(super) async fn apply_penalties(
    db: &PgPool,
    c: &OpenClaim,
    stage_seq: i32,
    items: &[PenaltyItem],
) -> anyhow::Result<Vec<String>> {
    let mut notes: Vec<String> = Vec::new();
    if items.is_empty() {
        return Ok(notes);
    }
    let mut tx = db.begin().await?;
    for item in items {
        // 幂等闸：同认领同 kind 已处置过则跳过（表上还有 UNIQUE 兜底）
        let done: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM exam_penalties \
             WHERE claim_id = $1 AND kind = $2)",
        )
        .bind(c.id)
        .bind(&item.kind)
        .fetch_one(&mut *tx)
        .await?;
        if done {
            continue;
        }
        let applied = apply_one(&mut tx, c, item).await?;
        sqlx::query(
            "INSERT INTO exam_penalties \
             (user_id, claim_id, task_id, stage_seq, kind, params, applied) \
             VALUES ($1, $2, $3, $4, $5, $6, $7) \
             ON CONFLICT (claim_id, kind) DO NOTHING",
        )
        .bind(c.user_id)
        .bind(c.id)
        .bind(c.task_id)
        .bind(stage_seq)
        .bind(&item.kind)
        .bind(serde_json::to_value(item).unwrap_or_default())
        .bind(&applied)
        .execute(&mut *tx)
        .await?;
        // 审计行：用 serde_json 构造（任务名是站长可填的自由文本，
        // 手拼 format! 出来的 JSON 遇到引号/反斜杠就非法 → jsonb
        // 转换报错 → 整个处置事务回滚 → 结算批次反复重试）。
        let ref_json = serde_json::json!({
            "user": c.user_id,
            "claim": c.id,
            "kind": item.kind,
            "task": c.task_name,
            "stage": stage_seq,
            "applied": applied,
        });
        sqlx::query(
            "INSERT INTO audit_log (id, actor_id, action, ref) \
             VALUES (nextval('audit_log_id_seq'), NULL, 'exam.penalty', \
             $1::jsonb)",
        )
        .bind(ref_json)
        .execute(&mut *tx)
        .await?;
        if let Some(n) = note_of(item, &applied) {
            notes.push(n);
        }
    }
    tx.commit().await?;
    Ok(notes)
}

/// 通知文案片段：实况优先于配置（扣了 300 不写「扣 500」——
/// settle_fail 的罚金文案曾有同类失实教训）。
pub(crate) fn note_of(
    item: &PenaltyItem,
    applied: &serde_json::Value,
) -> Option<String> {
    Some(match item.kind.as_str() {
        KIND_BAN => "账号已被封禁".into(),
        KIND_SUSPEND => "账号已被挂起（暂停下载资格）".into(),
        KIND_RESTRICT_DOWNLOAD => "下载权限已被暂停".into(),
        KIND_MUTE => "账号已被禁言".into(),
        KIND_BAN_UPLOAD => "发布权限已被暂停".into(),
        KIND_BAN_PAGES => {
            let denied = applied
                .get("denied")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str())
                        .collect::<Vec<_>>()
                        .join("、")
                })
                .unwrap_or_default();
            if denied.is_empty() {
                "部分页面访问权限已被限制".into()
            } else {
                format!("以下权限已被限制：{denied}")
            }
        }
        KIND_SPARK_FINE => format!(
            "扣除魔力 {}（实扣）",
            applied.get("took").and_then(|v| v.as_i64()).unwrap_or(0)
        ),
        KIND_TRAFFIC_FINE => {
            let col = if applied.get("target").and_then(|v| v.as_str())
                == Some("downloaded")
            {
                "下载量"
            } else {
                "上传量"
            };
            format!(
                "扣除{} {}",
                col,
                applied.get("took").and_then(|v| v.as_i64()).unwrap_or(0)
            )
        }
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_filters_unknown_kinds() {
        let v = serde_json::json!([
            { "kind": "ban" },
            { "kind": "spark_fine", "amount": 500 },
            { "kind": "no_such_thing" },
        ]);
        let items = parse_penalty(v);
        assert_eq!(items.len(), 2);
        assert_eq!(items[1].amount, Some(500));
    }

    #[test]
    fn parse_empty_and_garbage_both_fall_back() {
        assert!(parse_penalty(serde_json::json!([])).is_empty());
        assert!(parse_penalty(serde_json::json!({})).is_empty());
        assert!(parse_penalty(serde_json::json!("nope")).is_empty());
    }

    #[test]
    fn note_matches_actual_deduction() {
        let item = PenaltyItem {
            kind: KIND_SPARK_FINE.into(),
            amount: Some(500),
            target: None,
            pages: None,
        };
        // 余额不足只扣到 0：文案必须是实扣 300 而不是配置的 500
        let note =
            note_of(&item, &serde_json::json!({ "took": 300, "wanted": 500 }))
                .unwrap();
        assert!(note.contains("300"), "{note}");
        assert!(!note.contains("500"), "{note}");
    }

    #[test]
    fn traffic_note_distinguishes_target() {
        let up = PenaltyItem {
            kind: KIND_TRAFFIC_FINE.into(),
            amount: Some(1024),
            target: None,
            pages: None,
        };
        let n1 = note_of(
            &up,
            &serde_json::json!({ "target": "uploaded", "took": 1024 }),
        )
        .unwrap();
        assert!(n1.contains("上传量"), "{n1}");
        let n2 = note_of(
            &up,
            &serde_json::json!({ "target": "downloaded", "took": 1024 }),
        )
        .unwrap();
        assert!(n2.contains("下载量"), "{n2}");
    }

    /// 流量扣减被 GREATEST 夹到 0 时，文案不能写请求值。
    #[test]
    fn traffic_note_reports_actual_not_wanted() {
        let it = PenaltyItem {
            kind: KIND_TRAFFIC_FINE.into(),
            amount: Some(500),
            target: None,
            pages: None,
        };
        let applied = serde_json::json!({
            "target": "uploaded",
            "took": 100,
            "wanted": 500,
        });
        let n = note_of(&it, &applied).unwrap();
        assert!(n.contains("100"), "{n}");
        assert!(!n.contains("500"), "{n}");
    }
}
