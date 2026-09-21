//! me 站点设置键值（M01）：GET/PUT /me/settings。
//! 从 auth_http.rs 按域拆出。

use actix_web::{get, put, web, HttpRequest, Responder};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

#[get("/me/settings")]
pub async fn me_settings_get(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let s: UserSettings = sqlx::query_as(
        "SELECT parked, accept_pm, delete_pm, save_pm, comment_pm, notify_topic_reply, notify_hr, \
         gender, country, download_speed, upload_speed, isp, info, avatar_url, browsecat, \
         stylesheet, fontsize, site_language, pm_per_page, show_description, show_imdb, \
         show_comment, show_ad, time_type, torrents_per_page, incl_dead, sp_state, \
         incl_bookmarked, tooltip, append_sticky, append_new, append_promotion, append_picked, \
         small_descr, dl_icon, bm_icon, show_com_num, show_last_com, topics_per_page, \
         posts_per_page, view_avatars, view_signatures, tt_last_post, click_topic, signature, privacy \
         FROM users WHERE id = $1",
    )
    .bind(auth.id)
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(s))
}

/// 逐字段 COALESCE 更新（与 NexusPHP usercp save 一致：只改提交的字段）
#[put("/me/settings")]
pub async fn me_settings_put(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<SettingsUpdate>,
) -> DomainResult<impl Responder> {
    let auth = require_auth(&req, &state).await?;
    let b = body.into_inner();
    // 枚举合法性（后端权威，§8.1）
    for (v, allowed) in [
        (&b.accept_pm, &["yes", "friends", "no"][..]),
        (&b.fontsize, &["small", "medium", "large"][..]),
        (&b.time_type, &["timeadded", "timealive"][..]),
        (&b.tooltip, &["minorimdb", "medianimdb", "off"][..]),
        (
            &b.append_promotion,
            &["highlight", "word", "icon", "off"][..],
        ),
        (&b.show_last_com, &["yes", "no"][..]),
        (&b.click_topic, &["firstpage", "lastpage"][..]),
        (&b.privacy, &["normal", "low", "strong"][..]),
    ] {
        if let Some(v) = v {
            if !allowed.contains(&v.as_str()) {
                return Err(DomainError::Validation("非法的设定值".into()));
            }
        }
    }
    for n in [
        b.gender.map(|v| v as i32),
        b.country,
        b.download_speed,
        b.upload_speed,
        b.isp,
        b.pm_per_page,
        b.torrents_per_page,
        b.incl_dead,
        b.sp_state,
        b.incl_bookmarked,
        b.topics_per_page,
        b.posts_per_page,
    ]
    .into_iter()
    .flatten()
    {
        if !(-1..=200).contains(&n) {
            return Err(DomainError::Validation("数值超出范围".into()));
        }
    }
    let updated = sqlx::query(
        "UPDATE users SET \
         parked = COALESCE($2, parked), accept_pm = COALESCE($3, accept_pm), \
         delete_pm = COALESCE($4, delete_pm), save_pm = COALESCE($5, save_pm), \
         comment_pm = COALESCE($6, comment_pm), notify_topic_reply = COALESCE($7, notify_topic_reply), \
         notify_hr = COALESCE($8, notify_hr), gender = COALESCE($9, gender), \
         country = COALESCE($10, country), download_speed = COALESCE($11, download_speed), \
         upload_speed = COALESCE($12, upload_speed), isp = COALESCE($13, isp), \
         info = COALESCE($14, info), avatar_url = COALESCE($15, avatar_url), \
         browsecat = COALESCE($16, browsecat), stylesheet = COALESCE($17, stylesheet), \
         fontsize = COALESCE($18, fontsize), site_language = COALESCE($19, site_language), \
         pm_per_page = COALESCE($20, pm_per_page), show_description = COALESCE($21, show_description), \
         show_imdb = COALESCE($22, show_imdb), show_comment = COALESCE($23, show_comment), \
         show_ad = COALESCE($24, show_ad), time_type = COALESCE($25, time_type), \
         torrents_per_page = COALESCE($26, torrents_per_page), incl_dead = COALESCE($27, incl_dead), \
         sp_state = COALESCE($28, sp_state), incl_bookmarked = COALESCE($29, incl_bookmarked), \
         tooltip = COALESCE($30, tooltip), append_sticky = COALESCE($31, append_sticky), \
         append_new = COALESCE($32, append_new), append_promotion = COALESCE($33, append_promotion), \
         append_picked = COALESCE($34, append_picked), small_descr = COALESCE($35, small_descr), \
         dl_icon = COALESCE($36, dl_icon), bm_icon = COALESCE($37, bm_icon), \
         show_com_num = COALESCE($38, show_com_num), show_last_com = COALESCE($39, show_last_com), \
         topics_per_page = COALESCE($40, topics_per_page), posts_per_page = COALESCE($41, posts_per_page), \
         view_avatars = COALESCE($42, view_avatars), view_signatures = COALESCE($43, view_signatures), \
         tt_last_post = COALESCE($44, tt_last_post), click_topic = COALESCE($45, click_topic), \
         signature = COALESCE($46, signature), privacy = COALESCE($47, privacy) \
         WHERE id = $1",
    )
    .bind(auth.id)
    .bind(b.parked).bind(b.accept_pm).bind(b.delete_pm).bind(b.save_pm)
    .bind(b.comment_pm).bind(b.notify_topic_reply).bind(b.notify_hr)
    .bind(b.gender).bind(b.country).bind(b.download_speed).bind(b.upload_speed)
    .bind(b.isp).bind(b.info).bind(b.avatar_url).bind(b.browsecat)
    .bind(b.stylesheet).bind(b.fontsize).bind(b.site_language).bind(b.pm_per_page)
    .bind(b.show_description).bind(b.show_imdb).bind(b.show_comment).bind(b.show_ad)
    .bind(b.time_type).bind(b.torrents_per_page).bind(b.incl_dead).bind(b.sp_state)
    .bind(b.incl_bookmarked).bind(b.tooltip).bind(b.append_sticky).bind(b.append_new)
    .bind(b.append_promotion).bind(b.append_picked).bind(b.small_descr).bind(b.dl_icon)
    .bind(b.bm_icon).bind(b.show_com_num).bind(b.show_last_com)
    .bind(b.topics_per_page).bind(b.posts_per_page).bind(b.view_avatars).bind(b.view_signatures)
    .bind(b.tt_last_post).bind(b.click_topic).bind(b.signature).bind(b.privacy)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if updated.rows_affected() == 0 {
        return Err(DomainError::Unauthorized);
    }
    state
        .repo
        .audit(Some(auth.id), "usercp_settings_save", Some(auth.id))
        .await;
    Ok(ok(serde_json::json!({ "ok": true })))
}

#[derive(serde::Deserialize, Default)]
#[serde(default)]
pub(super) struct SettingsUpdate {
    pub(super) parked: Option<bool>,
    pub(super) accept_pm: Option<String>,
    pub(super) delete_pm: Option<bool>,
    pub(super) save_pm: Option<bool>,
    pub(super) comment_pm: Option<bool>,
    pub(super) notify_topic_reply: Option<bool>,
    pub(super) notify_hr: Option<bool>,
    pub(super) gender: Option<i16>,
    pub(super) country: Option<i32>,
    pub(super) download_speed: Option<i32>,
    pub(super) upload_speed: Option<i32>,
    pub(super) isp: Option<i32>,
    pub(super) info: Option<String>,
    pub(super) avatar_url: Option<String>,
    pub(super) browsecat: Option<String>,
    pub(super) stylesheet: Option<String>,
    pub(super) fontsize: Option<String>,
    pub(super) site_language: Option<String>,
    pub(super) pm_per_page: Option<i32>,
    pub(super) show_description: Option<bool>,
    pub(super) show_imdb: Option<bool>,
    pub(super) show_comment: Option<bool>,
    pub(super) show_ad: Option<bool>,
    pub(super) time_type: Option<String>,
    pub(super) torrents_per_page: Option<i32>,
    pub(super) incl_dead: Option<i32>,
    pub(super) sp_state: Option<i32>,
    pub(super) incl_bookmarked: Option<i32>,
    pub(super) tooltip: Option<String>,
    pub(super) append_sticky: Option<bool>,
    pub(super) append_new: Option<bool>,
    pub(super) append_promotion: Option<String>,
    pub(super) append_picked: Option<bool>,
    pub(super) small_descr: Option<bool>,
    pub(super) dl_icon: Option<bool>,
    pub(super) bm_icon: Option<bool>,
    pub(super) show_com_num: Option<bool>,
    pub(super) show_last_com: Option<String>,
    pub(super) topics_per_page: Option<i32>,
    pub(super) posts_per_page: Option<i32>,
    pub(super) view_avatars: Option<bool>,
    pub(super) view_signatures: Option<bool>,
    pub(super) tt_last_post: Option<bool>,
    pub(super) click_topic: Option<String>,
    pub(super) signature: Option<String>,
    pub(super) privacy: Option<String>,
}

#[derive(sqlx::FromRow, serde::Serialize)]
pub(super) struct UserSettings {
    pub(super) parked: bool,
    pub(super) accept_pm: String,
    pub(super) delete_pm: bool,
    pub(super) save_pm: bool,
    pub(super) comment_pm: bool,
    pub(super) notify_topic_reply: bool,
    pub(super) notify_hr: bool,
    pub(super) gender: i16,
    pub(super) country: i32,
    pub(super) download_speed: i32,
    pub(super) upload_speed: i32,
    pub(super) isp: i32,
    pub(super) info: Option<String>,
    pub(super) avatar_url: Option<String>,
    // tracker
    pub(super) browsecat: Option<String>,
    pub(super) stylesheet: String,
    pub(super) fontsize: String,
    pub(super) site_language: String,
    pub(super) pm_per_page: i32,
    pub(super) show_description: bool,
    pub(super) show_imdb: bool,
    pub(super) show_comment: bool,
    pub(super) show_ad: bool,
    pub(super) time_type: String,
    pub(super) torrents_per_page: i32,
    pub(super) incl_dead: i32,
    pub(super) sp_state: i32,
    pub(super) incl_bookmarked: i32,
    pub(super) tooltip: String,
    pub(super) append_sticky: bool,
    pub(super) append_new: bool,
    pub(super) append_promotion: String,
    pub(super) append_picked: bool,
    pub(super) small_descr: bool,
    pub(super) dl_icon: bool,
    pub(super) bm_icon: bool,
    pub(super) show_com_num: bool,
    pub(super) show_last_com: String,
    // forum
    pub(super) topics_per_page: i32,
    pub(super) posts_per_page: i32,
    pub(super) view_avatars: bool,
    pub(super) view_signatures: bool,
    pub(super) tt_last_post: bool,
    pub(super) click_topic: String,
    pub(super) signature: Option<String>,
    // security（只读展示，改密走独立接口）
    pub(super) privacy: String,
}
