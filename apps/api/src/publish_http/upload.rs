//! 发种（M04）：POST /torrents 主上传链路。
//! 从 publish_http.rs 按域拆出；NFO 解码助手在 ptgen.rs。

use actix_web::{post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::{require_auth, AuthUser};
use crate::state::AppState;

use super::descr_image::descr_images;
use super::ptgen::{build_media_info, decode_nfo};
use super::{upload_precheck as precheck, upload_revive};

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
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    match upload_core(&state, &auth, req.query_string(), payload).await? {
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
    upload_core(state, auth, query_string, payload).await
}

/// 发种核心：网页表单与开放 API 共用。调用方负责鉴权与表单来源。
async fn upload_core(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &AuthUser,
    query_string: &str,
    mut payload: actix_multipart::Multipart,
) -> DomainResult<UploadOutcome> {
    // 发种基础权限（默认配给全体用户 class 1；可用于限制上传资格）
    crate::authz::require_perm(state, auth, crate::authz::perm::TORRENT_UPLOAD)
        .await?;
    // 体积上限、multipart 读取与元数据装配都在 upload_body.rs
    let body =
        super::upload_body::read_body(&state.repo.db, &mut payload).await?;
    let form = super::upload_fields::build_form(query_string, body.fields)?;

    let parsed = crate::bencode::parse_torrent(&body.torrent)
        .map_err(DomainError::TorrentInvalid)?;
    // 结构校验（P0-6 实测：无 `pieces`、`pieces` 非 20 倍数、`piece length=0`、
    // 总大小 0、空标题、`..\..\` 路径穿越 六种畸形 .torrent 全部 200 入库；
    // 这类种子在客户端必然校验失败，变成 0 做种死种，工单全回审核员面前）
    crate::bencode::validate_for_upload(&parsed)
        .map_err(DomainError::TorrentInvalid)?;
    // 站外取源键（HTTP seed / DHT 提示）入库这一侧直接拒收；下载侧另有剥离
    crate::bencode::reject_off_tracker_sources(&body.torrent)
        .map_err(DomainError::TorrentInvalid)?;

    // 重复检测（M04：info_hash 唯一）。**同一作者的墓碑（status=3）不算重复**——
    // 走复活路径，否则「删了再发」这条路被唯一索引永久锁死（P0-4 实测）。
    let existing: Option<(i64, Option<i64>, i16)> = sqlx::query_as(
        "SELECT id, owner_id, approval_status FROM torrents \
         WHERE info_hash = $1 OR raw_info_hash = $2 ORDER BY id LIMIT 1",
    )
    .bind(&parsed.info_hash_hex)
    .bind(&parsed.raw_info_hash_hex)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let revive_id: Option<i64> = match existing {
        Some((id, owner, status)) if status == 3 && owner == Some(auth.id) => {
            Some(id)
        }
        Some((id, _, _)) => {
            // 幂等：把已存在的那一枚的 id 一并回给调用方（Token 发种据此判重）
            return Ok(UploadOutcome::Duplicate {
                id: Some(id),
                info_hash: parsed.info_hash_hex.clone(),
                pieces_hash: parsed.pieces_hash_hex.clone(),
            });
        }
        None => None,
    };
    // 异主墓碑（P2-7）：内容仍被软删锁着，直接回「种子重复」等于死路——
    // 上传者不知道该找谁。落一条 staff 待办（reports 队列），站务恢复后
    // 上传者即可正常重发。同表已有 (ref_type, ref_id, reporter_id) 开件
    // 唯一约束，重复撞同一墓碑不会刷屏。
    if let Some((id, owner, status)) = &existing {
        if *status == 3 && *owner != Some(auth.id) {
            let _ = sqlx::query(
                "INSERT INTO reports (reporter_id, ref_type, ref_id, reason) \
                 SELECT $1, 'tombstone_reupload', $2, \
                 '异主墓碑重发：内容被他人删除后同 hash 重发被拦，待站务恢复' \
                 WHERE NOT EXISTS (SELECT 1 FROM reports \
                 WHERE ref_type = 'tombstone_reupload' AND ref_id = $2 \
                   AND status IN (0, 2))",
            )
            .bind(auth.id)
            .bind(id)
            .execute(&state.repo.db)
            .await;
        }
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
    // 0077 被拒禁发（NP upload_deny_approval_deny_count 口径）：累计被拒达阈值直接拦。
    // 审计 2026-10-08 P1-2：旧代码读 `upload_deny_limit`，而设置页（0039 迁移）
    // 种的是 `upload_deny_approval_deny_count` —— 两键互不相认，站长在设置页
    // 改阈值恒不生效（实测改 5 后仍按缺省 2 拦）。统一读设置页的键。
    let (deny_count, streak): (i32, i32) = sqlx::query_as(
        "SELECT deny_count, approve_streak FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let deny_limit: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE \
         name = 'upload_deny_approval_deny_count'), 2)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(2);
    if deny_count >= deny_limit {
        return Err(DomainError::Validation(
            "因多次发布被拒，上传资格已暂停；请先通过『联系我们』申诉".into(),
        ));
    }
    // 0077 免审通道（NP offer_skip_approved_count 口径）：连续**人工过审** ≥5 的发布者免审。
    // P1-8：连击只由审核员累加（review_decide）。原先免审通道自己也 +1，等于
    // 「免审 → 连击涨 → 更免审」的自激，一旦过 5 就事实永久免审。
    let streak_skip = streak >= precheck::STREAK_SKIP_MIN;
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
    let nfo_dec = body.nfo.as_deref().map(decode_nfo);
    let nfo_text: Option<String> = nfo_dec.filter(|s| !s.trim().is_empty());
    // ---- 以下全部在 INSERT 之前（P0-4：过去 tags / 推荐位 / 价格越界都在入库后
    //      才报错，留下待审残种 + 同一 .torrent 永久「种子重复」）----
    precheck::check_lengths(&form)?;
    let price = precheck::check_price(form.price)?;
    precheck::check_tags(&state.repo.db, &form, auth).await?;
    let promo = precheck::check_promo(&form, auth)?;
    // 0148 C1：descr/name 里的 IMDB 引用自动提取（tt1234567，大小写不敏感）
    let imdb_id: Option<String> = form
        .descr
        .as_deref()
        .and_then(extract_imdb)
        .or_else(|| extract_imdb(&name));
    // 简介里的图 → screenshots 列（该列过去全仓无写入，审核队列的「截图数」恒 0）
    let shots = descr_images(form.descr.as_deref());
    precheck::check_quality(&state.repo.db, &form, &parsed, shots.len())
        .await?;
    let rip = super::upload_logcheck::run(&body.logcheck, &body.logs)?;
    // sections 先整体校验再落种子：原先校验在 INSERT 之后、且与写入交织，
    // 报错时种子已入库，重试同一 .torrent 永远撞 TorrentDuplicate
    super::upload_sections::parse_sections(
        &state.repo.db,
        form.sections.as_ref(),
    )
    .await?;
    // 重复发布策略（suggest/block/group）：pieces_hash 命中同内容时的处置
    let dup_group: Option<i64> =
        match precheck::dup_policy(&state.repo.db, &parsed.pieces_hash_hex)
            .await?
        {
            precheck::DupVerdict::AutoGroup(gid) => Some(gid),
            _ => None,
        };
    let group_id = form.group_id.or(dup_group);
    // 复活路径（同一作者删过的同内容）：复用原 id，内容整体覆盖，重回审核流
    let id: i64 = match revive_id {
        Some(old) => {
            upload_revive::revive_tombstone(
                &state.repo.db,
                old,
                &form,
                &name,
                &parsed,
                &media_info,
                &nfo_text,
                price,
                &imdb_id,
                group_id,
                approval_status,
                &shots,
            )
            .await?
        }
        None => {
            let insert_res: Result<i64, sqlx::Error> = sqlx::query_scalar(
                "INSERT INTO torrents (info_hash, raw_info_hash, pieces_hash, \
                 group_id, name, small_descr, descr, category_id, medium_id, \
                 grade_id, edition_id, owner_id, anonymous, size, numfiles, \
                 approval_status, approved_at, media_info, nfo, price, imdb_id, \
                 screenshots) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, \
                 $13, $14, $15, $16, CASE WHEN $16::smallint = 1 THEN now() ELSE NULL END, \
                 $17, $18, $19, $20, \
                 $21::jsonb) RETURNING id",
            )
            .bind(&parsed.info_hash_hex)
            .bind(&parsed.raw_info_hash_hex)
            .bind(&parsed.pieces_hash_hex)
            .bind(group_id)
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
            .bind(serde_json::json!(shots))
            .fetch_one(&state.repo.db)
            .await;
            match insert_res {
                Ok(v) => v,
                Err(e) => {
                    // 并发上传同一 .torrent：判重与 INSERT 之间的窗口由唯一约束兜底，
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
                    return Err(crate::errors::db_to_domain(
                        e,
                        "分类/媒介/学段/版本",
                    ));
                }
            }
        }
    };

    // 多维属性（第八轮 Section）与标签：外提至 sections_store::store_sections_tags
    // （标签已在 INSERT 前按同一判据校验过，这里只会因写库失败而错）
    super::upload_sections::store_sections_tags(state, &form, auth, id).await?;
    // 新种子进列表：推进列表缓存代际，否则首屏 45s 内看不到刚发的种
    let app: &AppState = state;
    crate::torrent_http::bump_list_cache_gen(app).await;
    // 免审种（approval_status=1）即刻在 tracker 白名单内，待审种本就装载；
    // 这里统一 bump 一次，免审种不必等下一个 60s 周期才能做种（P2-2）
    crate::http::bump_guard_ver(state.get_ref()).await;
    // 存原始 .torrent 字节（下载时重新注入 announce，M05）
    sqlx::query("INSERT INTO torrent_files (torrent_id, raw) VALUES ($1, $2)")
        .bind(id)
        .bind(&parsed.raw)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 文件清单/自动促销/推荐位：外提至 upload_files_promo::store_files_promo
    // （推荐位的权限与取值已在 INSERT 前校验，这里只负责写）
    super::upload_files_promo::store_files_promo(
        state, &parsed, id, auth, promo,
    )
    .await?;
    super::upload_logcheck::store(&state.repo.db, id, &rip).await?;
    // M28 插件 Hook：发布成功后分发（异步、失败不影响主流程）
    state.plugins.dispatch_upload(state.get_ref(), id, auth.id);

    // 聚合组推荐（同名候选 / 同源命中）：外提至 upload_suggest.rs
    let group_suggest = super::upload_suggest::group_suggest(
        &state.repo.db,
        &parsed.pieces_hash_hex,
        &name,
        id,
        group_id,
    )
    .await;
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
