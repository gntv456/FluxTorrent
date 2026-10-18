//! seedbox 申报审核（员工侧，0380）。
//!
//! 审核员在这里决定「谁拿得到反作弊放宽」。这是整套设计里**最需要
//! 人工背书**的一环——系统 ASN 自动识别（detected）只给 speed_relax，
//! H&R 豁免与保种倍率必须在这里显式勾选并留名。
//!
//! 【为什么审核权不下放给自动识别】放宽反作弊等于给真实作弊者开门。
//! ASN 抽样天然误判（企业出口、家宽 CDN 都可能命中机房段），一旦
//! 自动识别能直接给 H&R 豁免，误判成本就变成「白嫖做种不计数」。

//!
//! 本文件自身只放「MediaInfo 一致性巡检」一个读口：把判定器算出来的
//! 不一致清单交给审核台与管理组，处置仍由人来做（判定是提示不是闸门）。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::DomainResult;
use crate::state::AppState;

use super::guard::staff;

#[derive(serde::Deserialize)]
struct MiQ {
    #[serde(default = "def_limit")]
    limit: i64,
}
fn def_limit() -> i64 {
    50
}

/// 不一致清单（假 4K / 声明 HDR 实 SDR / 编码不符……）。
///
/// 2026-10-16 上线。此前判定器已写好但全仓零调用点，清单恒为空；
/// 本口是这条链的**读侧出口**，没有它，判定落库后无人可见。
#[get("/admin/mediainfo/mismatches")]
pub(crate) async fn mediainfo_mismatches(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: crate::http::Q<MiQ>,
) -> DomainResult<HttpResponse> {
    let _auth = staff(&req, &state).await?;
    let rows =
        crate::mediainfo_service::list_mismatches(&state, q.limit).await?;
    Ok(ok(serde_json::json!({
        "mismatches": rows,
        "note": "判定是提示不是闸门：不拦截发布、不自动下架，处置由人工判定。",
    })))
}
