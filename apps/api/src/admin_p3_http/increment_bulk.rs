//! 批量发放（好学 increment-bulk.php 口径）：火花/上传量/邀请/补签卡 ×（等级|职务|指定用户）。
//! 从 admin_p3_http.rs 按域拆出；校验与目标选择见 increment_targets.rs，
//! 各 kind 的发放实现见 increment_grant.rs（0286 拆出守行数门禁）。
//!
//! 0291 补三条发放管理必需口径：
//! - **跨请求幂等**：`idempotency_key` 撞 `grant_batches` 唯一约束即拒。此前
//!   batch_id 每次新生成、幂等键又嵌着它，所以「同参数点两次」是两批真发
//!   （实测魔力 +200）；界面上那个「试运行」旁边就是「发放」，手抖代价是钱。
//! - **台账**：每批先占位（pending）再执行，结果回写 done/partial，
//!   受众全量入库（旧版只在审计里留前 20 个抽样，漏发无从核对）。
//! - **试运行要看库存**：只解析受众不查库存的 dry_run 是假绿——真发时才撞闸。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::increment_grant::grant_kind;
use super::increment_stock::{stock_item_id, stock_read};
use super::increment_targets::{
    increment_bulk_targets, increment_bulk_validate,
};
use super::staff;
use super::{batch_finish, batch_open, BatchSpec};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[derive(Deserialize)]
pub(super) struct IncrementBulkReq {
    /// spark | uploaded | invite | resub_card | medal | item
    pub(super) kind: String,
    /// spark/invite/resub_card 为数量；uploaded 为 GB（正加负减）
    pub(super) amount: i64,
    /// 目标等级（多选）；与 roles/user_ids 至少其一
    #[serde(default)]
    pub(super) classes: Vec<i32>,
    /// 目标职务（多选）
    #[serde(default)]
    pub(super) roles: Vec<String>,
    /// 指定用户（优先于 classes/roles）
    #[serde(default)]
    pub(super) user_ids: Vec<i64>,
    /// 临时邀请有效期（天，1-365）：仅 kind=invite 时有效——直接生成 N 天到期的邀请码
    /// （好学 tmp_invites 口径）；缺省走 quota_extra 配额
    #[serde(default)]
    pub(super) days: Option<i32>,
    /// PM 通知（可选；空则不发）
    #[serde(default)]
    pub(super) subject: Option<String>,
    #[serde(default)]
    pub(super) body: Option<String>,
    /// 发送者：self = 操作者，system = 系统私信（sender NULL）
    #[serde(default)]
    pub(super) sender: Option<String>,
    /// 同时发邮件（闭环审查 C13：旧版只发站内信，当事人不上站就蒙在鼓里）。
    /// 缺省 false 保持旧行为；SMTP 未配置时由 mailer 降级为日志（不报错）。
    #[serde(default)]
    pub(super) email: bool,
    /// kind=medal：勋章 id（0204）
    #[serde(default)]
    pub(super) medal_id: Option<i64>,
    /// kind=medal 的有效期覆盖（天，1-3650；0287 与单发 days 对齐）：
    /// 缺省随 medals.duration_days
    #[serde(default)]
    pub(super) medal_days: Option<i32>,
    /// kind=item：道具 id（0204）
    #[serde(default)]
    pub(super) item_id: Option<i64>,
    /// 试运行（0286）：只解析受众并返回命中清单，不实际发放
    #[serde(default)]
    pub(super) dry_run: bool,
    /// 幂等键（0291，8~120 字符）：同键重放直接拒，防双击双发。
    /// 与单号调账 `/admin/users/adjust` 口径一致。
    #[serde(default)]
    pub(super) idempotency_key: Option<String>,
}

#[post("/admin/increment-bulk")]
async fn increment_bulk(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<IncrementBulkReq>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    increment_bulk_validate(&state, &auth, &body).await?;
    let db = &state.repo.db;
    let idem = body
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|k| !k.is_empty());
    if let Some(k) = idem {
        if k.len() < 8 || k.len() > 120 {
            return Err(DomainError::Validation(
                "idempotency_key 需 8~120 字符".into(),
            ));
        }
    }
    let targets: Vec<i64> = increment_bulk_targets(db, &body).await?;
    // dry_run（0286 建议② + 0291 库存体检）：解析受众之外还要回答
    // 「这一批发得出去吗」——只报受众的试运行会让站长按「能发」去点真发。
    if body.dry_run {
        let needed = body.amount.max(0) * targets.len() as i64;
        let (stock_item, stock_ok, stock_left) =
            match stock_item_id(db, &body).await? {
                Some(iid) => {
                    let (used, quota) = stock_read(db, iid).await?;
                    let left = quota.map(|q| q - used);
                    (Some(iid), left.map_or(true, |l| l >= needed), left)
                }
                None => (None, true, None),
            };
        state
            .repo
            .audit_detail(
                Some(auth.id),
                "increment_bulk.dry_run",
                None,
                None,
                Some(serde_json::json!({
                    "kind": &body.kind,
                    "amount": body.amount,
                    "targets": targets.len(),
                    "stock_ok": stock_ok,
                })),
            )
            .await;
        return Ok(ok(serde_json::json!({
            "dry_run": true,
            "kind": body.kind,
            "amount": body.amount,
            "targets": targets.len() as i64,
            "target_ids": targets,
            "stock_ok": stock_ok,
            "stock_item_id": stock_item,
            "stock_needed": needed,
            "stock_left": stock_left,
        })));
    }
    let batch_id = uuid::Uuid::new_v4().simple().to_string();
    let sender_id: Option<i64> = if body.sender.as_deref() == Some("system") {
        None
    } else {
        Some(auth.id)
    };

    // 台账占位：幂等键在这里撞唯一约束（INSERT 上撞，不是先查再插——
    // 先查再插时并发双击两道都能查到「没用过」）
    let mut selector = serde_json::Map::new();
    if !body.user_ids.is_empty() {
        selector.insert(
            "user_ids".to_string(),
            serde_json::json!(&body.user_ids[..body.user_ids.len().min(500)]),
        );
    }
    if !body.classes.is_empty() {
        selector.insert(
            "classes".to_string(),
            serde_json::json!(&body.classes),
        );
    }
    if !body.roles.is_empty() {
        selector.insert("roles".to_string(), serde_json::json!(&body.roles));
    }
    let spec = BatchSpec {
        batch_id: &batch_id,
        idem,
        actor_id: auth.id,
        kind: &body.kind,
        amount: body.amount,
        item_id: body.item_id,
        medal_id: body.medal_id,
        days: body.days.or(body.medal_days),
        selector: serde_json::Value::Object(selector.clone()),
        targets: &targets,
        subject: body.subject.as_deref(),
    };
    batch_open(db, &spec).await?;

    // 分批执行（每批 500，一批一个事务）
    let mut affected: u64 = 0;
    let mut done_chunks: usize = 0;
    for chunk in targets.chunks(500) {
        match grant_kind(&body, db, &batch_id, chunk, &auth).await {
            Ok(n) => affected += n,
            Err(e) => {
                let msg = format!("{e}");
                if done_chunks == 0 {
                    // 一批都没发出去 ⇒ 这就是普通的校验/库存拒绝，原样透传
                    // 状态码（400）。把它包成 500 会让站长以为服务端炸了，
                    // 然后去重试同一批。
                    let _ = batch_finish(db, &batch_id, 0, "failed",
                                         Some(&msg)).await;
                    return Err(e);
                }
                // 前面几批是**真发出去了**的：把这批标成 partial 落库，
                // 并把已发人数写进错误文案。只回一个 500 会让站长在
                // 「以为没发」和「实际发了一半」之间猜，然后重发一遍。
                let _ = batch_finish(
                    db,
                    &batch_id,
                    affected as i64,
                    "partial",
                    Some(&format!(
                        "第 {} 批失败：{msg}",
                        done_chunks + 1
                    )),
                )
                .await;
                return Err(DomainError::Internal(anyhow::anyhow!(
                    "本批 {batch_id} 已发放 {affected} 人后中断（{msg}）；\
                     前面的人已真实到账，用 batch 号回查台账再决定补发"
                )));
            }
        }
        if let Err(e) = super::bulk_notify::pm_chunk(
            db,
            chunk,
            body.subject.as_deref().unwrap_or(""),
            body.body.as_deref().unwrap_or(""),
            sender_id,
            body.email,
        )
        .await
        {
            let msg = format!("{e}");
            let _ = batch_finish(
                db,
                &batch_id,
                affected as i64,
                "partial",
                Some(&format!("通知失败：{msg}")),
            )
            .await;
            return Err(DomainError::Internal(anyhow::anyhow!(
                "发放已完成 {affected} 人，但站内信发送中断（{msg}）；\
                 账已落地，无需重发本批"
            )));
        }
        done_chunks += 1;
    }

    // 审计带明细（0286 P2a）：完整受众在 grant_batches，这里留可检索锚点
    state
        .repo
        .audit_detail(
            Some(auth.id),
            &format!("increment_bulk.{}", body.kind),
            None,
            None,
            Some(serde_json::json!({
                "batch_id": batch_id,
                "amount": body.amount,
                "targets": targets.len(),
                "affected": affected,
                "selector": selector,
            })),
        )
        .await;
    let ledger = if batch_finish(db, &batch_id, affected as i64, "done", None)
        .await
        .is_ok()
    {
        "ok"
    } else {
        // 账没回写不等于没发——报出去让人重发才是真事故
        "write_failed"
    };
    Ok(ok(serde_json::json!({
        "affected": affected as i64,
        "targets": targets.len() as i64,
        "kind": body.kind,
        "amount": body.amount,
        "batch_id": batch_id,
        "ledger": ledger,
        // 发放回执（0286 建议③）：返回完整命中清单，管理者核对漏发/留档
        "target_ids": targets,
    })))
}
