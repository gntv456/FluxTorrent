//! M17 字幕区核心链路（上传/列表/下载 + 0146 治理加固）。
//! 元数据 CRUD、评分举报、语言字典在 subtitles_meta.rs；共用辅助/请求体在
//! subtitles_util.rs（300 行门禁按域拆分）。

use actix_web::{post, web, HttpRequest, HttpResponse};

use super::subtitles_upload::{resolve_file, resolve_release};
use super::subtitles_util::{
    subtitle_setting, SubtitleUploadMeta, SubtitleUploadReq,
};
use crate::dto::ok;
use crate::economy_http::earn_spark;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 上传（0146 加固）：体积上限 + 扩展名白名单（按 kind）+ sha 查重防刷火花 +
/// 元数据落库 + 审核态判定。历史直链（http/https）不受体积/白名单校验。
#[post("/subtitles")]
pub(super) async fn subtitle_upload(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SubtitleUploadReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    if body.title.trim().is_empty() {
        return Err(DomainError::Validation("字幕标题不能为空".into()));
    }
    // torrent_id=0 → 独立字幕（外键可空，这是「不关联种子」的正式入口）。
    // 非 0 就必须过统一准入门：旧写法只判 `EXISTS`，于是任意会员都能往
    // **别人待审/被驳回的种**上挂一行字幕（自己在详情页根本看不到它，
    // 下载口又要求 approval_status=1 ⇒ 白做还不报错）。
    // 「不存在」与「存在但不可见」同回 1004，别把两者演成两种表征——
    // 那正是 P-643 刚关掉的那类存在性探针。
    if body.torrent_id > 0 {
        crate::torrents::assert_visible(
            &state.repo.db,
            body.torrent_id,
            (auth.id, auth.class_id >= 90),
        )
        .await?;
    }
    let torrent_id = if body.torrent_id > 0 {
        Some(body.torrent_id)
    } else {
        None
    };
    // 审核开关（0146）：开 → status=0 待审；关 → 发布即生效（NP 口径）
    let moderation =
        subtitle_setting(&state.repo.db, "subtitle_moderation", "0").await?
            == "1";
    let (file_ref, size, ext) =
        resolve_file(&state, &auth, &body, torrent_id).await?;
    let meta = SubtitleUploadMeta::validate(&body)?;
    let release_name =
        resolve_release(&state, torrent_id, meta.release_name.as_deref())
            .await?;
    // lang → lang_id（0146 字典表；旧 lang 字符串同步写，读端双轨过渡）
    let lang_id: Option<i16> = match body.lang.as_deref().map(str::trim) {
        Some(code) if !code.is_empty() && code != "0" => {
            sqlx::query_scalar::<_, i16>(
                "SELECT id FROM subtitle_langs WHERE code = $1",
            )
            .bind(code)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
        }
        _ => None,
    };
    // 0148 C1：挂了种子 → 回填种子 imdb_id（同片合并的行级冗余）
    let imdb_id: Option<String> = match torrent_id {
        Some(tid) => {
            sqlx::query_scalar("SELECT imdb_id FROM torrents WHERE id = $1")
                .bind(tid)
                .fetch_optional(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .flatten()
        }
        None => None,
    };
    let status: i16 = if moderation { 0 } else { 1 };
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO subtitles (torrent_id, user_id, title, lang, lang_id, \
         file_ref, size, ext, fps, machine_translated, hearing_impaired, \
         foreign_parts_only, source, producer, proofreader, author_name, \
         release_name, anon, status, imdb_id) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, \
         $14, $15, $16, $17, $18, $19, $20) RETURNING id",
    )
    .bind(torrent_id)
    .bind(auth.id)
    .bind(body.title.trim())
    .bind(&body.lang)
    .bind(lang_id)
    .bind(&file_ref)
    .bind(size)
    .bind(&ext)
    .bind(meta.fps)
    .bind(meta.machine_translated)
    .bind(meta.hearing_impaired)
    .bind(meta.foreign_parts_only)
    .bind(&meta.source)
    .bind(&meta.producer)
    .bind(&meta.proofreader)
    .bind(&meta.author_name)
    .bind(release_name)
    .bind(meta.anon)
    .bind(status)
    .bind(imdb_id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 发字幕奖励（P-4 接线）——读站点设定 `uploadsubtitle`（行为奖励卡，
    // 缺省 5），站长改数值此前不生效（硬编码 5）。仅附件上传得奖励；
    // 外链填 URL 零成本刷分不得发钱（四轮 A1）。奖励金额随链接与否落库。
    // P-42（本批实测）：幂等键必须锚「文件」而不是「字幕行」——旧键
    // subtitle:{uid}:{id} 每次重传都是新 id，而查重只看 deleted_at IS NULL，
    // 于是「上传→自删→再传」可无限再领（实测流水 2 条 / 10 火花）。
    // 顺带：已发过的这一次不再谎报 reward（响应要与流水一致）。
    let subtitle_reward: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::bigint FROM site_settings \
         WHERE name = 'uploadsubtitle'), 5)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(5);
    let mut reward: i64 = if file_ref.starts_with("attach://") {
        subtitle_reward
    } else {
        0
    };
    if reward > 0 {
        let sha = &file_ref["attach://".len()..];
        let idem = format!("subtitle:file:{}:{}", auth.id, sha);
        let already: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM spark_ledger WHERE \
             idempotency_key = $1)",
        )
        .bind(&idem)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        if already {
            reward = 0;
        } else {
            earn_spark(&state.repo.db, auth.id, reward, "subtitle", &idem)
                .await?;
        }
    }
    announce_upload(&state, &auth, &meta, body.title.trim()).await;
    Ok(ok(
        serde_json::json!({ "id": id, "reward": reward, "status": status }),
    ))
}

/// 播报联动（0407 三渠分立）：全站动态流（site_activity）是缺省承接方
/// ——公告区（announcements）回归管理员官方发布专用，播报不再写入；
/// 站长开 `shoutbox_announce_subtitles` 后同时广播聊天室（传统 NP 气氛）。
/// 失败静默——播报不可用不应阻断上传主链路。
async fn announce_upload(
    state: &web::Data<std::sync::Arc<AppState>>,
    auth: &crate::http::AuthUser,
    meta: &SubtitleUploadMeta,
    title: &str,
) {
    let display = if meta.anon {
        "一位匿名用户".to_string()
    } else if let Some(name) = &meta.author_name {
        name.clone()
    } else {
        sqlx::query_scalar::<_, String>(
            "SELECT username FROM users WHERE id = $1",
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .unwrap_or_else(|_| "用户".into())
    };
    let text = format!("{display} 上传了字幕「{title}」");
    // 全站动态流（缺省路；0405/0406 曾先后落聊天室/公告区，皆因渠道
    // 语义不符回迁——timeline 见迁移 0407 头注释）
    let _ = sqlx::query(
        "INSERT INTO site_activity (kind, text, actor_id) \
         VALUES ('subtitle_upload', $1, $2)",
    )
    .bind(text.clone())
    .bind(auth.id)
    .execute(&state.repo.db)
    .await;
    // 聊天室广播（回开路，缺省关）
    let broadcast: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings \
         WHERE name = 'shoutbox_announce_subtitles'",
    )
    .bind(0)
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten();
    if broadcast.as_deref() == Some("1") {
        let _ = sqlx::query(
            "INSERT INTO shoutbox (user_id, message) VALUES ($1, $2)",
        )
        .bind(auth.id)
        .bind(text)
        .execute(&state.repo.db)
        .await;
    }
}
