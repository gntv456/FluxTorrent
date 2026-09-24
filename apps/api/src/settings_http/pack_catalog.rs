//! 内容包目录（生态商店 M2，策划案 §7）：内置目录 + 远程索引合并。
//!
//! 内置目录：site_type_packs 的 categories/sections **动态生成** taxonomy
//! 包（不物化，0145 口径）。离线可用，是「货架不为空」的底线（§7.3 自营铺货）。
//! 远程索引：marketplace_index_url 可配；拉取失败静默降级（degraded 标记 +
//! 前端横幅提示），商店永不阻塞站点（不变式 4）。远程条目带 source=remote，
//! 安装 = 服务端按索引 url 拉包文件后走 M1 同一条导入路径。

use actix_web::{get, post, web, HttpRequest, HttpResponse};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::http::require_auth;
use crate::state::AppState;

/// 目录条目（前端卡片）
#[derive(serde::Serialize)]
pub(crate) struct CatalogItem {
    pack_id: String,
    kind: String,
    name: String,
    description: String,
    version: String,
    /// builtin | remote
    source: &'static str,
    /// 内置目录条目的站型 code（前端「适配当前站型」排序用）
    site_type: Option<String>,
    /// 远程条目的包文件直链（安装时服务端拉取）
    url: Option<String>,
}

/// 远程索引形状：{ "format": "fluxtorrent.marketplace", "version": 1,
///   "items": [ { pack_id, kind, name, description?, version?, url } ] }
const INDEX_FORMAT: &str = "fluxtorrent.marketplace";

fn internal<E: Into<anyhow::Error>>(e: E) -> DomainError {
    DomainError::Internal(e.into())
}

/// 站型包行（内置目录数据源）
type PackRow = (
    String,
    String,
    Option<String>,
    serde_json::Value,
    Option<serde_json::Value>,
);

/// 内置目录：站型包 → taxonomy 条目（分类+维度动态拼装，不落库）
async fn builtin_catalog(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<Vec<CatalogItem>> {
    // 首个站型作名义 version（目录每次随核心发版更新）
    let rows: Vec<PackRow> = sqlx::query_as(
        "SELECT code, name, description, categories, sections \
         FROM site_type_packs ORDER BY sort",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    Ok(rows
        .into_iter()
        .map(|(code, name, descr, cats, sections)| {
            let n_cats = cats.as_array().map(|a| a.len()).unwrap_or(0);
            let n_kinds = sections
                .as_ref()
                .and_then(|s| s.pointer("/kinds"))
                .and_then(|k| k.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            CatalogItem {
                pack_id: format!("builtin.taxonomy.{code}"),
                kind: "taxonomy".into(),
                name: format!("{name}·分类学"),
                description: format!(
                    "{}（{} 分类 / {} 维度）",
                    descr.unwrap_or_default(),
                    n_cats,
                    n_kinds
                ),
                version: "1".into(),
                source: "builtin",
                site_type: Some(code),
                url: None,
            }
        })
        .collect())
}

/// 远程索引拉取：失败返回 None（降级为仅内置），错误细节进日志
async fn remote_catalog(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> Option<(Vec<CatalogItem>, String)> {
    let url: String = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'marketplace_index_url'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .flatten()
    .map(|v: String| v.trim().to_string())
    .filter(|v| !v.is_empty())?;
    // R11 加固：索引源仅允许 https（防 http 明文/内网探测面）
    if !url.starts_with("https://") {
        tracing::warn!("商店远程索引非 https，拒绝拉取（降级为仅内置）");
        return None;
    }
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(8))
        .send()
        .await
        .map_err(|e| {
            tracing::warn!(?e, "商店远程索引拉取失败（降级为仅内置）");
        })
        .ok()?;
    if !resp.status().is_success() {
        tracing::warn!(
            status = resp.status().as_u16(),
            "商店远程索引非 2xx（降级为仅内置）"
        );
        return None;
    }
    // R11 加固：索引大小上限 2MiB（原实现全量入内存，超大响应可打内存）
    if resp.content_length().unwrap_or(0) > 2 * 1024 * 1024 {
        tracing::warn!("商店远程索引超过 2MiB 上限（降级为仅内置）");
        return None;
    }
    let body: serde_json::Value = resp
        .json()
        .await
        .inspect_err(|_| {
            tracing::warn!("商店远程索引 JSON 解析失败（降级为仅内置）");
        })
        .ok()?;
    if body.get("format").and_then(|f| f.as_str()) != Some(INDEX_FORMAT)
    {
        tracing::warn!("商店远程索引 format 不识别（降级为仅内置）");
        return None;
    }
    let mut items = Vec::new();
    if let Some(arr) = body.get("items").and_then(|i| i.as_array()) {
        for it in arr {
            let (Some(pack_id), Some(kind), Some(name), Some(item_url)) = (
                it.get("pack_id").and_then(|v| v.as_str()),
                it.get("kind").and_then(|v| v.as_str()),
                it.get("name").and_then(|v| v.as_str()),
                it.get("url").and_then(|v| v.as_str()),
            ) else {
                continue;
            };
            if !["taxonomy", "theme"].contains(&kind) {
                continue;
            }
            items.push(CatalogItem {
                pack_id: pack_id.to_string(),
                kind: kind.to_string(),
                name: name.to_string(),
                description: it
                    .get("description")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string(),
                version: it
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("1")
                    .to_string(),
                source: "remote",
                site_type: None,
                url: Some(item_url.to_string()),
            });
        }
    }
    Some((items, url))
}

/// GET /admin/content-packs/catalog —— 目录（内置 ∪ 远程）+ 已装版本对照
#[get("/admin/content-packs/catalog")]
pub(crate) async fn pack_catalog(
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
    let mut items = builtin_catalog(&state).await?;
    items.extend(builtin_assets_catalog(&state).await?);
    let mut index_url: Option<String> = None;
    if let Some((remote, url)) = remote_catalog(&state).await {
        index_url = Some(url);
        items.extend(remote);
    }
    // 已装对照：pack_id → (id, version)（前端标「已装/可更新」）
    let installed: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT pack_id, id, version FROM content_packs",
    )
    .fetch_all(&state.repo.db)
    .await
    .map_err(internal)?;
    let installed_map: std::collections::HashMap<String, (i64, String)> =
        installed
            .into_iter()
            .map(|(pid, id, ver)| (pid, (id, ver)))
            .collect();
    let items: Vec<serde_json::Value> = items
        .iter()
        .map(|it| {
            let mut v = serde_json::to_value(it).unwrap_or_default();
            if let Some(obj) = v.as_object_mut() {
                if let Some((row_id, ver)) = installed_map.get(&it.pack_id)
                {
                    obj.insert(
                        "installed_row_id".into(),
                        serde_json::json!(row_id),
                    );
                    obj.insert(
                        "installed_version".into(),
                        serde_json::json!(ver),
                    );
                }
            }
            v
        })
        .collect();
    Ok(ok(serde_json::json!({
        "items": items,
        "count": items.len(),
        "remote_index": index_url,
        // 降级标记：配了远程索引但拉取失败（前端横幅提示）
        "degraded": index_url.is_none()
            && remote_configured(&state).await,
    })))
}

async fn remote_configured(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> bool {
    sqlx::query_scalar::<_, String>(
        "SELECT value FROM site_settings WHERE name = 'marketplace_index_url'",
    )
    .fetch_optional(&state.repo.db)
    .await
    .ok()
    .flatten()
    .map(|v: String| !v.trim().is_empty())
    .unwrap_or(false)
}

/// POST /admin/content-packs/install —— 目录条目安装（内置 = 站型包拼装；
/// 远程 = 服务端拉包文件后走 M1 导入路径）
#[derive(serde::Deserialize)]
pub(crate) struct InstallBody {
    /// 目录条目的 pack_id（builtin.* 或远程 pack_id）
    pack_id: String,
    #[serde(default)]
    confirm: bool,
}

#[post("/admin/content-packs/install")]
pub(crate) async fn pack_install(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    body: web::Json<InstallBody>,
) -> DomainResult<HttpResponse> {
    let auth = require_auth(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let pack = if let Some(code) =
        body.pack_id.strip_prefix("builtin.taxonomy.")
    {
        builtin_taxonomy_pack(&state, code).await?
    } else if let Some(code) =
        body.pack_id.strip_prefix("builtin.assets.")
    {
        builtin_assets_pack(&state, code).await?
    } else {
        // 远程条目：从远程索引反查 url 再拉包文件
        let items = remote_catalog(&state).await;
        let item = items
            .and_then(|(its, _)| {
                its.into_iter().find(|i| i.pack_id == body.pack_id)
            })
            .ok_or_else(|| DomainError::Validation(
                "目录条目不存在或远程索引不可用".into(),
            ))?;
        fetch_remote_pack(&item.url.unwrap_or_default()).await?
    };
    super::pack_import::import(&state, &auth.id, &pack, body.confirm).await
}

/// 内置素材目录（0174 builtin_assets → CatalogItem；动态生成不物化）
async fn builtin_assets_catalog(
    state: &web::Data<std::sync::Arc<AppState>>,
) -> DomainResult<Vec<CatalogItem>> {
    let rows: Vec<(String, String, String, serde_json::Value)> =
        sqlx::query_as(
            "SELECT code, name, description, payload              FROM builtin_assets ORDER BY code",
        )
        .fetch_all(&state.repo.db)
        .await
        .map_err(internal)?;
    Ok(rows
        .into_iter()
        .map(|(code, name, descr, payload)| {
            let n_medals = payload
                .pointer("/tables/medals")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            let n_frames = payload
                .pointer("/tables/avatar_frames")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);
            CatalogItem {
                pack_id: format!("builtin.assets.{code}"),
                kind: "assets".into(),
                name,
                description: format!(
                    "{descr}（{n_medals} 勋章 / {n_frames} 头像框）"
                ),
                version: "1".into(),
                source: "builtin",
                site_type: None,
                url: None,
            }
        })
        .collect())
}

/// builtin_assets 行 → assets 包文件（payload 原样搬运）
async fn builtin_assets_pack(
    state: &web::Data<std::sync::Arc<AppState>>,
    code: &str,
) -> DomainResult<serde_json::Value> {
    let row: Option<(String, serde_json::Value)> = sqlx::query_as(
        "SELECT name, payload FROM builtin_assets WHERE code = $1",
    )
    .bind(code)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(internal)?;
    let Some((name, payload)) = row else {
        return Err(DomainError::Validation(
            "内置素材条目不存在".into(),
        ));
    };
    Ok(serde_json::json!({
        "format": "fluxtorrent.contentpack",
        "version": 1,
        "kind": "assets",
        "pack_id": format!("builtin.assets.{code}"),
        "name": name,
        "core_compat": "*",
        "pack_version": "1",
        "payload": payload,
    }))
}

/// 站型包 → taxonomy 包文件（categories/sections 动态拼装）
async fn builtin_taxonomy_pack(
    state: &web::Data<std::sync::Arc<AppState>>,
    code: &str,
) -> DomainResult<serde_json::Value> {
    let row: Option<(
        String,
        serde_json::Value,
        Option<serde_json::Value>,
    )> = sqlx::query_as(
        "SELECT name, categories, sections FROM site_type_packs \
         WHERE code = $1",
    )
    .bind(code)
    .fetch_optional(&state.repo.db)
    .await
    .map_err(internal)?;
    let Some((name, categories, sections)) = row else {
        return Err(DomainError::Validation(
            "内置目录条目不存在（站型包缺失）".into(),
        ));
    };
    // sections 归一：kinds/dict 缺省为空（快照口径一致）
    let empty = serde_json::json!({ "kinds": [], "dict": {} });
    let sections = sections.unwrap_or(empty);
    let mut sections = serde_json::from_value::<serde_json::Value>(
        sections,
    )
    .unwrap_or_else(|_| serde_json::json!({ "kinds": [], "dict": {} }));
    if let Some(obj) = sections.as_object_mut() {
        obj.entry("kinds".to_string())
            .or_insert_with(|| serde_json::json!([]));
        obj.entry("dict".to_string())
            .or_insert_with(|| serde_json::json!({}));
    }
    Ok(serde_json::json!({
        "format": "fluxtorrent.contentpack",
        "version": 1,
        "kind": "taxonomy",
        "pack_id": format!("builtin.taxonomy.{code}"),
        "name": format!("{name}·分类学"),
        "core_compat": "*",
        "pack_version": "1",
        // 商店目录载荷对齐（二审 R10-4）：taxonomy 包只携带分类学；
        // 模块矩阵/标签/经济预设等完整站型能力不随包走，此处明示，
        // 避免「装了 movie 包却没有 movie 模块矩阵」的预期落差。
        "description": "仅含分类学与质量维度；模块开关/标签/等级叙事等完整站型能力请走后台「站型切换」",
        "payload": { "categories": categories, "sections": sections },
    }))
}

/// 远程包文件拉取：校验 format 后原样交给导入路径
async fn fetch_remote_pack(
    url: &str,
) -> DomainResult<serde_json::Value> {
    if url.is_empty() || !url.starts_with("https://") {
        return Err(DomainError::Validation(
            "远程包地址无效（需 https）".into(),
        ));
    }
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|e| DomainError::Validation(
            format!("远程包拉取失败：{e}"),
        ))?;
    if !resp.status().is_success() {
        return Err(DomainError::Validation(format!(
            "远程包上游异常（HTTP {}）",
            resp.status().as_u16()
        )));
    }
    // R11 加固：包文件大小上限 16MiB（与适配器 wasm 上限同量级）
    if resp.content_length().unwrap_or(0) > 16 * 1024 * 1024 {
        return Err(DomainError::Validation(
            "远程包超过 16MiB 上限".into(),
        ));
    }
    let pack: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| DomainError::Validation(
            format!("远程包 JSON 解析失败：{e}"),
        ))?;
    if pack.get("format").and_then(|f| f.as_str())
        != Some("fluxtorrent.contentpack")
    {
        return Err(DomainError::Validation(
            "远程包 format 不识别".into(),
        ));
    }
    Ok(pack)
}
