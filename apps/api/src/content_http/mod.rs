//! M17 求种/候选/字幕 + M18 课本中心 + M20 排行榜 HTTP 接口。
//! 按域拆分（300 行门禁）：求种 requests.rs、候选 offers.rs、字幕 subtitles.rs、
//! 课本/排行榜/促销 misc.rs；mount_content 留在此。

mod misc;
mod offers;
mod promo;
mod requests;
mod subtitles;
mod subtitles_awards;
mod subtitles_close;
mod subtitles_detail;
mod subtitles_dl;
mod subtitles_flow;
mod subtitles_fulfill;
mod subtitles_govern;
mod subtitles_list;
mod subtitles_meta;
mod subtitles_requests;
mod subtitles_util;

use misc::*;
use offers::*;
use promo::*;
use requests::*;
use subtitles::*;
use subtitles_awards::*;
use subtitles_close::*;
use subtitles_detail::*;
use subtitles_dl::*;
use subtitles_flow::*;
use subtitles_fulfill::*;
use subtitles_govern::*;
use subtitles_list::*;
use subtitles_meta::*;
use subtitles_requests::*;

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
        // M17 字幕（0146 治理加固 + 元数据/评分/求字幕悬赏；全部落
        // /api/v1/subtitles 前缀下由模块网关统一拦截）
        .service(subtitle_upload)
        .service(subtitle_list)
        .service(subtitle_download)
        .service(subtitle_detail)
        .service(subtitle_patch)
        .service(subtitle_delete)
        .service(subtitle_vote)
        .service(subtitle_report)
        .service(subtitle_langs)
        .service(subtitle_request_create)
        .service(subtitle_request_list)
        .service(subtitle_request_contribute)
        .service(subtitle_request_fulfill)
        // 0148 字幕工作流（认领/弃单/交稿/验收/协作确认 + 评选榜）
        .service(subtitle_request_claim)
        .service(subtitle_request_abandon)
        .service(subtitle_request_deliver)
        .service(subtitle_request_accept)
        .service(subtitle_request_crew_accept)
        .service(subtitle_awards_board)
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

/// 评选候选生成（admin 手动补跑与 worker 共用；build_candidates 的 crate 出口）
pub async fn subtitles_awards_build(
    db: &sqlx::PgPool,
    period: &str,
) -> anyhow::Result<usize> {
    subtitles_awards::build_candidates(db, period).await
}
