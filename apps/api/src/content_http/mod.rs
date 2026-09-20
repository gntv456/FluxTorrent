//! M17 求种/候选/字幕 + M18 课本中心 + M20 排行榜 HTTP 接口。
//! 按域拆分（300 行门禁）：求种 requests.rs、候选 offers.rs、字幕 subtitles.rs、
//! 课本/排行榜/促销 misc.rs；mount_content 留在此。

mod misc;
mod offers;
mod promo;
mod requests;
mod subtitles;

use misc::*;
use offers::*;
use promo::*;
use requests::*;
use subtitles::*;

pub fn mount_content(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        // M17 求种
        .service(request_create)
        .service(request_list)
        .service(request_fulfill)
        // M17 候选
        .service(offer_create)
        .service(offer_list)
        .service(offer_vote)
        .service(offer_promote)
        // M17 字幕
        .service(subtitle_upload)
        .service(subtitle_list)
        .service(subtitle_download)
        // M18 课本
        .service(textbook_list)
        .service(textbook_link)
        // M20 排行榜
        .service(top_boards)
        // 0101 用户自购置顶/限时免费
        .service(promo_plans)
        .service(promo_buy)
}

/// 模块开关读取：module_{name} = 'no' 时模块关闭（site_type_packs 只是初始快照，
/// 运行时权威在 site_settings；此前后端不设防，仅前端隐藏导航，直连 URL 仍全功能可用）。
pub(super) async fn module_disabled(db: &sqlx::PgPool, name: &str) -> bool {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = $1",
    )
    .bind(format!("module_{name}"))
    .fetch_optional(db)
    .await
    .ok()
    .flatten()
    .map(|v| v == "no")
    .unwrap_or(false)
}
