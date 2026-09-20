//! 发布 / 下载（M04 / M05）+ 聚合组（0069）+ PTGen 元数据抓取。
//! 从 http.rs 机械外移（审查路线图第 4 周「拆上帝文件」）：含 upload（发种）、
//! download（.torrent 动态生成）、PTGen、torrent_groups 聚合组四组端点。

use actix_web::{get, post, web, HttpRequest, HttpResponse};
use serde::Deserialize;

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;
use crate::torrents;

// ============ 发布 / 下载（M04 / M05） ============

#[derive(Deserialize)]
struct UploadForm {
    name: Option<String>,
    small_descr: Option<String>,
    descr: Option<String>,
    category_id: i32,
    /// 介质列（0087 起可空，仅为兼容老数据；新数据以 torrent_sections.kind='media' 为准）
    #[serde(default)]
    medium_id: Option<i32>,
    grade_id: Option<i32>,
    edition_id: Option<i32>,
    #[serde(default)]
    anonymous: bool,
    /// 海报/封面外链 URL（存 media_info.poster；列表 46px 封面位与首页海报墙共用）
    #[serde(default)]
    poster: Option<String>,
    /// 多维属性（第八轮 Section）：kind → section_dict.id
    /// multipart 场景以 JSON 字符串传递：sections={"codec":1,"team":2}
    #[serde(default)]
    sections: Option<String>,
    /// 聚合组（0069）：加入既有组（同一资源的多个版本共享元数据），缺省为独立种子
    #[serde(default)]
    group_id: Option<i64>,
    /// 标签（NP upload.php tags 口径）：tag_dict.id 数组，multipart 场景以 JSON 字符串传递
    #[serde(default)]
    tags: Option<String>,
    /// IMDb 链接（NP imdbpage 口径）：存 media_info.imdb，搜索区 4 已按此键命中
    #[serde(default)]
    imdb: Option<String>,
    /// MediaInfo 文本（NP 详情页折叠块口径）：存 media_info.mediainfo，详情页原样展示
    #[serde(default)]
    mediainfo: Option<String>,
    /// 付费下载定价（0086）：0 = 免费，≤ 1,000,000；下载者支付，发布者得 (100-税)%
    #[serde(default)]
    price: Option<i64>,
    /// 推荐位（0089，NP 挑选 口径）：pos_state 0/1/2 = 不置顶/一级/二级，需管理组
    #[serde(default)]
    pos_state: Option<i16>,
    /// 置顶截止时间（ISO 8601；空 = 永久置顶）
    #[serde(default)]
    pos_state_until: Option<String>,
    /// 推荐影片（0089）：pick_type 0/1/2 = 普通/推荐/经典，需管理组
    #[serde(default)]
    pick_type: Option<i16>,
}

/// CP437 高位区（0x80-0xFF）→ Unicode（DOS 风格 NFO 的事实编码；表由 Python cp437 编解码器生成）
const CP437_HIGH: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{00A0}";

/// NFO 字节解码：合法 UTF-8 直接用，否则按 CP437 逐字节映射（经典场景 NFO 的字符画不丢）
fn decode_nfo(bytes: &[u8]) -> String {
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
    let enabled: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = 'metadata_sources'), 'imdb,douban,bangumi,indienova')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_else(|_| "imdb,douban,bangumi,indienova".into());
    let allowed = enabled.contains("imdb") && host.ends_with("imdb.com")
        || enabled.contains("douban") && host.ends_with("douban.com")
        || enabled.contains("bangumi")
            && (host == "bgm.tv"
                || host.ends_with(".bgm.tv")
                || host == "bangumi.tv"
                || host.ends_with(".bangumi.tv"))
        || enabled.contains("indienova") && host.ends_with("indienova.com");
    if !allowed {
        return Err(DomainError::Validation(
            "链接无效或该元数据源未在本站启用（imdb / douban / bangumi / indienova）".into(),
        ));
    }
    let api = url::Url::parse_with_params(
        "https://ptgen.rachpt.dev/api",
        &[("url", url)],
    )
    .map_err(|_| DomainError::Validation("链接无效".into()))?;
    let client = reqwest::Client::new();
    let resp = client
        .get(api)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
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
const TORRENT_MAX_BYTES: usize = 4 * 1024 * 1024; // 单文件 4MiB
const NFO_MAX_BYTES: usize = 1 * 1024 * 1024; // 单文件 1MiB

/// multipart：file=<.torrent> + 表单字段
#[post("/torrents")]
pub async fn upload(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    mut payload: actix_multipart::Multipart,
    form: web::Query<UploadForm>,
) -> DomainResult<HttpResponse> {
    use actix_web::web::Bytes;
    use futures_util::StreamExt;

    let auth = require_auth(&req, &state).await?;
    // 发种基础权限（默认配给全体用户 class 1；可用于限制上传资格）
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::TORRENT_UPLOAD,
    )
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
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE info_hash = $1 OR raw_info_hash = $2)",
    )
    .bind(&parsed.info_hash_hex)
    .bind(&parsed.raw_info_hash_hex)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if dupe {
        return Err(DomainError::TorrentDuplicate);
    }

    let name = form
        .name
        .clone()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or(parsed.name.clone());
    // 封面外链 + IMDb 链接 → media_info（JSONB 键合并；搜索区 4 按 imdb 键命中）
    let mut media_obj = serde_json::Map::new();
    if let Some(u) = form
        .poster
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
    {
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
    let media_info: Option<serde_json::Value> = if media_obj.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(media_obj))
    };
    // 发布员职务 / 免审核权限 → 发布即通过（torrent.approval.auto）；
    // 第八轮：命中「自动过审」分类同样免审（categories.auto_approve）
    let cat_auto: bool = sqlx::query_scalar(
        "SELECT COALESCE(bool_or(auto_approve), FALSE) FROM categories WHERE id = $1",
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
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE name = 'upload_deny_limit'), 2)",
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
    let auto_approve = cat_auto
        || streak_skip
        || crate::authz::can(
            &state,
            &auth,
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
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO torrents (info_hash, raw_info_hash, pieces_hash, group_id, name, small_descr, descr, category_id, medium_id, grade_id, edition_id, owner_id, anonymous, size, numfiles, approval_status, media_info, nfo, price) \
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19) RETURNING id",
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
    .fetch_one(&state.repo.db)
    .await
    .map_err(|e| {
        // 并发上传同一 .torrent：EXISTS 检查与 INSERT 之间的窗口由唯一约束兜底，
        // 映射为语义化的重复错误而非裸 500（raw_info_hash 只有普通索引，见 0081）
        if e.to_string().contains("torrents_info_hash_key")
            || e.to_string().contains("duplicate key")
        {
            DomainError::TorrentDuplicate
        } else {
            DomainError::Internal(e.into())
        }
    })?;

    // 多维属性（第八轮 Section）：校验 kind 白名单后写 torrent_sections
    if let Some(json) = form
        .sections
        .as_deref()
        .map(str::trim)
        .filter(|j| !j.is_empty())
    {
        let map: std::collections::HashMap<String, i64> =
            serde_json::from_str(json).map_err(|_| {
                DomainError::Validation("sections 需为 JSON 对象".into())
            })?;
        for (kind, dict_id) in &map {
            // 0085/0087：维度可由站方自建（含 media/grades/editions），白名单查 section_kinds
            if !crate::admin_p3_http::is_custom_kind(&state.repo.db, kind).await
            {
                return Err(DomainError::Validation(format!(
                    "未知维度 {kind}"
                )));
            }
            // 字典归属校验：dict_id 必须属于该 kind（防跨维度错挂）
            let ok: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM section_dict WHERE id = $2 AND kind = $1)",
            )
            .bind(kind)
            .bind(dict_id)
            .fetch_one(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            if !ok {
                return Err(DomainError::Validation(format!(
                    "维度 {kind} 的字典项 {dict_id} 不存在"
                )));
            }
            sqlx::query(
                "INSERT INTO torrent_sections (torrent_id, kind, dict_id) VALUES ($1, $2, $3)                  ON CONFLICT (torrent_id, kind) DO UPDATE SET dict_id = EXCLUDED.dict_id",
            )
            .bind(id)
            .bind(kind)
            .bind(dict_id)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }

    // 标签（NP upload.php tags 口径）：发布时直接打标；启用字典校验 + 官方标签仅 staff
    if let Some(json) = form
        .tags
        .as_deref()
        .map(str::trim)
        .filter(|j| !j.is_empty())
    {
        let ids: Vec<i32> = serde_json::from_str(json).map_err(|_| {
            DomainError::Validation("tags 需为 JSON 数组".into())
        })?;
        if ids.len() > 12 {
            return Err(DomainError::Validation("标签最多选择 12 个".into()));
        }
        let is_staff = auth.class_id >= 90;
        for tid in &ids {
            let row: Option<(String, bool)> = sqlx::query_as(
                "SELECT kind, COALESCE(enabled, TRUE) FROM tag_dict \
                 WHERE id = $1 AND scope = 'torrent'",
            )
            .bind(tid)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
            let Some((kind, enabled)) = row else {
                return Err(DomainError::Validation(format!(
                    "标签 {tid} 不存在"
                )));
            };
            if !enabled {
                return Err(DomainError::Validation(format!(
                    "标签 {tid} 已停用"
                )));
            }
            if kind == "official" && !is_staff {
                return Err(DomainError::Forbidden); // 与详情页打标同口径
            }
            sqlx::query(
                "INSERT INTO tags (torrent_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(tid)
            .execute(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }

    // 存原始 .torrent 字节（下载时重新注入 announce，M05）
    sqlx::query("INSERT INTO torrent_files (torrent_id, raw) VALUES ($1, $2)")
        .bind(id)
        .bind(&parsed.raw)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 文件清单入 files 表（修复前从不写入：新种的文件列表/按文件名搜索永远为空）
    for (idx, (path, len)) in parsed.files.iter().enumerate() {
        sqlx::query(
            "INSERT INTO files (torrent_id, file_index, path, size) VALUES ($1, $2, $3, $4)",
        )
        .bind(id)
        .bind(idx as i32)
        .bind(path)
        .bind(len)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }

    // 发种自动促销（0089，NP 促销设置 口径）：管理后台配置默认促销（类型+天数），
    // 发布即自动套用——促销跟随站点，不再由发布者单独设置。
    let auto_kind: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = 'upload_auto_promo_kind'), '')",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or_default();
    let auto_days: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE name = 'upload_auto_promo_days'), 0)",
    )
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(0);
    let auto_kind = auto_kind.trim().to_lowercase();
    if !auto_kind.is_empty()
        && auto_days > 0
        && ["free", "x2", "x2free", "half", "x2half", "p30"]
            .contains(&auto_kind.as_str())
    {
        sqlx::query(
            "INSERT INTO promotions (scope, torrent_id, kind, starts_at, ends_at, source, created_by) \
             VALUES ('torrent', $1, $2::promotion_kind_enum, now(), now() + make_interval(days => $3), \
                     'manual'::promotion_source, $4)",
        )
        .bind(id)
        .bind(&auto_kind)
        .bind(auto_days.clamp(1, 720))
        .bind(auth.id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }

    // 推荐位（0089，NP 挑选 口径）：置顶位置/截止 + 推荐影片，管理组专属；
    // 发布页人人可见，无权限提交会被此处拦截（与参考站服务端强校验同口径）
    if form.pos_state.unwrap_or(0) != 0
        || form.pick_type.unwrap_or(0) != 0
        || form
            .pos_state_until
            .as_deref()
            .map(str::trim)
            .is_some_and(|s| !s.is_empty())
    {
        if auth.class_id < 90 {
            return Err(DomainError::Forbidden); // 置顶/推荐仅管理组
        }
        let pos = form.pos_state.unwrap_or(0);
        if ![0, 1, 2].contains(&pos) {
            return Err(DomainError::Validation("置顶位置取值 0/1/2".into()));
        }
        let pick = form.pick_type.unwrap_or(0);
        if ![0, 1, 2].contains(&pick) {
            return Err(DomainError::Validation("推荐影片取值 0/1/2".into()));
        }
        let until = form
            .pos_state_until
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|_| {
                        DomainError::Validation("置顶截止时间格式无效".into())
                    })
                    .map(|dt| dt.with_timezone(&chrono::Utc))
            })
            .transpose()?;
        sqlx::query(
            "UPDATE torrents SET pos_state = $2, pos_state_until = $3, pick_type = $4, mtime = now() WHERE id = $1",
        )
        .bind(id)
        .bind(pos)
        .bind(until)
        .bind(pick)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }

    state
        .repo
        .audit(Some(auth.id), "torrent_upload", Some(id))
        .await;
    // M28 插件 Hook：发布成功后分发（异步、失败不影响主流程）
    state.plugins.dispatch_upload(&state, id, auth.id);

    // 0075 聚合组推荐（未显式指定组时）：
    //   a) pieces_hash 命中已有组 → 直接建议锁定（跨站同源再发布场景）
    //   b) 否则名称相似度（trgm）> 0.4 的组 → 候选列表
    let mut group_suggest: serde_json::Value = serde_json::json!(null);
    if form.group_id.is_none() {
        let lock: Option<i64> = sqlx::query_scalar(
            "SELECT t2.group_id FROM torrents t2              WHERE t2.pieces_hash = $1 AND t2.pieces_hash <> '' AND t2.group_id IS NOT NULL LIMIT 1",
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
                "SELECT g.id, g.name FROM torrent_groups g                  WHERE similarity(g.name, $1) > 0.4                  ORDER BY similarity(g.name, $1) DESC LIMIT 3",
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
        let _ = sqlx::query("UPDATE users SET approve_streak = approve_streak + 1 WHERE id = $1")
            .bind(auth.id)
            .execute(&state.repo.db)
            .await;
    }

    Ok(ok(serde_json::json!({
        "id": id,
        "approval_status": approval_status,
        "auto_approved": auto_approve,
        "group_suggest": group_suggest,
    })))
}

/// 构建给指定用户的 .torrent 字节（注入本站 announce + passkey + private=1）。
/// 网页下载 / NP 兼容下载（compat_http）/ 临时凭证下载共用；鉴权与下载闸门由调用方先行完成。
/// announce 地址来源：站点设定 announce_url / https_announce_url 优先（设定页可改），
/// PUBLIC_TRACKER_URL 环境变量兜底。配置了 https 时首选加密汇报，http 作 BEP12 回退。
pub async fn build_torrent_bytes(
    state: &web::Data<std::sync::Arc<AppState>>,
    user_id: i64,
    torrent_id: i64,
) -> DomainResult<Vec<u8>> {
    let raw: Option<Vec<u8>> = sqlx::query_scalar(
        "SELECT f.raw FROM torrent_files f \
         JOIN torrents t ON t.id = f.torrent_id \
         WHERE f.torrent_id = $1 AND t.approval_status = 1",
    )
    .bind(torrent_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(raw) = raw else {
        return Err(DomainError::NotFound(torrent_id));
    };
    let user = state
        .repo
        .find_user_by_id(user_id)
        .await?
        .ok_or(DomainError::Unauthorized)?;
    // 下载闸门：与 tracker announce 的 left>0 拦截同口径——被停下载/挂起账号
    // 不应还能提前拿到 .torrent 文件
    let (download_enabled, suspended): (bool, bool) = sqlx::query_as(
        "SELECT download_enabled, suspended FROM users WHERE id = $1",
    )
    .bind(user_id)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    .ok_or(DomainError::Unauthorized)?;
    if suspended {
        return Err(DomainError::Forbidden);
    }
    if !download_enabled {
        return Err(DomainError::Validation(
            "您的下载权限已被暂停，请联系管理组".into(),
        ));
    }
    // info dict 不动 → info_hash 与上传时一致（M05）
    async fn setting(db: &sqlx::PgPool, name: &str) -> Option<String> {
        sqlx::query_scalar::<_, String>(
            "SELECT value FROM site_settings WHERE name = $1",
        )
        .bind(name)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .map(|v| v.trim().trim_end_matches('/').to_string())
        .filter(|v| !v.is_empty())
    }
    let env_host = std::env::var("PUBLIC_TRACKER_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:7070".into())
        .trim_end_matches('/')
        .to_string();
    // 审计修复（P0）：历史默认 announce_url 自带尾部 "/announce"（0034 迁移），拼接后
    // 生成 ".../announce/announce/<passkey>" 双重路径 + 错端口，真实客户端必然 404。
    // 这里剥掉尾部 /announce 双保险（迁移 0080 已同时纠正站点设定值本身）。
    let strip_announce = |v: String| -> String {
        let v = v.trim().trim_end_matches('/').to_string();
        match v.strip_suffix("/announce") {
            Some(s) => s.to_string(),
            None => v,
        }
    };
    let base_http = strip_announce(
        setting(&state.repo.db, "announce_url")
            .await
            .unwrap_or(env_host),
    );
    let announce = match setting(&state.repo.db, "https_announce_url").await {
        Some(https) if https != base_http => {
            format!("{}/announce/{}", strip_announce(https), user.passkey)
        }
        _ => format!("{base_http}/announce/{}", user.passkey),
    };
    // http 回退仅在与首选不同时下发（BEP12 单 tier：失败自动降级，不支持 TLS 的老客户端可用）
    let mut fallbacks = Vec::new();
    if !announce.starts_with(&format!("{base_http}/")) {
        fallbacks.push(format!("{base_http}/announce/{}", user.passkey));
    }
    // UDP tracker（BEP15）：默认不启用。私有站的计费身份靠 passkey 随 URL path 传递——
    // HTTP announce（BEP3）天然支持；UDP 包格式没有 path，标准客户端（libtorrent/
    // qBittorrent）不会附带 passkey，UDP tier 只对本站扩展约定的客户端可用。
    // NexusPHP 系站点全部走 HTTP announce，这也是私有 tracker 的行业惯例。
    // 需要时显式设 TRACKER_UDP_URL=udp://host:port 追加 tier（客户端失败后按 BEP12
    // 降级 HTTP 回退，不再默认推断给所有客户端强加一个必然失败的 UDP tier）。
    let udp_url = std::env::var("TRACKER_UDP_URL")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_default();
    if !udp_url.is_empty() {
        fallbacks.push(format!(
            "{}/{}",
            udp_url.trim_end_matches('/'),
            user.passkey
        ));
    }
    crate::bencode::build_download_torrent(&raw, &announce, &fallbacks)
        .map_err(DomainError::TorrentInvalid)
}

// ============ 聚合组（0069：同一资源多版本，GZ Torrent Group 的教育域映射） ============

#[derive(Deserialize)]
struct GroupAttachReq {
    name: String,
    #[serde(default)]
    descr: Option<String>,
}

/// 把种子挂入聚合组：同名组直接复用（UNIQUE 天然幂等），否则创建新组。
/// 仅发布者本人或 staff（class ≥ 90）可操作。
#[post("/torrents/{id}/group")]
pub async fn group_attach(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
    body: web::Json<GroupAttachReq>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let torrent_id = path.into_inner();
    let name = body.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(DomainError::Validation("组名需 1-100 字".into()));
    }
    let owner: Option<i64> =
        sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
            .bind(torrent_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let Some(owner) = owner else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if owner != auth.id && auth.class_id < 90 {
        return Err(DomainError::Forbidden);
    }
    let gid: i64 = match sqlx::query_scalar::<_, i64>(
        "SELECT id FROM torrent_groups WHERE name = $1",
    )
    .bind(name)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?
    {
        Some(g) => g,
        None => sqlx::query_scalar(
            "INSERT INTO torrent_groups (name, descr, created_by) VALUES ($1, $2, $3) RETURNING id",
        )
        .bind(name)
        .bind(
            body.descr
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty()),
        )
        .bind(auth.id)
        .fetch_one(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?,
    };
    sqlx::query("UPDATE torrents SET group_id = $1 WHERE id = $2")
        .bind(gid)
        .bind(torrent_id)
        .execute(&state.repo.db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "group_id": gid, "name": name })))
}

/// 订阅聚合组（0075：新版本入组并过审时推送）
#[post("/torrents/groups/{group_id}/subscribe")]
pub async fn group_subscribe(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let gid = path.into_inner();
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrent_groups WHERE id = $1)",
    )
    .bind(gid)
    .fetch_one(&state.repo.db)
    .await
    .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(gid));
    }
    sqlx::query(
        "INSERT INTO group_subscriptions (user_id, group_id) VALUES ($1, $2)          ON CONFLICT DO NOTHING",
    )
    .bind(auth.id)
    .bind(gid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "subscribed": gid })))
}

/// 退订聚合组
#[post("/torrents/groups/{group_id}/unsubscribe")]
pub async fn group_unsubscribe(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    let gid = path.into_inner();
    sqlx::query(
        "DELETE FROM group_subscriptions WHERE user_id = $1 AND group_id = $2",
    )
    .bind(auth.id)
    .bind(gid)
    .execute(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(ok(serde_json::json!({ "unsubscribed": gid })))
}

/// 组详情 + 组内全部过审版本（详情页「同组资源」数据源；未入组返回 group=null）
#[get("/torrents/{id}/group")]
pub async fn group_info(
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    let torrent_id = path.into_inner();
    // torrents.group_id 可空：fetch_optional 得 Option<Option<i64>>——外层 None=行不存在，
    // 内层 None=未挂组。旧版直接解一层 Option，group_id 为 NULL 时把内层 None 当
    // 行不存在之外还触发 sqlx「unexpected null」解码错（500）。显式双层解构。
    let group_id: Option<i64> =
        sqlx::query_scalar("SELECT group_id FROM torrents WHERE id = $1")
            .bind(torrent_id)
            .fetch_optional(&state.repo.db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?
            .flatten();
    let Some(gid) = group_id else {
        return Ok(ok(serde_json::json!({ "group": null })));
    };
    let row: Option<(String, Option<String>, Option<i32>)> = sqlx::query_as(
        "SELECT name, descr, category_id FROM torrent_groups WHERE id = $1",
    )
    .bind(gid)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((name, descr, category_id)) = row else {
        return Ok(ok(serde_json::json!({ "group": null })));
    };
    let items: Vec<(i64, String, Option<String>, i64, i32, i32, i32, bool)> = sqlx::query_as(
        "SELECT id, name, small_descr, size, seeders, leechers, times_completed, official_tag \
         FROM torrents WHERE group_id = $1 AND approval_status = 1 ORDER BY id",
    )
    .bind(gid)
    .fetch_all(&state.repo.db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let items: Vec<_> = items
        .into_iter()
        .map(|(id, n, sd, size, s, l, c, official)| {
            serde_json::json!({
                "id": id, "name": n, "small_descr": sd, "size": size,
                "seeders": s, "leechers": l, "times_completed": c,
                "official": official, "current": id == torrent_id,
            })
        })
        .collect();
    Ok(ok(serde_json::json!({
        "group": { "id": gid, "name": name, "descr": descr, "category_id": category_id },
        "items": items,
    })))
}

#[get("/torrents/{id}/download")]
pub async fn download(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    path: web::Path<i64>,
) -> DomainResult<HttpResponse> {
    use actix_web::body::BoxBody;

    let auth = require_auth(&req, &state).await?;
    let torrent_id = path.into_inner();
    // 付费下载（0086）：免费/发布者/已购直接放行，否则扣费（余额不足拦截）
    torrents::charge_for_download(&state.repo.db, auth.id, torrent_id).await?;
    let body = build_torrent_bytes(&state, auth.id, torrent_id).await?;
    let mut resp = HttpResponse::with_body(
        actix_web::http::StatusCode::OK,
        BoxBody::new(body),
    );
    resp.headers_mut().insert(
        actix_web::http::header::CONTENT_TYPE,
        actix_web::http::header::HeaderValue::from_static(
            "application/x-bittorrent",
        ),
    );
    Ok(resp)
}
