//! PTGen 元数据抓取 + NFO 解码/HTML 转文本助手。
//! 从 publish_http.rs 按域拆出（upload.rs 也用 decode_nfo）。

use actix_web::{get, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

// ============ 发布 / 下载（M04 / M05） ============

#[derive(Deserialize)]
pub(super) struct UploadForm {
    pub(super) name: Option<String>,
    pub(super) small_descr: Option<String>,
    pub(super) descr: Option<String>,
    pub(super) category_id: i32,
    /// 介质列（0087 起可空，仅为兼容老数据；新数据以 torrent_sections.kind='media' 为准）
    #[serde(default)]
    pub(super) medium_id: Option<i32>,
    pub(super) grade_id: Option<i32>,
    pub(super) edition_id: Option<i32>,
    #[serde(default)]
    pub(super) anonymous: bool,
    /// 海报/封面外链 URL（存 media_info.poster；列表 46px 封面位与首页海报墙共用）
    #[serde(default)]
    pub(super) poster: Option<String>,
    /// 多维属性（第八轮 Section）：kind → section_dict.id
    /// multipart 场景以 JSON 字符串传递：sections={"codec":1,"team":2}
    #[serde(default)]
    pub(super) sections: Option<String>,
    /// 聚合组（0069）：加入既有组（同一资源的多个版本共享元数据），缺省为独立种子
    #[serde(default)]
    pub(super) group_id: Option<i64>,
    /// 标签（NP upload.php tags 口径）：tag_dict.id 数组，multipart 场景以 JSON 字符串传递
    #[serde(default)]
    pub(super) tags: Option<String>,
    /// IMDb 链接（NP imdbpage 口径）：存 media_info.imdb，搜索区 4 已按此键命中
    #[serde(default)]
    pub(super) imdb: Option<String>,
    /// MediaInfo 文本（NP 详情页折叠块口径）：存 media_info.mediainfo，详情页原样展示
    #[serde(default)]
    pub(super) mediainfo: Option<String>,
    /// 付费下载定价（0086）：0 = 免费，≤ 1,000,000；下载者支付，发布者得 (100-税)%
    #[serde(default)]
    pub(super) price: Option<i64>,
    /// 推荐位（0089，NP 挑选 口径）：pos_state 0/1/2 = 不置顶/一级/二级，需管理组
    #[serde(default)]
    pub(super) pos_state: Option<i16>,
    /// 置顶截止时间（ISO 8601；空 = 永久置顶）
    #[serde(default)]
    pub(super) pos_state_until: Option<String>,
    /// 推荐影片（0089）：pick_type 0/1/2 = 普通/推荐/经典，需管理组
    #[serde(default)]
    pub(super) pick_type: Option<i16>,
}

/// CP437 高位区（0x80-0xFF）→ Unicode（DOS 风格 NFO 的事实编码；表由 Python cp437 编解码器生成）
const CP437_HIGH: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{00A0}";

/// NFO 字节解码：合法 UTF-8 直接用，否则按 CP437 逐字节映射（经典场景 NFO 的字符画不丢）
pub(super) fn decode_nfo(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.to_string();
    }
    bytes
        .iter()
        .map(|b| {
            if *b < 0x80 {
                *b as char
            } else {
                CP437_HIGH
                    .chars()
                    .nth((*b - 0x80) as usize)
                    .unwrap_or('\u{FFFD}')
            }
        })
        .collect()
}

#[derive(serde::Deserialize)]
struct PtgenQ {
    url: String,
}

/// PT-Gen 代理（NP ptgen.php 口径）：服务端转发 imdb/douban/bangumi/indienova，
/// 规避浏览器 CORS；返回 HTML 剥离为纯文本，匹配前端 markdown-lite 简介渲染
#[get("/ptgen")]
pub async fn ptgen(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    q: web::Query<PtgenQ>,
) -> DomainResult<HttpResponse> {
    let _auth = require_auth(&req, &state).await?;
    let url = q.url.trim();
    let parsed = url::Url::parse(url)
        .map_err(|_| DomainError::Validation("链接无效".into()))?;
    let host = parsed.host_str().unwrap_or_default().to_lowercase();
    // 站点启用源（0087 metadata_sources）∩ PT-Gen 支持的源：host 后缀映射
    // （默认串与 0221 迁移后的设置值对齐；mediainfo 非 URL 源，不参与此处映射）
    let enabled: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'metadata_sources'), \
         'imdb,douban,bangumi,indienova,mediainfo')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "imdb,douban,bangumi,indienova,mediainfo".into());
    // P2（2026-10-06 安全审计）：ends_with("imdb.com") 会放行 evil-imdb.com；
    // 改为「裸域或 .子域」精确匹配，bangumi 原本就是这个口径
    let domain_ok =
        |h: &str, base: &str| h == base || h.ends_with(&format!(".{base}"));
    let allowed = enabled.contains("imdb") && domain_ok(&host, "imdb.com")
        || enabled.contains("douban") && domain_ok(&host, "douban.com")
        || enabled.contains("bangumi")
            && (domain_ok(&host, "bgm.tv") || domain_ok(&host, "bangumi.tv"))
        || enabled.contains("indienova") && domain_ok(&host, "indienova.com");
    if !allowed {
        return Err(DomainError::Validation(
            "链接无效或该元数据源未在本站启用（imdb / douban / bangumi / indienova）".into(),
        ));
    }
    // 适配器优先（生态商店 M4 收尾）：启用的 metadata 适配器且 host 命中其
    // 白名单 → 沙箱执行；失败自动回退 PT-Gen（站长无感降级，§5.1）
    if let Some((name, descr)) =
        super::super::adapter_http::try_adapter_metadata(&state, url).await
    {
        return Ok(ok(serde_json::json!({
            "name": name, "descr": descr, "via": "adapter",
        })));
    }
    // 上游可配（0284 P0-2）：site_settings.ptgen_upstream 优先（自托管可替换），
    // 缺省回落内置公共实例；空串 = 站长显式禁用——返回可操作文案而非 500
    let upstream: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'ptgen_upstream'), 'https://ptgen.rachpt.dev/api')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "https://ptgen.rachpt.dev/api".into());
    if upstream.trim().is_empty() {
        return Err(DomainError::Validation(
            "一键填充未启用（PT-Gen 服务未配置，请联系站长或在后台填写 \
             ptgen_upstream）"
                .into(),
        ));
    }
    let api = url::Url::parse_with_params(upstream.trim(), &[("url", url)])
        .map_err(|_| DomainError::Validation("链接无效".into()))?;
    let client = reqwest::Client::new();
    let resp = client
        .get(api)
        .timeout(std::time::Duration::from_secs(
            crate::http::OUTBOUND_FETCH_TIMEOUT_SECS,
        ))
        .send()
        .await
        // 网络层失败降级为 Validation（0284 P0-2）：DNS 断/连不上是上游环境的
        // 常态，不该以 500 内部错误的形态出现在用户面前
        .map_err(|e| {
            DomainError::Validation(format!(
                "PT-Gen 服务不可达（{e}），请稍后重试或手动填写简介"
            ))
        })?;
    if !resp.status().is_success() {
        return Err(DomainError::Validation(format!(
            "PT-Gen 上游异常（HTTP {}）",
            resp.status().as_u16()
        )));
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let html = body
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if body.get("success").and_then(|v| v.as_bool()) != Some(true)
        || html.is_empty()
    {
        return Err(DomainError::Validation("PT-Gen 未能解析该链接".into()));
    }
    Ok(ok(serde_json::json!({
        "name": body.get("name").and_then(|v| v.as_str()).unwrap_or(""),
        "descr": html_to_text(html),
    })))
}

/// 简易 HTML → 纯文本（PT-Gen 返回物）：块级标签转行、剥其余标签、解常见实体
fn html_to_text(html: &str) -> String {
    let mut s = html
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n")
        .replace("</div>", "\n")
        .replace("</tr>", "\n")
        .replace("</li>", "\n")
        .replace("<li>", "- ")
        .replace("</td>", "  ")
        .replace("</th>", "  ");
    // 剥离其余标签（PT-Gen 输出为受信源生成的受控 HTML，逐字符状态机即可）
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    s = out;
    for (ent, ch) in [
        ("&nbsp;", " "),
        ("&amp;", "&"),
        ("&lt;", "<"),
        ("&gt;", ">"),
        ("&quot;", "\""),
        ("&#39;", "'"),
    ] {
        s = s.replace(ent, ch);
    }
    // 折叠空行 + 去行尾空白
    let mut lines: Vec<String> = Vec::new();
    for line in s.lines() {
        let t = line.trim_end();
        if t.is_empty() && lines.last().map(String::is_empty).unwrap_or(true) {
            continue;
        }
        lines.push(t.to_string());
    }
    lines.join("\n").trim().to_string()
}

/// .torrent 本体远小于附件（极端多文件大 piece 也在 MiB 级）；nfo 为纯文本。
/// 必须在流式循环内拦截：actix 默认 2MB PayloadConfig 只约束 Json 提取器，不约束 Multipart。

/// 组装 media 信息 JSON（poster/imdb/mediainfo 三可选键；空则 None）。
/// 从 upload 主链路外提（拆分时纯搬移）。
/// 0159 用户反馈：未填封面 URL 时回落「简介里第一张图」（NP/UNIT3D 同口径）。
pub(super) fn build_media_info(form: &UploadForm) -> Option<serde_json::Value> {
    let mut media_obj = serde_json::Map::new();
    let poster = form
        .poster
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map(str::to_string)
        .or_else(|| {
            super::descr_image::first_descr_image(form.descr.as_deref())
        });
    if let Some(u) = poster {
        media_obj.insert("poster".into(), serde_json::json!(u));
    }
    if let Some(i) = form
        .imdb
        .as_deref()
        .map(str::trim)
        .filter(|i| !i.is_empty())
    {
        media_obj.insert("imdb".into(), serde_json::json!(i));
    }
    if let Some(mi) = form
        .mediainfo
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        // 截断防滥用（MediaInfo 全文通常 < 64KB）
        let mi = &mi[..mi.len().min(60_000)];
        media_obj.insert("mediainfo".into(), serde_json::json!(mi));
    }
    if media_obj.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(media_obj))
    }
}
