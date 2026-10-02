//! 发种（M04）：POST /torrents 主上传链路。
//! 从 publish_http.rs 按域拆出；NFO 解码助手在 ptgen.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{require_auth, AuthUser};
use crate::state::AppState;

use super::ptgen::{
    build_media_info, decode_nfo, UploadForm, NFO_MAX_BYTES, TORRENT_MAX_BYTES,
};

/// 发种结果。Token 化发种（`/open/torrents`）需要把「重复」当**成功**返回
/// （幂等语义：工具重试同一 .torrent 不该报错），而网页发种必须继续报
/// 3004 让用户看到「种子重复」—— 故核心层返回枚举，由两个入口各自翻译。
pub(crate) enum UploadOutcome {
    Created(serde_json::Value),
    Duplicate {
        id: Option<i64>,
        info_hash: String,
        pieces_hash: String,
    },
}

#[post("/torrents")]
pub async fn upload(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    payload: actix_multipart::Multipart,
    form: web::Query<UploadForm>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    match upload_core(&state, &auth, form.into_inner(), payload).await? {
        UploadOutcome::Created(v) => Ok(ok(v)),
        UploadOutcome::Duplicate { .. } => Err(DomainError::TorrentDuplicate),
    }
}

/// Token 化发种入口（0267，`POST /open/torrents`）：与网页上传走**同一条**
/// 核心链路（同一套过审/扣费/权限/幂等规则），差别只有两处：
/// ①鉴权来自 API Token（外部合成 AuthUser）；
/// ②元数据来自原始 query string —— 这样无需把 UploadForm 及其 25 个字段
/// 提升为 crate 可见，避免把 web 表单结构泄漏成对外契约。
pub(crate) async fn upload_token(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &AuthUser,
    query_string: &str,
    payload: actix_multipart::Multipart,
) -> DomainResult<UploadOutcome> {
    let form = web::Query::<UploadForm>::from_query(query_string)
        .map_err(|e| DomainError::Validation(format!("查询参数非法: {e}")))?
        .into_inner();
    upload_core(state, auth, form, payload).await
}

/// 发种核心：网页表单与开放 API 共用。调用方负责鉴权与表单来源。
async fn upload_core(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &AuthUser,
    form: UploadForm,
    mut payload: actix_multipart::Multipart,
) -> DomainResult<UploadOutcome> {
    use actix_web::web::Bytes;
    use futures_util::StreamExt;

    // 发种基础权限（默认配给全体用户 class 1；可用于限制上传资格）
    crate::authz::require_perm(state, auth, crate::authz::perm::TORRENT_UPLOAD)
        .await?;
    let mut file_bytes: Option<Bytes> = None;
    let mut nfo_bytes: Option<Bytes> = None;
    while let Some(item) = payload.next().await {
        let mut field =
            item.map_err(|e| DomainError::Validation(e.to_string()))?;
        match field.name() {
            Some("file") => {
                let mut buf = web::BytesMut::new();
                while let Some(chunk) = field.next().await {
                    buf.extend_from_slice(
                        &chunk.map_err(|e| {
                            DomainError::Validation(e.to_string())
                        })?,
                    );
                    if buf.len() > TORRENT_MAX_BYTES {
                        return Err(DomainError::Validation(
                            ".torrent 超过 4MiB 上限".into(),
                        ));
                    }
                }
                file_bytes = Some(buf.freeze());
            }
            // NFO 文件（NP upload.php nfo 口径）：文本解码后落 torrents.nfo
            Some("nfo") => {
                let mut buf = web::BytesMut::new();
                while let Some(chunk) = field.next().await {
                    buf.extend_from_slice(
                        &chunk.map_err(|e| {
                            DomainError::Validation(e.to_string())
                        })?,
                    );
                    if buf.len() > NFO_MAX_BYTES {
                        return Err(DomainError::Validation(
                            "NFO 超过 1MiB 上限".into(),
                        ));
                    }
                }
                nfo_bytes = Some(buf.freeze());
            }
            _ => {}
        }
    }
    let bytes = file_bytes
        .ok_or(DomainError::Validation("缺少 .torrent 文件".into()))?;

    let parsed = crate::bencode::parse_torrent(&bytes)
        .map_err(DomainError::TorrentInvalid)?;

    // 重复检测（M04：info_hash 唯一）
    let dupe: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE info_hash = $1 OR \
         raw_info_hash = $2)",
    )
    .bind(&parsed.info_hash_hex)
    .bind(&parsed.raw_info_hash_hex)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if dupe {
        // 幂等：把已存在的那一枚的 id 一并回给调用方（Token 发种据此判重）
        let existing: Option<i64> = sqlx::query_scalar(
            "SELECT id FROM torrents WHERE info_hash = $1 OR \
             raw_info_hash = $2 ORDER BY id LIMIT 1",
        )
        .bind(&parsed.info_hash_hex)
        .bind(&parsed.raw_info_hash_hex)
        .fetch_optional(&state.repo.db)
        .await
        .unwrap_or(None);
        return Ok(UploadOutcome::Duplicate {
            id: existing,
            info_hash: parsed.info_hash_hex.clone(),
            pieces_hash: parsed.pieces_hash_hex.clone(),
        });
    }

    let name = form
        .name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or(parsed.name.clone());
    // 封面外链 + IMDb 链接 → media_info（JSONB 键合并；搜索区 4 按 imdb 键命中）
    let media_info: Option<serde_json::Value> = build_media_info(&form);
    // 发布员职务 / 免审核权限 → 发布即通过（torrent.approval.auto）；
    // 第八轮：命中「自动过审」分类同样免审（categories.auto_approve）
    let cat_auto: bool = sqlx::query_scalar(
        "SELECT COALESCE(bool_or(auto_approve), FALSE) FROM categories \
         WHERE id = $1",
    )
    .bind(form.category_id)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    // 0077 被拒禁发（NP upload_deny_approval_deny_count 口径）：累计被拒达阈值直接拦
    let (deny_count, streak): (i32, i32) = sqlx::query_as(
        "SELECT deny_count, approve_streak FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let deny_limit: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE \
         name = 'upload_deny_limit'), 2)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(2);
    if deny_count >= deny_limit {
        return Err(DomainError::Validation(
            "因多次发布被拒，上传资格已暂停；请先通过『联系我们』申诉".into(),
        ));
    }
    // 0077 免审通道（NP offer_skip_approved_count 口径）：连续过审 ≥5 的发布者免审
    let streak_skip = streak >= 5;
    // 0170 等级免审：class ≥ upload_auto_approve_class（缺省 92 论坛版主）
    // 发布即通过；阈值同步约束编辑回退（interact.rs），「版主以上免审核」全链一致
    let auto_class =
        crate::torrents::manage_perm::auto_approve_threshold(&state.repo.db)
            .await;
    let auto_approve = cat_auto
        || streak_skip
        || auth.class_id >= i32::from(auto_class)
        || crate::authz::can(
            state,
            auth,
            crate::authz::perm::TORRENT_APPROVAL_AUTO,
        )
        .await;
    let approval_status: i16 = if auto_approve { 1 } else { 0 };
    // 聚合组（0069）：显式传入的 group_id 必须存在（防悬挂引用）
    if let Some(gid) = form.group_id {
        let g: Option<i64> =
            sqlx::query_scalar("SELECT id FROM torrent_groups WHERE id = $1")
                .bind(gid)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        if g.is_none() {
            return Err(DomainError::Validation("聚合组不存在".into()));
        }
    }
    let nfo_text: Option<String> = nfo_bytes
        .as_deref()
        .map(decode_nfo)
        .filter(|s| !s.trim().is_empty());
    let price = form.price.unwrap_or(0).clamp(0, 1_000_000);
    // 0148 C1：descr/name 里的 IMDB 引用自动提取（tt1234567，大小写不敏感）
    let imdb_id: Option<String> = form
        .descr
        .as_deref()
        .and_then(extract_imdb)
        .or_else(|| extract_imdb(&name));
    // sections 先整体校验再落种子：原先校验在 INSERT 之后、且与写入交织，
    // 报错时种子已入库，重试同一 .torrent 永远撞 TorrentDuplicate
    super::upload_sections::parse_sections(
        &state.repo.db,
        form.sections.as_ref(),
    )
    .await?;
    let insert_res: Result<i64, sqlx::Error> = sqlx::query_scalar(
        "INSERT INTO torrents (info_hash, raw_info_hash, pieces_hash, \
         group_id, name, small_descr, descr, category_id, medium_id, \
         grade_id, edition_id, owner_id, anonymous, size, numfiles, \
         approval_status, media_info, nfo, price, imdb_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, \
         $14, $15, $16, $17, $18, $19, $20) RETURNING id",
    )
    .bind(&parsed.info_hash_hex)
    .bind(&parsed.raw_info_hash_hex)
    .bind(&parsed.pieces_hash_hex)
    .bind(form.group_id)
    .bind(&name)
    .bind(&form.small_descr)
    .bind(&form.descr)
    .bind(form.category_id)
    .bind(form.medium_id)
    .bind(form.grade_id)
    .bind(form.edition_id)
    .bind(auth.id)
    .bind(form.anonymous)
    .bind(parsed.size)
    .bind(parsed.numfiles)
    .bind(approval_status)
    .bind(media_info)
    .bind(nfo_text)
    .bind(price)
    .bind(imdb_id)
    .fetch_one(&state.repo.db)
    .await;
    let id: i64 = match insert_res {
        Ok(v) => v,
        Err(e) => {
            // 并发上传同一 .torrent：EXISTS 检查与 INSERT 之间的窗口由唯一约束兜底，
            // 映射为语义化的重复结果而非裸 500（raw_info_hash 只有普通索引，见 0081）
            if e.to_string().contains("torrents_info_hash_key")
                || e.to_string().contains("duplicate key")
            {
                let existing: Option<i64> = sqlx::query_scalar(
                    "SELECT id FROM torrents WHERE info_hash = $1 \
                     ORDER BY id LIMIT 1",
                )
                .bind(&parsed.info_hash_hex)
                .fetch_optional(&state.repo.db)
                .await
                .unwrap_or(None);
                return Ok(UploadOutcome::Duplicate {
                    id: existing,
                    info_hash: parsed.info_hash_hex.clone(),
                    pieces_hash: parsed.pieces_hash_hex.clone(),
                });
            }
            // 分类/媒介/学段/版本不存在 → 外键违规，是输入问题不是服务器故障
            return Err(crate::errors::db_to_domain(e, "分类/媒介/学段/版本"));
        }
    };

    // 多维属性（第八轮 Section）与标签：外提至 sections_store::store_sections_tags
    super::upload_sections::store_sections_tags(state, &form, auth, id).await?;
    // 新种子进列表：推进列表缓存代际，否则首屏 45s 内看不到刚发的种
    let app: &AppState = state;
    crate::torrent_http::bump_list_cache_gen(app).await;
    // 存原始 .torrent 字节（下载时重新注入 announce，M05）
    sqlx::query("INSERT INTO torrent_files (torrent_id, raw) VALUES ($1, $2)")
        .bind(id)
        .bind(&parsed.raw)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 文件清单/自动促销/推荐位：外提至 upload_files_promo::store_files_promo
    super::upload_files_promo::store_files_promo(
        state, &form, &parsed, id, auth,
    )
    .await?;
    // M28 插件 Hook：发布成功后分发（异步、失败不影响主流程）
    state.plugins.dispatch_upload(state.get_ref(), id, auth.id);

    // 0075 聚合组推荐（未显式指定组时）：
    //   a) pieces_hash 命中已有组 → 直接建议锁定（跨站同源再发布场景）
    //   b) 否则名称相似度（trgm）> 0.4 的组 → 候选列表
    let mut group_suggest: serde_json::Value = serde_json::json!(null);
    if form.group_id.is_none() {
        let lock: Option<i64> = sqlx::query_scalar(
            "SELECT t2.group_id FROM torrents t2 WHERE \
             t2.pieces_hash = $1 AND t2.pieces_hash <> '' AND t2.group_id IS \
             NOT NULL LIMIT 1",
        )
        .bind(&parsed.pieces_hash_hex)
        .fetch_optional(&state.repo.db)
        .await
        .unwrap_or(None);
        if let Some(gid) = lock {
            let gname: String = sqlx::query_scalar(
                "SELECT name FROM torrent_groups WHERE id = $1",
            )
            .bind(gid)
            .fetch_one(&state.repo.db)
            .await
            .unwrap_or_default();
            group_suggest = serde_json::json!({ "locked": true, "group_id": gid, "name": gname });
        } else {
            let cands: Vec<(i64, String)> = sqlx::query_as(
                                "SELECT g.id, \
                 g.name FROM torrent_groups g WHERE similarity(g.name, $1) > 0.4 ORDER BY similarity(g.name, $1) DESC LIMIT 3",
            )
            .bind(&name)
            .fetch_all(&state.repo.db)
            .await
            .unwrap_or_default();
            if !cands.is_empty() {
                group_suggest =
                    serde_json::json!({ "locked": false, "candidates": cands });
            }
        }
    }

    // 0075 免审积分：自动过审的发布连续 +1（被拒路径在 admin 审核处清零）
    if auto_approve {
        let _ = sqlx::query(
            "UPDATE users SET approve_streak = \
         approve_streak + 1 WHERE id = $1",
        )
        .bind(auth.id)
        .execute(&state.repo.db)
        .await;
    }

    Ok(UploadOutcome::Created(serde_json::json!({
        "id": id,
        "approval_status": approval_status,
        "auto_approved": auto_approve,
        "group_suggest": group_suggest,
        "pieces_hash": parsed.pieces_hash_hex,
        "info_hash": parsed.info_hash_hex,
    })))
}

/// descr/名称里的 IMDB id 提取（0148 C1）：tt1234567 / tt12345678，
/// 大小写不敏感；统一大写存储（TT…）与 subtitles.imdb_id / ?imdb= 对齐。
/// manage.rs 编辑 descr 后复用（crate 出口）。
pub(crate) fn extract_imdb_pub(text: &str) -> Option<String> {
    extract_imdb(text)
}

fn extract_imdb(text: &str) -> Option<String> {
    let re = regex::Regex::new(r"(?i)\b(tt[0-9]{7,8})\b").ok()?;
    re.captures(text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_ascii_uppercase())
}

/// 简介首图提取的 crate 出口（manage.rs 编辑 descr 后同步回落 poster 用，
/// 与上传链 build_media_info 同一实现，防两处口径漂移）。
pub(crate) fn first_descr_image_pub(descr: Option<&str>) -> Option<String> {
    super::descr_image::first_descr_image(descr)
}
