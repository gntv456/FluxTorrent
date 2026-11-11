//! 种子当前在线 peer（详情页「做种/下载者」面板）。
//!
//! 数据源：tracker 已把整个 swarm 表 JSON 快照周期性写入 Redis
//! （`flux:tracker:peers`，TTL 1800；见 `apps/tracker/src/main.rs`），
//! 故此处只需「读快照 + 按 info_hash 过滤」，**tracker 侧零改动**。
//!
//! 隐私口径（重要）：给非 staff 的响应一律脱敏——IP 掩掉末段、
//! peer_id 只留前 8 字节（够识别客户端）；staff（class_id >= 90）
//! 才看完整 IP。
//! 用户名按 `/snatches`（NP viewsnatches）既有口径下发给登录成员：
//! 「谁在做种」在私有站本就是公开信息，站点另一张列表早已给出真实
//! 用户名；这里不另造一套隐藏规则，避免同一页两张表口径打架。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

mod model;
#[cfg(test)]
mod tests;

use self::model::*;

/// tracker 写入的 swarm 快照键（写入方见 tracker main.rs，TTL 1800s）
const SNAPSHOT_KEY: &str = "flux:tracker:peers";
/// 外置 peer 存储（FLUX_TRACKER_PEER_STORE=redis）下的权威位置：
/// 每 swarm 一个 Hash，field=peer_id hex、value=Peer JSON。
const SWARM_KEY_PREFIX: &str = "flux:swarm:";

/// 多副本模式下 tracker 不再写单键快照（那正是副本互抹的根源），
/// 面板必须改读外置 Hash，否则永远显示「0 人在线」（审计 10-07 P1-6）。
fn peer_store_external() -> bool {
    std::env::var("FLUX_TRACKER_PEER_STORE").unwrap_or_default() == "redis"
}
/// 单次返回上限（大 swarm 只给最活跃的一批，计数仍按全量）
const MAX_ITEMS: usize = 50;

#[utoipa::path(
    get,
    path = "/api/v1/torrents/{id}/peers",
    tag = "种子",
    description = "当前 peer 列表（含做种者用户名，非 staff 的 IP 已脱敏）。",
    params(
        ("id" = i64, description = "种子 id"),
    ),
    responses(
        (status = 200, description = "ok"),
        (status = 401, description = "会话未登录（code 2001）"),
        (status = 404, description = "种子不存在或不可见（code 1004）"),
    )
)]
#[get("/torrents/{id}/peers")]
pub(crate) async fn torrent_peers(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let id = path.into_inner();
    let auth = require_auth(&req, &state).await?;
    // 与主详情同口径（0286 / P1-1）：未过审种子的 info_hash 与在线 peer 不外泄
    crate::torrents::assert_visible(
        &state.repo.db,
        id,
        (auth.id, auth.class_id >= 90),
    )
    .await?;
    // 双口径（审计 10-07 P1-6）：tracker 的桶键是 announce 侧原始字节 hex
    // （= raw_info_hash），库内 info_hash 是规范化重编码口径。两者不同的种子
    // 旧实现恒显示 0 人在线——版主治理面看不见真实 peer。
    let hashes: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT info_hash, raw_info_hash FROM torrents WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((ih, ih_raw)) = hashes else {
        // NotFound 的载荷是资源 id（errors.rs::NotFound(i64)）
        return Err(DomainError::NotFound(id));
    };

    let mut c = state.redis.clone();
    let (available, peers) = load_peers(&mut c, &ih, ih_raw.as_deref()).await;
    // 「谁在做种」：与 /snatches 同口径，一次批量查补用户名，避免逐行
    // N+1。查询失败不拖垮面板（回落 #id），在线列表本身仍照常返回。
    let user_ids: Vec<i64> = peers.iter().map(|p| p.user_id).collect();
    let names: std::collections::HashMap<i64, String> =
        sqlx::query_as::<_, (i64, String)>(
            "SELECT id, username FROM users WHERE id = ANY($1)",
        )
        .bind(&user_ids)
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();
    let full_ip = auth.class_id >= 90;
    let now = chrono::Utc::now();
    let mut items: Vec<PeerItem> = Vec::new();
    // 实时计数与 tracker 的 announce/scrape 口径对齐（P2-6）：同一账号在一个
    // swarm 里只计一次，否则多开客户端的人把自己显示成 10 个做种者
    let (mut seeders, mut leechers) = (
        std::collections::HashSet::<i64>::new(),
        std::collections::HashSet::<i64>::new(),
    );
    for p in peers {
        let seeder = p.left == 0;
        let (ip, port) = p.endpoint();
        if seeder {
            seeders.insert(p.user_id);
        } else {
            leechers.insert(p.user_id);
        }
        items.push(PeerItem {
            user_id: p.user_id,
            username: display_name(&names, p.user_id),
            peer_id: p.key.peer_id.chars().take(16).collect(),
            client: client_of(&p.key.peer_id),
            ip: mask_ip(ip, full_ip),
            port,
            seeder,
            progress: progress_of(seeder, p.downloaded, p.left),
            uploaded: p.uploaded,
            downloaded: p.downloaded,
            last_seen_secs: now
                .signed_duration_since(p.last_seen)
                .num_seconds()
                .max(0),
            connectable: p.connectable,
            is_self: p.user_id == auth.id,
        });
    }
    // 做种者优先，其次最近活跃
    items.sort_by(|a, b| {
        b.seeder
            .cmp(&a.seeder)
            .then(a.last_seen_secs.cmp(&b.last_seen_secs))
    });
    let total = items.len();
    items.truncate(MAX_ITEMS);

    Ok(ok(serde_json::json!({
        // available 如实反映数据源是否存在：旧版恒 true，前端无法区分
        // 「tracker 挂了」与「真没人」（代码注释本来就说要返回 false）
        "available": available,
        "masked": !full_ip,
        "seeders": seeders.len(),
        "leechers": leechers.len(),
        "total": total,
        "items": items,
    })))
}

/// 取该种子的在线 peer：外置模式读 `flux:swarm:{hash}`，单机模式读周期快照。
/// 返回 (数据源是否存在, peers)。
async fn load_peers(
    c: &mut redis::aio::ConnectionManager,
    ih: &str,
    ih_raw: Option<&str>,
) -> (bool, Vec<SnapPeer>) {
    use redis::AsyncCommands;
    if peer_store_external() {
        let key = format!("{SWARM_KEY_PREFIX}{}", ih_raw.unwrap_or(ih));
        let exists: i64 = redis::cmd("EXISTS")
            .arg(&key)
            .query_async(c)
            .await
            .unwrap_or(0);
        let map: std::collections::HashMap<String, String> =
            redis::AsyncCommands::hgetall(c, key)
                .await
                .unwrap_or_default();
        let peers = map
            .values()
            .filter_map(|v| serde_json::from_str::<SnapPeer>(v).ok())
            .collect();
        return (exists > 0, peers);
    }
    let raw: Option<String> = redis::AsyncCommands::get(c, SNAPSHOT_KEY)
        .await
        .unwrap_or(None);
    let available = raw.is_some();
    // 先按 Value 拆桶、再逐条转 SnapPeer：一行历史脏数据 / 混形态行
    // 不再让整份（线上 7.7MB）快照反序列化归零，面板全空。
    type Snap = Vec<(String, Vec<serde_json::Value>)>;
    let peers = raw
        .and_then(|r| serde_json::from_str::<Snap>(&r).ok())
        .map(|snap| {
            // 双口径（info_hash / raw_info_hash）同时命中时，同一条 peer 会在
            // 两个桶里各出现一次：调用方的 seeders/leechers 计数已按 user_id
            // 去重（「同一账号只计一次，否则多开客户端的人把自己显示成 10 个
            // 做种者」），列表却按 peer 原样输出 ⇒ 同一个人被列两遍。列表的
            // 去重尺是 peer_id（多开两个客户端仍是两条真实记录），不是人。
            let mut seen = std::collections::HashSet::<String>::new();
            snap.into_iter()
                .filter(|(h, _)| h == ih || ih_raw.is_some_and(|r| h == r))
                .flat_map(|(_, ps)| ps)
                .filter_map(|v| serde_json::from_value::<SnapPeer>(v).ok())
                .filter(|p| seen.insert(p.key.peer_id.clone()))
                .collect()
        })
        .unwrap_or_default();
    (available, peers)
}
