//! M16 信箱文件夹（pmboxes）：列表 / 新建 / 改名 / 删除。
//! 从 `message_box.rs` 按域拆出（该文件已贴近 300 行门禁，
//! 而 P-322 的两处修法要加守卫与单元测试，不拆就会顶破门禁）。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct PmBoxRow {
    id: i64,
    name: String,
    count: i64,
}

/// 文件夹列表 + 夹内未删信件数
#[get("/messages/boxes")]
async fn message_boxes(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let rows = sqlx::query_as::<_, PmBoxRow>(
        "SELECT b.id, b.name, (SELECT count(*) FROM messages m WHERE \
         m.folder = b.id AND m.receiver_id = $1 AND m.location = 1) AS count \
         FROM pmboxes b WHERE b.user_id = $1 ORDER BY b.id",
    )
    .bind(auth.id)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// P-322①：`deny_unknown_fields` 是必需的。读口把文件夹参数叫 `box_id`
/// （`message.rs` 的 `InboxQuery::box_id`），写口这个字段叫 `id`——拼错键名
/// 时 serde 默认忽略未知键并把 `id` 当 `None`，于是「改名」静默变成
/// 「新建一个同名文件夹」并回 200（2026-10-15 现网实测：发
/// `{box_id:999, name:'EP322改名测试'}` 得到 `{id:33}`，999 那个夹子
/// 根本没被碰，却凭空多出一个文件夹）。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BoxReq {
    /// 缺省 = 新建；有值 = 改名/删除该 id
    id: Option<i64>,
    name: String,
}

/// 新建/改名/删除（editmailboxes 口径：一人最多 3 个、名 ≤14 字；
/// 清空名 = 删除并连带清信）
#[post("/messages/boxes")]
async fn message_box_upsert(
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
        sqlx::query("DELETE FROM messages WHERE folder = $2 AND \
                     receiver_id = $1")
            .bind(auth.id)
            .bind(bid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        let gone = sqlx::query("DELETE FROM pmboxes WHERE id = $2 AND \
                                user_id = $1")
            .bind(auth.id)
            .bind(bid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
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
            let n: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM pmboxes WHERE user_id = $1",
            )
            .bind(auth.id)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or(0);
            if n >= 3 {
                return Err(DomainError::Validation("最多 3 个文件夹".into()));
            }
            let id: i64 = sqlx::query_scalar(
                "INSERT INTO pmboxes (user_id, name) VALUES ($1, $2) \
                 ON CONFLICT (user_id, name) DO UPDATE SET \
                 name = EXCLUDED.name RETURNING id",
            )
            .bind(auth.id)
            .bind(name)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            Ok(ok(serde_json::json!({ "id": id })))
        }
        Some(bid) => {
            let n = sqlx::query(
                "UPDATE pmboxes SET name = $3 WHERE id = $2 AND \
                 user_id = $1",
            )
            .bind(auth.id)
            .bind(bid)
            .bind(name)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .rows_affected();
            if n == 0 {
                return Err(DomainError::NotFound(0));
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
