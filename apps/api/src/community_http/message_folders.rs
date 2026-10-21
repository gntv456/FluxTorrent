//! M16 信箱文件夹（pmboxes）：列表 / 新建 / 改名 / 删除。
//! 从 `message_box.rs` 按域拆出（该文件已贴近 300 行门禁，
//! 而 P-322 的两处修法要加守卫与单元测试，不拆就会顶破门禁）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 文件夹列表 + 夹内未删信件数
#[utoipa::path(
    get,
    path = "/api/v1/messages/boxes",
    tag = "社区/消息",
    description = "自定义文件夹列表（含每夹未删信件数）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）")))]
#[get("/messages/boxes")]
pub(crate) async fn message_boxes(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows = crate::repo::pmboxes::list_boxes(&state.repo, auth.id).await;
    Ok(ok(rows))
}

/// P-322①：`deny_unknown_fields` 是必需的。读口把文件夹参数叫 `box_id`
/// （`message.rs` 的 `InboxQuery::box_id`），写口这个字段叫 `id`——拼错键名
/// 时 serde 默认忽略未知键并把 `id` 当 `None`，于是「改名」静默变成
/// 「新建一个同名文件夹」并回 200（2026-10-15 现网实测：发
/// `{box_id:999, name:'EP322改名测试'}` 得到 `{id:33}`，999 那个夹子
/// 根本没被碰，却凭空多出一个文件夹）。
#[derive(Debug, Deserialize, utoipa::ToSchema)]
#[serde(deny_unknown_fields)]
struct BoxReq {
    /// 缺省 = 新建；有值 = 改名/删除该 id
    id: Option<i64>,
    name: String,
}

/// 文件夹写口的错误归一（P-490/P-491）：`pmboxes` 有
/// `UNIQUE (user_id, name)`；重名在旧版被 `unwrap_or_default` 吞成 0 行，
/// 于是「改名撞名」回 404 资源不存在（0.10 实测），「新建撞名」更被
/// `ON CONFLICT DO UPDATE` 演成 200 新建成功而列表一封没多。
/// 两处都改成把唯一键冲突译成可读的 1002。
fn map_box_write<T>(r: Result<T, sqlx::Error>) -> DomainResult<T> {
    r.map_err(|e| match e {
        sqlx::Error::Database(db) if db.is_unique_violation() => {
            DomainError::Validation("已有同名文件夹".into())
        }
        e => DomainError::Internal(e.into()),
    })
}

/// 新建/改名/删除（editmailboxes 口径：一人最多 3 个、名 ≤14 字；
/// 清空名 = 删除并连带清信）
#[utoipa::path(
    post,
    path = "/api/v1/messages/boxes",
    tag = "社区/消息",
    description = "新建/改名/删除文件夹（改名传 id；清空 name 即删除并连带清信）。",
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 422, description = "参数校验失败（code 1002）")))]
#[post("/messages/boxes")]
pub(crate) async fn message_box_upsert(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<BoxReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let name = body.name.trim();
    // P-322②：删除分支原先写成 `if let Some(bid) = body.id { … }`，
    // 没有 id 就直接落到 `return deleted:true` —— 现网实测发
    // `{name:'  '}`（root 名下 0 个文件夹、0 封夹内信）照样回
    // `{"deleted":true}`，界面据此把行抹掉，而什么都没删。
    // 与同函数改名分支同一口径：没有目标就明说，别把「没动」报成「删了」。
    if name.is_empty() {
        let Some(bid) = body.id else {
            return Err(DomainError::Validation(
                "删除文件夹要给出它的 id（清空名称即表示删除）".into(),
            ));
        };
        // P-489：repo 里做的是「双删语义」——发件方仍存副本时只摘
        // 收件方这一侧，不再物理抹掉（旧版无条件 DELETE，实测把
        // sender 发件箱里 saved=1 的信一起删了）。
        let gone = map_box_write(
            crate::repo::pmboxes::delete_box(&state.repo, auth.id, bid).await,
        )?;
        if gone == 0 {
            return Err(DomainError::NotFound(bid));
        }
        return Ok(ok(serde_json::json!({ "deleted": true })));
    }
    if name.chars().count() > 14 {
        return Err(DomainError::Validation("文件夹名最多 14 字".into()));
    }
    match body.id {
        None => {
            let n =
                crate::repo::pmboxes::count_boxes(&state.repo, auth.id).await;
            if n >= 3 {
                return Err(DomainError::Validation("最多 3 个文件夹".into()));
            }
            let id = map_box_write(
                crate::repo::pmboxes::insert_box(&state.repo, auth.id, name)
                    .await,
            )?;
            Ok(ok(serde_json::json!({ "id": id })))
        }
        Some(bid) => {
            let n = map_box_write(
                crate::repo::pmboxes::rename_box(
                    &state.repo,
                    auth.id,
                    bid,
                    name,
                )
                .await,
            )?;
            if n == 0 {
                return Err(DomainError::NotFound(bid));
            }
            Ok(ok(serde_json::json!({ "renamed": true })))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BoxReq;

    /// P-322①：拼错键名（`box_id`/`boxId`）必须报错。否则 serde 忽略未知键
    /// 把 `id` 落成 None，「改名」就静默变成「新建一个文件夹」还回 200。
    #[test]
    fn boxes_reject_mistyped_field_names() {
        let bad: Result<BoxReq, _> =
            serde_json::from_str(r#"{"box_id":2,"name":"改名"}"#);
        assert!(bad.is_err(), "未知键 box_id 必须被拒");
        assert!(
            bad.unwrap_err().to_string().contains("unknown field"),
            "报错要指出是哪个键"
        );
        let good: Result<BoxReq, _> =
            serde_json::from_str(r#"{"id":2,"name":"改名"}"#);
        assert!(good.is_ok(), "正控：正确字段名必须收");
        let created: Result<BoxReq, _> =
            serde_json::from_str(r#"{"name":"新建"}"#);
        assert!(created.is_ok(), "正控：省略 id = 新建，仍要收");
    }
}
