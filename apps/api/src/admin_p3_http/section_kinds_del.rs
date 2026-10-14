//! 内容维度删除口：从 section_kinds.rs 按域拆出（300 行门禁）。
//! 删除会连带清空字典值与种子归属，护栏要把引用面数全（P-221 同族）。

use actix_web::{delete, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

use super::staff;

#[delete("/admin/section-kinds/{kind}")]
async fn section_kinds_delete(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<String>,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::CATEGORIES_MANAGE,
    )
    .await?;
    let kind = path.into_inner();
    // 内置维度不再冻结：项目定位是通用建站，维度归站长自定义（media/grades/
    // editions 是教育站时代的内置维，冻结会让站长删不掉「学段」这类不相干维度，
    // 也会被 pack_apply 删空后陷入「已内置、不能重建」的死锁）。
    // 数据安全由下面的「在用量」守卫兜住。
    // 删除会级联清空字典与种子归属，先挡在用中的维度。
    // B2：判据必须覆盖**自由值行**（dict_id IS NULL）——原实现 JOIN section_dict，
    // 自由值行不在该表里、计数为 0，站长删一个只被自由值使用的维度会静默丢数据。
    let (used, dicts): (i64, i64) = sqlx::query_as(
        "SELECT (SELECT count(*) FROM torrent_sections WHERE kind = $1), \
         (SELECT count(*) FROM section_dict WHERE kind = $1)",
    )
    .bind(&kind)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if used > 0 {
        return Err(DomainError::Validation(
            "该维度下已有种子在用，不能删除".into(),
        ));
    }
    // P-221 同族：section_dict.kind 对本表是 ON DELETE CASCADE，只数
    // torrent_sections 会让「建了字典值还没被种子用过」的维度删掉时
    // 把字典值一起带走，界面与账面都不知道少了什么。
    if dicts > 0 {
        return Err(DomainError::Validation(format!(
            "该维度仍有 {dicts} 个字典值（删除会连带清空），请先删字典值"
        )));
    }
    let gone: Option<String> =
        sqlx::query_scalar("SELECT label FROM section_kinds WHERE kind = $1")
            .bind(&kind)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let n = sqlx::query("DELETE FROM section_kinds WHERE kind = $1")
        .bind(&kind)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?
        .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(0));
    }
    state
        .repo
        .audit_detail(
            Some(auth.id),
            "section_kind.del",
            None,
            None,
            Some(serde_json::json!({"kind": kind, "label": gone})),
        )
        .await;
    Ok(ok(serde_json::json!({ "deleted": kind })))
}
