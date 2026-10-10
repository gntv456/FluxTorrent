//! 当季新番（0334 anime 专项）：按**播出季度**浏览番剧。
//!
//! 动漫站的门面是「2026 夏番有哪些」——蜜柑 / AnimeBytes / 动漫花园都以
//! 播出季分栏。0320 补的 `season` 是「第几季」（作品内部批次），与「哪一年
//! 哪一季播出」正交；0334 立 `air_season` 维度后，本模块给读口：
//!   · `GET /seasonal`              当季新番（默认 = 词表 sort 最大的季）
//!   · `GET /seasonal?season=2026夏` 指定季
//!
//! 返回结构同时给「季列表」（仅有内容的季，倒序）与「该季种列表」——
//! 前端据此渲染季切换条 + 番剧行，无需二次请求。
//!
//! 零新增实体：air_season 是 select 维度，值与筛选全走 section_dict /
//! torrent_sections 现有链路。

use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[derive(Deserialize)]
struct SeasonalQuery {
    /// 季名（"2026夏"）或 dict id 字符串；缺省 = 当季（sort 最大）。
    #[serde(default)]
    season: Option<String>,
}

/// 当季新番：默认当季，可指定；附有内容的季列表供前台切换。
#[get("/seasonal")]
pub async fn seasonal_anime(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<SeasonalQuery>,
) -> DomainResult<impl Responder> {
    let _auth = require_auth(&req, &state).await?;

    // 有内容的季（dict 定义了 + 至少一枚过审种），按 sort 倒序。
    let seasons: Vec<(i64, String, i64)> = sqlx::query_as(
        "SELECT sd.id, sd.name, count(DISTINCT t.id) AS n \
           FROM section_dict sd \
           JOIN torrent_sections ts ON ts.dict_id = sd.id \
                AND ts.kind = 'air_season' \
           JOIN torrents t ON t.id = ts.torrent_id \
                AND t.approval_status = 1 \
          WHERE sd.kind = 'air_season' \
          GROUP BY sd.id, sd.name, sd.sort \
          HAVING count(DISTINCT t.id) > 0 \
          ORDER BY sd.sort DESC, sd.id DESC",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;

    // 目标季：显式传入（名字或 id）优先；否则当季 = sort 最大的有内容季；
    // 若都无，回落到词表最新项（即便空，也让前端知道当前季名）。
    let want = q.season.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let target: Option<(i64, String)> = match want {
        Some(s) => sqlx::query_as(
            "SELECT id, name FROM section_dict \
              WHERE kind = 'air_season' \
                AND (name = $1 OR id::text = $1) LIMIT 1",
        )
        .bind(s)
        .fetch_optional(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?,
        None => match seasons.first() {
            Some((id, name, _)) => Some((*id, name.clone())),
            None => sqlx::query_as(
                "SELECT id, name FROM section_dict WHERE kind = 'air_season' \
                  ORDER BY sort DESC, id DESC LIMIT 1",
            )
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?,
        },
    };

    let mut items: Vec<serde_json::Value> = Vec::new();
    let mut season_name = String::new();
    if let Some((sid, sname)) = target {
        season_name = sname;
        // 该季的过审种：带话数区间、所属组、字幕组（同番同话多版本并列时
        // 前台按 group_id 折叠）。字幕组是 multiselect → 走 section_dict 反查。
        let rows: Vec<(i64, String, i64, i32, i32, Option<i64>, Option<i64>)> =
            sqlx::query_as(
                "SELECT t.id, t.name, t.size, t.seeders, \
                        t.times_completed, t.group_id, \
                        (SELECT max((ts2.value #>> '{}')::numeric)::bigint \
                           FROM torrent_sections ts2 \
                          WHERE ts2.torrent_id = t.id \
                            AND ts2.kind = 'ep_last') AS ep_last \
                   FROM torrents t \
                   JOIN torrent_sections ts ON ts.torrent_id = t.id \
                        AND ts.kind = 'air_season' \
                  WHERE ts.dict_id = $1 AND t.approval_status = 1 \
                  ORDER BY t.seeders DESC, t.id DESC LIMIT 300",
            )
            .bind(sid)
            .fetch_all(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        // 字幕组批取（0334 性能补全）：一次 ANY 拿全部 tid → 名单，
        // 替换逐行子查询（≤301 条 SQL 的 N+1；与 openapi dims_for 同范式）
        let tids: Vec<i64> = rows.iter().map(|r| r.0).collect();
        let sub_rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT DISTINCT ts3.torrent_id, sd2.name \
               FROM torrent_sections ts3 \
               JOIN section_dict sd2 ON sd2.id = ts3.dict_id \
              WHERE ts3.torrent_id = ANY($1) \
                AND ts3.kind = 'subtitle_group' \
              ORDER BY 2",
        )
        .bind(&tids)
        .fetch_all(&state.repo.db)
        .await
        .unwrap_or_default();
        use std::collections::BTreeMap;
        let mut subs_map: BTreeMap<i64, Vec<String>> = BTreeMap::new();
        for (tid, name) in sub_rows {
            subs_map.entry(tid).or_default().push(name);
        }
        for (tid, name, size, seeders, done, gid, ep) in rows {
            let subs = subs_map.get(&tid).cloned().unwrap_or_default();
            items.push(serde_json::json!({
                "id": tid, "name": name, "size": size,
                "seeders": seeders, "times_completed": done,
                "group_id": gid, "ep_last": ep, "subtitle_groups": subs,
            }));
        }
    }

    Ok(ok(serde_json::json!({
        "season": season_name,
        "seasons": seasons.iter().map(|(id, name, n)| {
            serde_json::json!({ "id": id, "name": name, "torrents": n })
        }).collect::<Vec<_>>(),
        "items": items,
    })))
}
