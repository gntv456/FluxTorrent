//! 批量发放（好学 increment-bulk.php 口径）：火花/上传量/邀请/补签卡 ×（等级|职务|指定用户）。
//! 从 admin_p3_http.rs 按域拆出；校验与目标选择见 increment_targets.rs，
//! 各 kind 的发放实现见 increment_grant.rs（0286 拆出守行数门禁）。

use actix_web::{post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use super::increment_grant::grant_kind;
use super::increment_targets::{
    increment_bulk_targets, increment_bulk_validate,
};
use super::staff;

use crate::dto::ok;
use crate::errors::DomainResult;
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
    let targets: Vec<i64> = increment_bulk_targets(db, &body).await?;
    // dry_run（0286 建议②）：只返回命中清单不发——按等级/职务圈人没有
    // 预览时，误选受众会立刻真发。审计同样留痕（防试探性圈人无记录）
    if body.dry_run {
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
                })),
            )
            .await;
        return Ok(ok(serde_json::json!({
            "dry_run": true,
            "kind": body.kind,
            "amount": body.amount,
            "targets": targets.len() as i64,
            "target_ids": targets,
        })));
    }
    let batch_id = uuid::Uuid::new_v4().simple().to_string();
    let sender_id: Option<i64> = if body.sender.as_deref() == Some("system") {
        None
    } else {
        Some(auth.id)
    };

    // 分批执行（每批 500，避免长事务）
    let mut affected: u64 = 0;
    for chunk in targets.chunks(500) {
        affected +=
            grant_kind(&body, db, &batch_id, chunk, &auth).await?;
        super::bulk_notify::pm_chunk(
            db,
            chunk,
            body.subject.as_deref().unwrap_or(""),
            body.body.as_deref().unwrap_or(""),
            sender_id,
            body.email,
        )
        .await?;
    }

    // 审计带明细（0286 P2a）：旧版 action 只记 kind、ref 全空——发了多少/
    // 发给谁事后无法回答。extra 落 batch_id/amount/受众口径/命中数/目标抽样
    // （前 20 个 id；全量清单走响应返回值，审计只留可检索的锚点）
    let mut selector = serde_json::Map::new();
    if !body.user_ids.is_empty() {
        selector.insert(
            "user_ids".to_string(),
            serde_json::json!(&body.user_ids[..body.user_ids.len().min(20)]),
        );
    }
    if !body.classes.is_empty() {
        selector.insert("classes".to_string(), serde_json::json!(&body.classes));
    }
    if !body.roles.is_empty() {
        selector.insert("roles".to_string(), serde_json::json!(&body.roles));
    }
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
                "sample_targets": &targets[..targets.len().min(20)],
                "selector": selector,
            })),
        )
        .await;
    Ok(ok(serde_json::json!({
        "affected": affected as i64,
        "targets": targets.len() as i64,
        "kind": body.kind,
        "amount": body.amount,
        "batch_id": batch_id,
        // 发放回执（0286 建议③）：返回完整命中清单，管理者核对漏发/留档
        "target_ids": targets,
    })))
}
