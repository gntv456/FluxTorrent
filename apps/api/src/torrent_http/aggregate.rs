//! 详情页 BFF 聚合（六维强化方案批次三）：首屏一次请求带回七块数据。
//!
//! 此前前端详情页要发 6 个并发 + 1 个串行 HTTP（torrent/detail/files/thanks/
//! comments/group/nfo），水合后 tags/subtitles 再各发一次——10 次 RTT、约 14 条 SQL，
//! 且 comments/thanks 计数在「计数端点 + 列表端点」重复聚合。本端点把首屏数据
//! 用 `tokio::join!` 并行折叠成一次响应：RTT 10 → 1（snatches 懒加载保持独立，
//! group/subtitles 因跨 publish/content 模块暂维持独立端点，下一批并入）。
//! 旧端点原样保留（compat 层与旧客户端仍可用）。
//!
//! 缓存（2.5 详情对象缓存）：共享段按 (id, reveal) 30s Redis + 写失效
//! （编辑/定价/删除/恢复/重提时 DEL，见 manage.rs）；个人段（thanks 内含
//! viewer 态）每用户实时，TTL 也不缓存。fail-open：Redis 故障直查库。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Serialize;

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

#[derive(Serialize)]
struct TorrentAggregate {
    torrent: torrents::TorrentRow,
    detail: torrents::TorrentDetailRow,
    files: Vec<torrents::FileRow>,
    thanks: Vec<torrents::ThankRow>,
    comments: Vec<torrents::CommentRow>,
    nfo: Option<String>,
    tags: serde_json::Value,
    /// 抓轨日志清单（0312；正文不进首屏，点开走 /torrents/{id}/logs/{ordinal}）
    logs: Vec<torrents::LogRow>,
    /// 种子工件清单（0323 game/software）：校验和/更新日志（不带正文）
    artifacts: Vec<serde_json::Value>,
    /// 更新链（0323）：挂在本种下的更新包（GGn GameDOX 语义）
    artifact_children: Vec<serde_json::Value>,
}

/// 共享段缓存键：reveal 影响 owner_name 口径，必须分桶。
fn shared_cache_key(id: i64, reveal: bool) -> String {
    format!("cache:tdetail:v1:{id}:{}", if reveal { 1 } else { 0 })
}

/// 写路径失效（manage.rs 各写端点调用；staff 视角桶一并清）。
/// 顺带推进列表缓存代际：种子变了，列表的排序/分类名/统计也跟着变，
/// 只清详情缓存会让列表继续吐旧内容。
pub async fn invalidate_tdetail_cache(state: &AppState, id: i64) {
    let mut c = state.redis.clone();
    for reveal in [false, true] {
        let _: Result<(), _> =
            redis::AsyncCommands::del(&mut c, shared_cache_key(id, reveal))
                .await;
    }
    super::list::bump_list_cache_gen(state).await;
}

#[get("/torrents/{id}/aggregate")]
async fn torrent_aggregate(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let id = path.into_inner();
    // 与 detail 端点同口径：持 view_anonymous 者可见匿名发布者；staff 视角放开暂缓/待审
    let reveal = crate::authz::can(
        &state,
        &auth,
        crate::authz::perm::TORRENT_VIEW_ANONYMOUS,
    )
    .await;
    let viewer = (auth.id, auth.class_id >= 90);

    // ---- 共享段：读缓存（fail-open，与列表 cache:tlist 同范式）----
    let mut c = state.redis.clone();
    let hit: Option<String> =
        redis::AsyncCommands::get(&mut c, shared_cache_key(id, reveal))
            .await
            .unwrap_or(None);
    if let Some(json) = hit {
        if let Ok(v) = serde_json::from_str::<TorrentAggregateShared>(&json) {
            // 命中也要过闸门：缓存桶只按 reveal 分，而取数时管理视角/作者本人
            // 会放宽到 approval_status IN (0,2,4)——不加这行，版主或作者打开
            // 一条未过审种后的 30 秒内，任何普通会员都能拿到它的标题/描述/文件。
            if v.torrent.approval_status == 1 {
                return Ok(ok(assemble(v, auth.id, &state, id).await?));
            }
        }
    }

    let torrent_f =
        torrents::get_torrent(&state.repo.db, id, reveal, Some(viewer));
    let detail_f = torrents::get_torrent_detail(&state.repo.db, id, auth.id);
    let files_f = torrents::list_files(&state.repo.db, id);
    let nfo_f = torrents::get_nfo(&state.repo.db, id, viewer);
    let tags_f = torrents::list_tags(&state.repo.db, id, viewer);
    let logs_f = torrents::list_logs(&state.repo.db, id);
    let arts_f =
        crate::publish_http::list_artifacts_for_detail(&state.repo.db, id);
    let arts_children_f =
        crate::publish_http::list_artifact_children(&state.repo.db, id);

    // torrent/detail 失败才整体短路（种子不存在 → 404）；files/nfo/tags
    // 为纯静态块降级为空。detail/thanks 带 viewer 态，留在个人段实时算。
    let joined = tokio::join!(
        torrent_f,
        detail_f,
        async { files_f.await.unwrap_or_default() },
        async { nfo_f.await.unwrap_or_else(|_| None) },
        async {
            let empty = || serde_json::json!({ "dict": [], "mine": [] });
            tags_f.await.unwrap_or_else(|_| empty())
        },
        async {
            // 降级本身是有意的，但静默降级不是：logs 读失败会让
            // 整个日志板块凭空消失（2026-10-09 实测到过一次空段）
            logs_f.await.unwrap_or_else(|e| {
                tracing::warn!(error = ?e, "aggregate logs 段降级为空");
                Vec::new()
            })
        },
        async { arts_f.await },
        async { arts_children_f.await },
    );
    let (
        torrent_r,
        detail_r,
        files,
        nfo,
        tags,
        logs,
        artifacts,
        artifact_children,
    ) = joined;
    // 只有对外可见的种子才允许进共享缓存（见上方命中判定）
    let cacheable = matches!(&torrent_r, Ok(t) if t.approval_status == 1);
    let torrent = torrent_r?;
    let detail = detail_r?;

    let shared = TorrentAggregateShared {
        _id: id,
        torrent,
        detail,
        files,
        nfo,
        tags,
        logs,
        artifacts,
        artifact_children,
    };
    if cacheable {
        if let Ok(json) = serde_json::to_string(&shared) {
            let _: Result<(), _> = redis::AsyncCommands::set_ex(
                &mut c,
                shared_cache_key(id, reveal),
                json,
                30u64,
            )
            .await;
        }
    }
    Ok(ok(assemble(shared, auth.id, &state, id).await?))
}

/// 共享段（可跨用户复用）：torrent 主行 + detail + files + nfo + tags。
/// detail 里 purchased/is_owner 等 viewer 态字段由个人段覆写。
#[derive(Serialize, serde::Deserialize)]
struct TorrentAggregateShared {
    _id: i64,
    torrent: torrents::TorrentRow,
    detail: torrents::TorrentDetailRow,
    files: Vec<torrents::FileRow>,
    nfo: Option<String>,
    tags: serde_json::Value,
    logs: Vec<torrents::LogRow>,
    artifacts: Vec<serde_json::Value>,
    artifact_children: Vec<serde_json::Value>,
}

/// 个人段：thanks（是否已感谢带 viewer 态）+ comments（点赞 viewer 态，0155）
/// 每用户实时，叠回共享段组成最终响应。
async fn assemble(
    shared: TorrentAggregateShared,
    uid: i64,
    state: &web::Data<std::sync::Arc<AppState>>,
    id: i64,
) -> DomainResult<TorrentAggregate> {
    let (thanks, comments) = tokio::join!(
        torrents::list_thanks(&state.repo.db, id),
        torrents::list_comments_as(&state.repo.db, id, 20, uid),
    );
    // detail 里的 is_owner / purchased 是 viewer 态，而共享缓存存的是「第一个访问者」
    // 的答案——必须按当前用户重算（0285 修过一次，被并行批次覆盖回 `let _ = uid;`，
    // 症状：B 打开付费种看到「已购买/我发布」，付费墙被隐藏）。
    let mut detail = shared.detail;
    let own: Option<(bool, bool)> = sqlx::query_as(
        "SELECT (t.owner_id = $2) AS is_owner, \
                EXISTS(SELECT 1 FROM torrent_purchases p \
                        WHERE p.torrent_id = t.id \
                          AND p.user_id = $2) AS purchased \
         FROM torrents t WHERE t.id = $1",
    )
    .bind(id)
    .bind(uid)
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten();
    if let Some((is_owner, purchased)) = own {
        detail.is_owner = is_owner;
        detail.purchased = purchased;
    }
    Ok(TorrentAggregate {
        torrent: shared.torrent,
        detail,
        files: shared.files,
        thanks: thanks.unwrap_or_default(),
        comments: match comments {
            Ok(c) => c,
            Err(e) => return Err(e),
        },
        nfo: shared.nfo,
        tags: shared.tags,
        logs: shared.logs,
        artifacts: shared.artifacts,
        artifact_children: shared.artifact_children,
    })
}
