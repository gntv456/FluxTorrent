use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

use super::module_disabled;

#[get("/textbooks")]
pub(super) async fn textbook_list(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    if module_disabled(&state.repo.db, "textbooks").await {
        return Err(DomainError::NotFound(0)); // 模块已关闭：与前端导航隐藏同口径
    }
    let rows = sqlx::query_as::<_, TextbookRow>(
        "SELECT tb.id, tb.subject, e.name AS edition, g.name AS grade, tb.volume, tb.publisher, tb.downloads, \
            (SELECT min(t.id) FROM torrents t WHERE t.textbook_id = tb.id AND t.approval_status = 1) AS torrent_id \
         FROM textbooks tb \
         JOIN editions e ON e.id = tb.edition_id \
         JOIN grades g ON g.id = tb.grade_id \
         ORDER BY tb.downloads DESC, tb.id LIMIT 100",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

#[derive(Deserialize)]
struct TextbookLinkReq {
    torrent_id: i64,
    textbook_id: i64,
}

#[post("/textbooks/link")]
pub(super) async fn textbook_link(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<TextbookLinkReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if module_disabled(&state.repo.db, "textbooks").await {
        return Err(DomainError::NotFound(0)); // 模块已关闭
    }
    let updated =
        sqlx::query("UPDATE torrents SET textbook_id = $2 WHERE id = $1")
            .bind(body.torrent_id)
            .bind(body.textbook_id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::NotFound(body.torrent_id));
    }
    state
        .repo
        .audit(Some(auth.id), "textbook_link", Some(body.torrent_id))
        .await;
    Ok(ok(serde_json::json!({ "linked": true })))
}

// ============ M20 排行榜（六榜卡片：最多魔力/上传量/下载量/最长做种时间/后宫时魔/发种量） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TopRow {
    rank: i64,
    username: String,
    class_name: String,
    title: Option<String>,
    avatar_url: Option<String>,
    #[sqlx(default)]
    avatar_frame_css: Option<String>,
    #[sqlx(default)]
    avatar_frame_image: Option<String>,
    val: f64,
}

/// 六个榜单统一口径：status<2、每榜 Top10；后宫时魔 = 做种时魔（与 worker 小时结算同式：
/// 基础10 + 做种数×2 + 做种体积TB，捐赠者×2）
#[get("/top/boards")]
pub(super) async fn top_boards(
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let sel = "row_number() OVER (ORDER BY val DESC) AS rank, u.username, c.name AS class_name, \
        u.title, u.avatar_url, f.css AS avatar_frame_css, f.image_url AS avatar_frame_image, x.val::float8 AS val";
    let base = |agg: &str,
                joins: &str,
                extra_where: &str,
                group_by: &str|
     -> String {
        format!(
            "SELECT {sel} FROM ( \
                SELECT u.id AS uid, {agg} AS val \
                FROM users u {joins} \
                WHERE u.status < 2 {extra_where} \
                GROUP BY u.id {group_by} \
                ORDER BY val DESC LIMIT 10 \
            ) x JOIN users u ON u.id = x.uid JOIN user_classes c ON c.id = u.class_id \
            LEFT JOIN avatar_frames f ON f.id = u.avatar_frame_id"
        )
    };
    let bonus_q = base("u.spark_balance", "", "AND u.spark_balance > 0", "");
    let uploaded_q = base("u.uploaded", "", "AND u.uploaded > 0", "");
    let downloaded_q = base("u.downloaded", "", "AND u.downloaded > 0", "");
    let seedtime_q = base(
        "COALESCE(sum(s.seeded_seconds), 0) / 3600.0",
        "JOIN snatches s ON s.user_id = u.id",
        "",
        "",
    );
    // 审计修复（P2）：seeding INNER JOIN 在无做种用户时全空导致恒空榜。
    // 改 LEFT JOIN + HAVING count>0：有做种记录者才进榜，口径与做种收益一致
    let hourly_q = base(
        "(10 + count(*) * 2 + COALESCE(sum(t.size), 0) / 1099511627776.0) \
            * CASE WHEN u.donor THEN 2 ELSE 1 END",
        "JOIN snatches s ON s.user_id = u.id AND s.seeding LEFT JOIN torrents t ON t.id = s.torrent_id",
        "",
        "HAVING count(*) > 0",
    );
    let torrents_q = base(
        "count(*)",
        "JOIN torrents t ON t.owner_id = u.id AND t.approval_status = 1",
        "",
        "",
    );

    let mut boards = serde_json::Map::new();
    for (key, q) in [
        ("bonus", bonus_q),
        ("uploaded", uploaded_q),
        ("downloaded", downloaded_q),
        ("seedtime", seedtime_q),
        ("hourly", hourly_q),
        ("torrents", torrents_q),
    ] {
        let rows = sqlx::query_as::<_, TopRow>(&q)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        boards.insert(
            key.to_string(),
            serde_json::to_value(rows).unwrap_or_default(),
        );
    }
    Ok(ok(serde_json::Value::Object(boards)))
}

// ============ 用户自购置顶/限时免费（0101，好学站插件口径） ============

#[derive(sqlx::FromRow, serde::Serialize)]
struct TextbookRow {
    id: i64,
    subject: String,
    edition: String,
    grade: String,
    volume: Option<String>,
    publisher: Option<String>,
    downloads: i32,
    torrent_id: Option<i64>,
}
