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

/// 课本 ↔ 种子关联。**守卫与种子编辑同一条门槛**：作者本人恒可，动别人的
/// 按站点设定 `torrent_edit_class`（缺省 94）——此前这个口只验登录，任何
/// 会员在 /textbooks 页面输一个别人的种子号就能改写它的教材归属（策展数据
/// 被外人改），还能把未过审的种挂上去。
/// 教材号不存在也不再是 500：`torrents.textbook_id` 有外键，原先直接撞约束
/// 被读成 Internal，现在与种子的存在性一起判掉，回 1004（界面 toast 说
/// 「资源不存在」）。
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
    let row: Option<(i64, bool)> = sqlx::query_as(
        "SELECT t.owner_id, EXISTS(SELECT 1 FROM textbooks b WHERE b.id = \
         $2) AS book_ok FROM torrents t WHERE t.id = $1 \
         AND t.approval_status = 1",
    )
    .bind(body.torrent_id)
    .bind(body.textbook_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner_id, book_ok)) = row else {
        return Err(DomainError::NotFound(body.torrent_id));
    };
    // 门槛口径与种子编辑同一条（interact.rs::edit_torrent 0159 阈值化）：
    // 作者本人恒可，动别人的按站点设定 torrent_edit_class（缺省 94）。
    if owner_id != auth.id {
        let threshold =
            crate::torrents::manage_perm::edit_threshold(&state.repo.db).await;
        if auth.class_id < i32::from(threshold) {
            return Err(DomainError::Forbidden);
        }
    }
    if !book_ok {
        return Err(DomainError::NotFound(body.textbook_id));
    }
    let updated =
        sqlx::query("UPDATE torrents SET textbook_id = $2 WHERE id = $1")
            .bind(body.torrent_id)
            .bind(body.textbook_id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    // 上面两道判据与这条 UPDATE 之间有窗口（种被删/被打回），0 行不许
    // 回「已关联」——那是收钱不发货的轻量同族。
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
    /// 榜主的 user id（P-33：排行榜的用户名链接要有真实去处，
    /// 资料页是 `/users/[id]`，此前榜行只带 username，链过去是自环）
    id: i64,
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

/// 六个榜单统一口径：status<2、每榜 Top10。
/// 后宫时魔不在这里写算式——它取 `spark_ledger` 里每人最近一条
/// kind='seeding_reward' 的实发额，本体永远是 worker 那一份
/// （DB 函数 `seeding_hourly` + `seeding_torrent_bonus`，参数在 site_settings）。
#[get("/top/boards")]
pub(super) async fn top_boards(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    // P-308：六榜都是会员数据（用户名 / 等级名 / 绝对上传下载量 / 做种时长 /
    // 发种量 / 头像框样式），此前匿名可直拉。路由侧 `/top` 不在 middleware 的
    // 匿名白名单（匿名访问被 307 到 /login），API 必须同一口径，否则「同一策略
    // 两处实现、弱的那处说了算」。前台是 RSC，取数走 lib/api-client.ts 的显式
    // 会话转发（第 101-120 行：跨源直连时 cookie 不随行，改带 Bearer/cookie），
    // 所以会员侧不受影响。
    let _auth = require_auth(&req, &state).await?;
    let sel = "row_number() OVER (ORDER BY val DESC) AS rank, x.uid AS id, \
        u.username, c.name AS class_name, \
        u.title, u.avatar_url, f.css AS avatar_frame_css, f.image_url AS avatar_frame_image, x.val::float8 AS val";
    // 外层补身份列：六个榜的内层都只出 (uid, val)，尾部这一段是共享的
    let tail = "JOIN users u ON u.id = x.uid JOIN user_classes c ON c.id = u.class_id \
        LEFT JOIN avatar_frames f ON f.id = u.avatar_frame_id";
    let base =
        |agg: &str, joins: &str, extra_where: &str, group_by: &str| -> String {
            format!(
                "SELECT {sel} FROM ( \
                SELECT u.id AS uid, {agg} AS val \
                FROM users u {joins} \
                WHERE u.status < 2 {extra_where} \
                GROUP BY u.id {group_by} \
                ORDER BY val DESC, u.id LIMIT 10 \
            ) x {tail}"
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
    // 后宫时魔榜读**上一次真正结算到的那笔**，不再自己复刻算式。
    // 旧写法把 0133 之前的算式（基础 10 + 做种数×2 + 体积TB，捐赠×2）留在榜里，
    // 而结算本体早已换成 DB 函数 seeding_hourly(Σ seeding_torrent_bonus(…))，
    // 参数在 site_settings 可热调（实测现值 base=10 cap=150 curve_k=0.12
    // scale=0.4 donor_mult=2）。两套式子并存的后果：榜按「做种颗数 × 体积」
    // 排序，恰好把 0133 立档要堵的「堆小种」排到前面
    // （见 apps/worker/src/jobs/seeding.rs 顶部的公式与标定注释）。
    // 口径照 seeding.rs 自己那句「权威在流水，快照仅展示」：取每人最近一条
    // kind='seeding_reward' 的实发额。没结算过的人不进榜（不是 0，是不适用），
    // 顺带也不再出现「全站没人挂着做种 ⇒ 整榜空白」那张死榜。
    let hourly_q = format!(
        "SELECT {sel} FROM ( \
            SELECT l.uid, l.val FROM ( \
                SELECT DISTINCT ON (g.user_id) g.user_id AS uid, \
                       g.amount::float8 AS val \
                FROM spark_ledger g WHERE g.kind = 'seeding_reward' \
                ORDER BY g.user_id, g.created_at DESC, g.id DESC \
            ) l JOIN users lu ON lu.id = l.uid \
            WHERE lu.status < 2 AND l.val > 0 \
            ORDER BY l.val DESC, l.uid LIMIT 10 \
        ) x {tail}"
    );
    // 发种量榜的口径必须与「公开把匿名种算给谁」一致：匿名种不记名。
    // 全站对 anonymous 的处理是**一律隐去发布者**——详情页与列表与 RSS 与保种页
    // 都写 `CASE WHEN t.anonymous THEN NULL ELSE u.username END`，
    // 本人主页的 uploads 与 /users/{id}/torrentlist 也都带 `NOT t.anonymous`
    // （profile.rs:47、user_torrentlist.rs:33）。这一榜原先只过 approval_status，
    // 于是「匿名发 5 颗种」的人主页写着「发布 0 种」，却以真名挂在全站排行榜第 1，
    // 榜替站方把匿名性卖了——还顺带造成两处数字对不上。
    let torrents_q = base(
        "count(*)",
        "JOIN torrents t ON t.owner_id = u.id \
         AND t.approval_status = 1 AND NOT t.anonymous",
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

// ============ 用户自购置顶/限时免费（0101，同类站插件口径） ============

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
