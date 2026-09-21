//! 产出-回收对账（0077）+ Torznab 出口。
//! 从 economy_http.rs 按域拆出。

use actix_web::{get, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 经济路由（挂载进主 /api/v1 scope，单 scope 避免遮蔽）

// ============ 产出-回收对账（0077，v3 §27-22）+ Torznab 出口 ============

/// 火花产出/回收月度对账（staff）：通胀监控数据底座（v_spark_flow_monthly）
#[get("/admin/spark-flow")]
async fn spark_flow_report(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_VIEW,
    )
    .await?;
    let rows: Vec<(String, i64, i64, i64, i64)> = sqlx::query_as(
        "SELECT month, minted::bigint, burned::bigint, net::bigint, \
         entries FROM v_spark_flow_monthly LIMIT 24",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(rows))
}

/// Torznab caps 端点（0077，cross-seed/Prowlarr 生态入口第一步）。
/// 0079：caps 补真实分类映射 + search 端点落地（映射到既有 torrents 搜索）。
#[get("/torznab")]
async fn torznab_caps() -> HttpResponse {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<torznab:search xmlns:torznab="http://torznab.com/schemas/2015/feed">
  <server version="1.0" title="FluxTorrent" url="/api/v1/torznab" />
  <limits max="100" default="50" />
  <categories>
    <category id="8000" name="Other" />
    <category id="8001" name="Other/Education" />
  </categories>
  <search-fields>
    <field name="q" type="text" />
  </search-fields>
</torznab:search>"#;
    HttpResponse::Ok().content_type("application/xml").body(xml)
}

/// Torznab search（0079）：q=关键字 → 复用 TorrentFilter 的 trgm 搜索，atom 输出。
/// 鉴权与 compat 层同源：`Authorization: Token <api_token>`（开放 API Token）。
/// enclosure 指向 download.php（passkey 形状），Prowlarr/cross-seed 拿链接后带 passkey 拉取。
#[get("/torznab/search")]
async fn torznab_search(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<std::collections::HashMap<String, String>>,
) -> DomainResult<HttpResponse> {
    // 审计修复（P2）：item link/enclosure 需绝对地址，Prowlarr 等才能直接请求。
    // PUBLIC_API_URL 未配置时按请求 Host 拼。
    let api_base = std::env::var("PUBLIC_API_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| {
            req.headers()
                .get("host")
                .and_then(|v| v.to_str().ok())
                .filter(|h| !h.is_empty())
                .map(|h| format!("http://{h}"))
                .unwrap_or_else(|| "http://127.0.0.1:8080".into())
        });
    let auth_uid = {
        // 审计修复（P1）：require_token 现返回 token 所属 user —— enclosure/link 的
        // passkey 占位符替换为该用户真实 passkey，否则 Prowlarr 拿到 PASSKEY 字面量必 401。
        let (uid, _) = crate::openapi_http::require_token(&req, &state).await?;
        let passkey: String =
            sqlx::query_scalar("SELECT passkey FROM users WHERE id = $1")
                .bind(uid)
                .fetch_one(&state.repo.db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
        passkey
    };
    let keyword = q.get("q").cloned().unwrap_or_default();
    let limit: usize = q
        .get("limit")
        .and_then(|v| v.parse().ok())
        .unwrap_or(50)
        .clamp(1, 100);
    let offset: i64 = q
        .get("offset")
        .and_then(|v| v.parse().ok())
        .unwrap_or(0)
        .max(0);
    let filter = crate::torrents::TorrentFilter {
        search: (!keyword.trim().is_empty())
            .then(|| keyword.trim().to_string()),
        ..Default::default()
    };
    // 翻页修复：list_torrents 内部把 limit clamp 到 50，offset≥50 时
    // 「取前 50 再 skip(50)」恒空。改为 offset ≤ 200 时按 offset+limit 原值直查
    // （绕过 clamp 的私有上限：传入的 limit 已在本端点 clamp(1,100)），
    // 更深翻页按 Torznab 惯例拒绝（Prowlarr 实际只翻到 1000）。
    let fetch_n = offset + limit as i64;
    if fetch_n > 1000 {
        return Err(DomainError::Validation(
            "offset+limit 不得超过 1000".into(),
        ));
    }
    let page = crate::torrents::list_torrents_noclamp(
        &state.repo.db,
        &filter,
        None,
        fetch_n,
    )
    .await?;
    let items: Vec<_> = page
        .items
        .into_iter()
        .skip(offset as usize)
        .take(limit)
        .map(|t| {
            let pub_date = t.created_at.to_rfc3339();
            // 促销标记（刷流时效性，NP 官方插件口径）：Torznab 标签 + 标题前缀双通道——
            // Prowlarr 按标签建索引，用户按标题肉眼/过滤规则识别。缺标记 = 免费信息断传。
            let promo_tag = match t.promotion.as_deref() {
                Some("free") => "[Free] ",
                Some("x2free") => "[2xFree] ",
                Some("half") => "[50%] ",
                Some("x2half") => "[2x50%] ",
                Some("x2") => "[2x] ",
                Some("p30") => "[30%] ",
                _ => "",
            };
            let torznab_tags = match t.promotion.as_deref() {
                Some(p @ ("free" | "x2free" | "half" | "x2half" | "x2" | "p30")) => {
                    format!("    <torznab:attr name=\"tags\" value=\"free,{p}\" />\n")
                }
                _ => String::new(),
            };
            format!(
                concat!(
                    "  <item>\n",
                    "    <title>{}{}</title>\n",
                    "    <guid isPermaLink=\"false\">torrent-{}</guid>\n",
                    "    <link>{}/api/v1/compat/nexusphp/download.php?id={}</link>\n",
                    "    <enclosure url=\"{}/api/v1/compat/nexusphp/download.php?id={}&amp;passkey={}\" type=\"application/x-bittorrent\" length=\"{}\" />\n",
                    "    <pubDate>{}</pubDate>\n",
                    "    <size>{}</size>\n",
                    "    <seeders>{}</seeders>\n",
                    "    <peers>{}</peers>\n",
                    "    <category id=\"8000\" name=\"Other\" />\n",
                    "{}",
                    "  </item>\n"
                ),
                promo_tag,
                xml_escape(&t.name),
                t.id,
                &api_base,
                t.id,
                &api_base,
                t.id,
                xml_escape(&auth_uid),
                t.size,
                pub_date,
                t.size,
                t.seeders,
                t.seeders + t.leechers,
                torznab_tags,
            )
        })
        .collect();
    let xml = format!(
        concat!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n",
            "<rss version=\"2.0\" xmlns:torznab=\"http://torznab.com/schemas/2015/feed\">\n",
            "<channel>\n",
            "  <title>FluxTorrent</title>\n",
            "  <description>FluxTorrent Torznab feed</description>\n",
            "{}",
            "</channel>\n",
            "</rss>\n"
        ),
        items.join("")
    );
    Ok(HttpResponse::Ok().content_type("application/xml").body(xml))
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
