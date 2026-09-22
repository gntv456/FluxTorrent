//! 字幕列表与下载（0146：分页信封、真实 size、评分、匿名脱敏、扩展名修正）。
//! 上传链路在 subtitles.rs；元数据治理在 subtitles_meta.rs。

use actix_web::{get, web, Responder};

use super::subtitles_util::subtitle_bad_threshold;
use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::state::AppState;

#[derive(sqlx::FromRow, serde::Serialize)]
struct SubtitleRow {
    id: i64,
    torrent_id: Option<i64>,
    username: Option<String>,
    title: String,
    lang: Option<String>,
    lang_id: Option<i16>,
    downloads: i32,
    created_at: chrono::DateTime<chrono::Utc>,
    /// 文件大小（字节；0146 起真实落库，历史行回填）
    size: i64,
    ext: Option<String>,
    anon: bool,
    rating_sum: i32,
    rating_count: i32,
    /// 上传者 id（anon 行照常返回，前端按本人判定编辑/删除入口）
    user_id: i64,
    verified: bool,
    /// 0148 C0/C6：三态口径——纯 AI（machine && proofreader 空）/
    /// AI+人工校对（machine && proofreader 非空）/ 纯人工
    machine_translated: bool,
    proofreader: Option<String>,
    /// 0149：上传者身份（gold 优先于 certified；NULL = 无）
    cert_tier: Option<String>,
}

/// 字幕列表（包子站 subtitles.php 口径 + 0146）：
/// search/lang/letter/torrent_id 筛选 + page/per_page/sort/order 分页排序。
/// 只出未删且过审（或免审）的行；坏字幕达阈值自动隐藏。
#[get("/subtitles")]
pub(super) async fn subtitle_list(
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<impl Responder> {
    let search = q
        .get("search")
        .map(|s| crate::http::like_pattern(s))
        .unwrap_or_else(|| "%".into());
    let lang = q.get("lang_id").filter(|s| s.as_str() != "0").cloned();
    let letter = q.get("letter").filter(|s| !s.is_empty()).cloned();
    // C0：AI 筛选——ai=only 只看纯 AI；ai=no 只看人工（含 AI+人工校对）
    let ai_filter = match q.get("ai").map(String::as_str) {
        Some("only") => "AND s.machine_translated",
        Some("no") => "AND NOT s.machine_translated",
        _ => "",
    };
    // C1：imdb 合并查询——同 imdb 的其他种子的字幕也算「本片字幕」。
    // 只留 [TT + 7~8 位数字]（与迁移/上传侧提取同口径），防 $-参数化后
    // 仍被拼进 match 排序分支（F1：注入面归零）。
    let imdb: Option<String> = q.get("imdb").and_then(|s| {
        let t = s.trim().to_ascii_uppercase();
        let d = t.strip_prefix("TT").unwrap_or(&t);
        (t.len() == 2 + d.len()
            && (d.len() == 7 || d.len() == 8)
            && d.chars().all(|c| c.is_ascii_digit()))
        .then_some(t)
    });
    let torrent_id: Option<i64> = q
        .get("torrent_id")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|v| *v > 0);
    let page = q
        .get("page")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|v| *v >= 1)
        .unwrap_or(1);
    let per_page = q
        .get("per_page")
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|v| [25, 50, 100].contains(v))
        .unwrap_or(50);
    let order = if q.get("order").map(String::as_str) == Some("asc") {
        "ASC"
    } else {
        "DESC"
    };
    let sort = match q.get("sort").map(String::as_str) {
        Some("downloads") => "s.downloads".to_string(),
        Some("size") => "s.size".to_string(),
        Some("rating") => {
            "(s.rating_sum::float / NULLIF(s.rating_count,0))".to_string()
        }
        // P1-4 匹配分排序（OpenSubtitles 加权收敛版）：torrent 命中 100 >
        // release_name 归一化相等 80 > imdb+语言同片 50 > verified 20 >
        // 下载数兜底。0148 C1 补 IMDB 档（imdb 参数命中时 +50）。
        Some("match") => {
            let sub = sqlx::query_scalar::<_, Option<String>>(
                "SELECT lower(regexp_replace(name, '[^a-zA-Z0-9]', '', 'g')) \
                 FROM torrents WHERE id = $1",
            )
            .bind(torrent_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
            let imdb_hit = match &imdb {
                Some(v) => format!(
                    " WHEN s.imdb_id = '{}' THEN 50",
                    v.replace('\'', "''")
                ),
                None => String::new(),
            };
            let rel_arm = match sub.as_deref() {
                Some(s) if !s.is_empty() => format!(
                    " WHEN s.release_name = '{}' THEN 80",
                    s.replace('\'', "''")
                ),
                _ => String::new(),
            };
            match torrent_id {
                Some(tid) => format!(
                    "(CASE WHEN s.torrent_id = {tid} THEN 100{imdb_hit}\
                     {rel_arm} ELSE 0 END \
                     + CASE WHEN s.verified THEN 20 ELSE 0 END + s.downloads)"
                ),
                None => format!(
                    "(CASE{imdb_hit} ELSE 0 END \
                     + CASE WHEN s.verified THEN 20 ELSE 0 END + s.downloads)"
                ),
            }
        }
        _ => "s.created_at".to_string(),
    };
    // C0/C6：AI 降权附加层——纯 AI（machine 且无人工校对）在所有排序下
    // 都排人工之后；AI+人工校对（ai_proofread）等同人工档
    let ai_demote = "(CASE WHEN s.machine_translated AND \
         COALESCE(s.proofreader, '') = '' THEN 1 ELSE 0 END)";
    let threshold = subtitle_bad_threshold(&state.repo.db).await?;
    // 谓词只拼一份：count 与列表同谓词（A7 验收点）。
    // 0148：$5/$6 同传时为「本种子字幕 ∪ 同片字幕」（种子页 C1 合并口径）：
    //   imdb 未传（$6 NULL）→ 仅 torrent_id 过滤（原有行为不变）
    //   imdb 传了 → torrent_id 命中 OR imdb 命中
    // ai_filter 为格式化拼接（值均为字面量）。
    let predicates = format!(
        "\
         s.deleted_at IS NULL AND s.status = 1 \
         AND (s.bad_reports < $4 OR $4 <= 0) \
         AND s.title ILIKE $1 \
         AND ($2::text IS NULL OR s.lang = $2) \
         AND ($3::text IS NULL OR s.title ILIKE $3 || '%') \
         AND ($6::text IS NULL AND ($5::bigint IS NULL OR s.torrent_id = $5) \
              OR $6::text IS NOT NULL AND ($5::bigint IS NULL OR s.torrent_id \
              = $5 OR s.imdb_id = $6)){ai_filter}"
    );
    let total: i64 = sqlx::query_scalar(&format!(
        "SELECT count(*) FROM subtitles s WHERE {predicates}"
    ))
    .bind(&search)
    .bind(&lang)
    .bind(&letter)
    .bind(threshold)
    .bind(torrent_id)
    .bind(&imdb)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let rows: Vec<SubtitleRow> = sqlx::query_as(&format!(
        "SELECT s.id, s.torrent_id, u.username, s.title, s.lang, \
             s.lang_id, s.downloads, s.created_at, s.size, s.ext, s.anon, \
             s.rating_sum, s.rating_count, s.user_id, s.verified, \
             s.machine_translated, s.proofreader, cert.cert_tier \
             FROM subtitles s LEFT JOIN users u ON u.id = s.user_id \
             LEFT JOIN LATERAL ( \
                 SELECT c.tier AS cert_tier FROM user_subtitle_certs c \
                 WHERE c.user_id = s.user_id AND c.revoked_at IS NULL \
                 ORDER BY CASE c.tier WHEN 'gold' THEN 0 ELSE 1 END LIMIT 1 \
             ) cert ON TRUE \
             WHERE {predicates} ORDER BY {ai_demote} ASC, {sort} {order}, \
             s.id DESC \
             OFFSET $7 LIMIT $8"
    ))
    .bind(&search)
    .bind(&lang)
    .bind(&letter)
    .bind(threshold)
    .bind(torrent_id)
    .bind(&imdb)
    .bind((page - 1) * per_page)
    .bind(per_page)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 匿名上传：列表口径直接脱敏——anon 行不回真实用户名
    let rows: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|mut r| {
            let name = if r.anon { None } else { r.username.take() };
            let rating = if r.rating_count > 0 {
                // 1-10 均分保留一位小数（sum/count 四舍五入到 0.1）
                serde_json::json!(
                    (r.rating_sum as f64 / r.rating_count as f64 * 10.0)
                        .round()
                        / 10.0
                )
            } else {
                serde_json::Value::Null
            };
            serde_json::json!({
                "id": r.id, "torrent_id": r.torrent_id, "username": name,
                "title": r.title, "lang": r.lang, "lang_id": r.lang_id,
                "downloads": r.downloads, "created_at": r.created_at,
                "size": r.size, "ext": r.ext, "rating": rating,
                "rating_count": r.rating_count,
                // 本人判定用（anon 行 username 已脱敏但 id 保留给编辑/删除入口）
                "user_id": r.user_id, "verified": r.verified,
                // 0149：上传者身份徽章（certified / gold）
                "cert_tier": r.cert_tier,
                // 0148 C0/C6：ai 三态（human / ai_proofread / ai）
                "ai_state": if r.machine_translated {
                    if r.proofreader.as_deref().map(str::trim).unwrap_or(
                        "",
                    ).is_empty() {
                        "ai"
                    } else {
                        "ai_proofread"
                    }
                } else {
                    "human"
                },
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "items": rows, "total": total, "page": page, "per_page": per_page,
    })))
}
