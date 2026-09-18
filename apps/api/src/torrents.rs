//! 种子域仓储与查询（M02/M03）：游标分页 + 覆盖索引（§6.2）。

use serde::{Deserialize, Serialize};
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct TorrentRow {
    pub id: i64,
    pub info_hash: String,
    pub name: String,
    pub small_descr: Option<String>,
    pub category_id: i32,
    /// 介质列（0087 起可空，仅为兼容老数据；新数据在 torrent_sections）
    pub medium_id: Option<i32>,
    pub grade_id: Option<i32>,
    pub edition_id: Option<i32>,
    pub size: i64,
    pub seeders: i32,
    pub leechers: i32,
    pub times_completed: i32,
    pub comments: i64,
    #[serde(rename = "official")]
    pub official_tag: bool,
    pub anonymous: bool,
    pub approval_status: i16,
    pub sticky: bool,
    pub owner_name: Option<String>,
    pub promotion: Option<String>,
    /// 进行中促销的截止时刻（列表展示「剩余时间」，参考站口径）
    pub promotion_ends_at: Option<chrono::DateTime<chrono::Utc>>,
    /// 媒体评分（media_info.rating，豆瓣/IMDb 口径由录入方决定；首页海报墙展示）
    pub rating: Option<String>,
    /// 海报图 URL（media_info.poster；缺省时前端用生成式海报兜底）
    pub poster: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// 详情页扩展字段（简介/文件数/感谢数；列表不需要，独立查询）
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TorrentDetailRow {
    pub id: i64,
    pub descr: Option<String>,
    pub numfiles: i32,
    pub thanks_count: i64,
    pub bookmark_count: i64,
    pub last_action: Option<chrono::DateTime<chrono::Utc>>,
    pub views: i64,
    /// 付费下载（0086）：定价（0 = 免费）
    pub price: i64,
    /// 当前用户是否已支付（owner 恒免）
    pub purchased: bool,
    pub is_owner: bool,
    /// 多维属性（第八轮 Section）：kind → { dict_id, name }
    pub sections: serde_json::Value,
    /// MediaInfo 全文（media_info.mediainfo；详情页折叠块原样展示）
    pub mediainfo: Option<String>,
}

/// 详情页文件列表（files 表；无记录时前端隐藏该区块）
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FileRow {
    pub file_index: i32,
    pub path: String,
    pub size: i64,
}

/// 感谢者列表（近 50 人）
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ThankRow {
    pub username: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Default, Serialize)]
pub struct TorrentFilter {
    /// 分类筛选（0088 起支持多选：数组传 ANY 命中；空数组 = 不过滤）
    pub category_id: Option<Vec<i32>>,
    pub medium_id: Option<i32>,
    pub grade_id: Option<i32>,
    pub edition_id: Option<i32>,
    pub official: Option<bool>,
    pub include_dead: bool,
    /// 查看未过审（待审/被拒）种子——需 torrent.see_banned 权限，端点侧校验
    pub include_unapproved: bool,
    pub search: Option<String>,
    /// 列表排序（旧站 torrents.php 口径）：created（默认）/ seeders / size / completed
    pub sort: Option<String>,
    /// 标签筛选（T-04）：tag_dict.id，命中 tags 关联
    pub tag_id: Option<i32>,
    /// 第八轮 Section 多维筛选：kind → dict_id（kind 走白名单，dict_id 为整数，拼接安全）
    #[serde(default)]
    pub sections: Vec<(String, i64)>,
    /// 搜索范围（旧站口径）：0=标题(默认) 1=副标题/简介 3=发布者 4=IMDb
    #[serde(default)]
    pub search_area: Option<i32>,
    /// 存活（0102）：None=默认(仅活种, 兼容 include_dead) Some(0)=全部 Some(1)=仅活种 Some(2)=仅断种
    #[serde(default)]
    pub alive: Option<i16>,
    /// 种子状态（0102，viewer 维度）：seeding/leeching/completed/incomplete/notseeding
    #[serde(default)]
    pub status: Option<String>,
    /// 审核状态（0102）：0=全部 1=通过 2=被拒（需 see_banned 由入口剥离）
    #[serde(default)]
    pub approval: Option<i16>,
    /// 匹配模式：0=AND 模糊(默认) 2=精确等值
    #[serde(default)]
    pub search_mode: Option<i32>,
    // ===== 高级搜索增强（体积/时间/做种数/排除词/优惠/发布者/仅我发布） =====
    /// 体积下界（字节，含）
    #[serde(default)]
    pub size_min: Option<i64>,
    /// 体积上界（字节，含）
    #[serde(default)]
    pub size_max: Option<i64>,
    /// 发布时间下界（`YYYY-MM-DD`，含当天；入口已校验格式）
    #[serde(default)]
    pub date_from: Option<String>,
    /// 发布时间上界（`YYYY-MM-DD`，含当天）
    #[serde(default)]
    pub date_to: Option<String>,
    /// 做种数下界（含）
    #[serde(default)]
    pub min_seeders: Option<i32>,
    /// 做种数上界（含）
    #[serde(default)]
    pub max_seeders: Option<i32>,
    /// 排除关键字（标题/简介均不得命中，OR 语义：整串作为一个词）
    #[serde(default)]
    pub exclude: Option<String>,
    /// 优惠筛选：free=免费(含 2x 免费) x2=2倍 half=半价 any=任意优惠 none=无优惠
    #[serde(default)]
    pub promo: Option<String>,
    /// 发布者用户名（模糊）
    #[serde(default)]
    pub owner: Option<String>,
    /// 仅显示当前视角用户发布的种子（viewer 维度）
    #[serde(default)]
    pub only_mine: bool,
    // ===== 高级搜索补齐（0118：下载数/完成数区间 + 匿名发布） =====
    /// 下载数（leechers）下界（含）
    #[serde(default)]
    pub min_leechers: Option<i32>,
    /// 下载数上界（含）
    #[serde(default)]
    pub max_leechers: Option<i32>,
    /// 完成数（times_completed）下界（含）
    #[serde(default)]
    pub min_completed: Option<i32>,
    /// 完成数上界（含）
    #[serde(default)]
    pub max_completed: Option<i32>,
    /// 匿名发布：None=不限 1=仅匿名 2=仅具名（口径同 alive 三态）
    #[serde(default)]
    pub anonymous: Option<i16>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TorrentPage {
    pub items: Vec<TorrentRow>,
    pub next_cursor: Option<String>,
    pub total_estimate: i64,
}

const MAX_LIMIT: i64 = 50;

/// LIKE 通配转义（防用户输入的 `%`/`_` 变成通配符；配合 SQL 侧 `ESCAPE chr(92)`）
fn esc_like(s: &str) -> String {
    s.replace('\\', "").replace('%', "\\%").replace('_', "\\_")
}

fn sort_expr(sticky_expr: &str, col: &str, asc: bool) -> String {
    let dir = if asc { "ASC" } else { "DESC" };
    format!("{sticky_expr}, {col} {dir}, t.id {dir}")
}

/// 优惠（promotions）命中的统一匹配式：种子级直挂 或 全局/官种/非官种/分类作用域。
/// 与列表 SELECT 中 promotion 子查询同口径，保证「筛选出的免费种」与徽标显示一致。
const PROMO_MATCH: &str = "p.starts_at <= now() AND p.ends_at > now() AND (\
     p.torrent_id = t.id OR (p.torrent_id IS NULL AND (\
       p.scope = 'global' \
       OR (p.scope = 'official' AND t.official_tag) \
       OR (p.scope = 'non_official' AND NOT t.official_tag) \
       OR (p.scope = 'category' AND t.category_id = p.category_id))))";

/// 追加筛选的占位符槽位。列表与计数两条 SQL 的参数编号各自独立，
/// 故谓词文本按槽位号生成（同一份语义，两套编号），避免手改编号串号。
#[derive(Clone, Copy)]
struct ExtraSlots {
    size_min: u32,
    size_max: u32,
    date_from: u32,
    date_to: u32,
    min_seeders: u32,
    max_seeders: u32,
    exclude: u32,
    promo: u32,
    owner: u32,
    mine: u32,
    min_leechers: u32,
    max_leechers: u32,
    min_completed: u32,
    max_completed: u32,
    anonymous: u32,
}

/// 高级搜索增强谓词（体积/时间/做种数/排除词/优惠/发布者/仅我发布）。
/// 全部走参数化绑定（`$n IS NULL OR ...`），无注入面；谓词是否生效由绑定的 None/Some 决定。
fn extra_preds(s: ExtraSlots) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        " AND (${a}::bigint IS NULL OR t.size >= ${a})",
        a = s.size_min
    ));
    out.push_str(&format!(
        " AND (${a}::bigint IS NULL OR t.size <= ${a})",
        a = s.size_max
    ));
    // 日期按「日历日」口径比较（避免时区把当天数据挤出去）：下界含当日 00:00，上界含当日 23:59:59
    out.push_str(&format!(
        " AND (${a}::date IS NULL OR t.created_at >= ${a}::date)",
        a = s.date_from
    ));
    out.push_str(&format!(
        " AND (${a}::date IS NULL OR t.created_at < (${a}::date + 1))",
        a = s.date_to
    ));
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR t.seeders >= ${a})",
        a = s.min_seeders
    ));
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR t.seeders <= ${a})",
        a = s.max_seeders
    ));
    // 排除词：标题与简介任一命中即排除（pattern 已由调用方转义/加通配）
    out.push_str(&format!(
        " AND (${a}::text IS NULL OR (t.name NOT ILIKE ${a} ESCAPE chr(92) \
           AND COALESCE(t.small_descr, '') NOT ILIKE ${a} ESCAPE chr(92) \
           AND COALESCE(t.descr, '') NOT ILIKE ${a} ESCAPE chr(92)))",
        a = s.exclude
    ));
    let p = s.promo;
    out.push_str(&format!(
        " AND (${p}::text IS NULL \
           OR (${p} = 'free' AND EXISTS(SELECT 1 FROM promotions p WHERE {PROMO_MATCH} AND p.kind::text IN ('free','x2free'))) \
           OR (${p} = 'x2' AND EXISTS(SELECT 1 FROM promotions p WHERE {PROMO_MATCH} AND p.kind::text IN ('x2','x2free','x2half'))) \
           OR (${p} = 'half' AND EXISTS(SELECT 1 FROM promotions p WHERE {PROMO_MATCH} AND p.kind::text IN ('half','x2half'))) \
           OR (${p} = 'any' AND EXISTS(SELECT 1 FROM promotions p WHERE {PROMO_MATCH})) \
           OR (${p} = 'none' AND NOT EXISTS(SELECT 1 FROM promotions p WHERE {PROMO_MATCH})))"
    ));
    out.push_str(&format!(
        " AND (${a}::text IS NULL OR u.username ILIKE ${a} ESCAPE chr(92))",
        a = s.owner
    ));
    out.push_str(&format!(
        " AND (${a}::bigint IS NULL OR t.owner_id = ${a})",
        a = s.mine
    ));
    // 0118 高级搜索补齐：下载数 / 完成数区间（与做种数同口径，含边界）
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR t.leechers >= ${a})",
        a = s.min_leechers
    ));
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR t.leechers <= ${a})",
        a = s.max_leechers
    ));
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR t.times_completed >= ${a})",
        a = s.min_completed
    ));
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR t.times_completed <= ${a})",
        a = s.max_completed
    ));
    // 匿名发布三态：1=仅匿名 2=仅具名（缺省/0 不筛；口径同 alive）
    out.push_str(&format!(
        " AND (${a}::int IS NULL OR ${a} = 0 OR (${a} = 1 AND t.anonymous) OR (${a} = 2 AND NOT t.anonymous))",
        a = s.anonymous
    ));
    out
}

/// 无视角列表（viewer=0）：不携带 snatches 状态筛选语义的通用入口。
#[allow(dead_code)] // 兼容保留：外部端点已迁移至 list_torrents_noclamp / list_torrents_as
pub async fn list_torrents(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<i64>,
    limit: i64,
) -> DomainResult<TorrentPage> {
    list_torrents_as(db, filter, cursor, limit, 0).await
}

/// viewer 版本（0102 种子状态筛选需要 snatches.user_id 视角；0 = 无人视角 = 状态筛选空转）
pub async fn list_torrents_as(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<i64>,
    limit: i64,
    viewer: i64,
) -> DomainResult<TorrentPage> {
    let limit = limit.clamp(1, MAX_LIMIT);
    list_torrents_noclamp_as(db, filter, cursor, limit, viewer).await
}

/// 不做 50 上限钳制的列表查询：仅供 Torznab 等需要 offset+limit>50 深翻页的外部端点，
/// 调用方必须自行 clamp 防滥用（见 economy_http torznab_search）。viewer=0（无状态筛选视角）。
pub async fn list_torrents_noclamp(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<i64>,
    limit: i64,
) -> DomainResult<TorrentPage> {
    list_torrents_noclamp_as(db, filter, cursor, limit, 0).await
}

/// 审计修复（P1）：list_torrents_as 此前把 viewer 转成字符串后丢弃，内部又硬编码
/// viewer_sql="0"，status=seeding/leeching/... 全部按「无人」匹配恒空。现在把
/// viewer 一路传到 SQL 构造（i64 内插无注入面）。
pub async fn list_torrents_noclamp_as(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<i64>,
    limit: i64,
    viewer: i64,
) -> DomainResult<TorrentPage> {
    let limit = limit.max(1);
    let viewer_sql = viewer.to_string();
    // 匹配模式（旧站 torrents.php 口径）：0/缺省 = AND 模糊；2 = 精确等值（不带通配）
    let exact = filter.search_mode == Some(2);
    let pattern = filter.search.as_deref().map(|s| {
        if exact {
            s.to_string()
        } else {
            format!("%{}%", esc_like(s))
        }
    });
    // 排除词/发布者：与关键字同口径（模糊 + 转义）
    let exclude_pat = filter
        .exclude
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", esc_like(s)));
    let owner_pat = filter
        .owner
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| format!("%{}%", esc_like(s)));

    // 排序白名单（防注入）；非 id 排序时退化为 OFFSET 无关的「前 N 截断」：
    // 排序键 + id 组成稳定排序，游标仍按 id 翻页（与默认排序一致，简单可靠）。
    // 置顶口径（0063 起；0089 扩展二级置顶）：pos_state 1=一级 2=二级，pos_state_until 到期自动回落，
    // 旧列 sticky 仍被官种联动使用——「任一生效即置顶」，一级 > 二级 > 普通置顶。
    let sticky_expr = "(GREATEST(t.sticky::int, CASE WHEN t.pos_state IN (1, 2) AND (t.pos_state_until IS NULL OR t.pos_state_until > now()) THEN CASE t.pos_state WHEN 1 THEN 2 WHEN 2 THEN 1 ELSE 0 END ELSE 0 END)) DESC";
    // 表头排序（NP colhead 口径）：asc 前缀反转方向；comments = 评论数
    let (key, asc) = match filter.sort.as_deref() {
        Some(s) if s.ends_with("_asc") => (&s[..s.len() - 4], true),
        other => (other.unwrap_or(""), false),
    };
    let order = match key {
        "seeders" => sort_expr(&sticky_expr, "t.seeders", asc),
        "leechers" => sort_expr(&sticky_expr, "t.leechers", asc),
        "size" => sort_expr(&sticky_expr, "t.size", asc),
        "completed" => sort_expr(&sticky_expr, "t.times_completed", asc),
        // 发布时间/名称排序（高级搜索排序扩展；NP「按发布时间/标题」口径）
        "created" => sort_expr(&sticky_expr, "t.created_at", asc),
        "name" => sort_expr(&sticky_expr, "t.name", asc),
        "comments" => sort_expr(
            &sticky_expr,
            "(SELECT count(*) FROM comments c WHERE c.torrent_id = t.id)",
            asc,
        ),
        _ => format!("{sticky_expr}, t.id DESC"),
    };
    // 第八轮 Section 多维筛选：每个维度一个子查询谓词（kind 以 section_kinds 存在性校验 + i64 内插，无注入面）
    // 多维多选（0102）：同维度多值 OR（= ANY），跨维度 AND
    let mut sec_sql = String::new();
    {
        use std::collections::BTreeMap;
        let mut by_kind: BTreeMap<&str, Vec<i64>> = BTreeMap::new();
        for (kind, dict_id) in &filter.sections {
            by_kind.entry(kind).or_default().push(*dict_id);
        }
        for (kind, ids) in by_kind {
            if !crate::admin_p3_http::is_custom_kind(db, kind).await {
                continue;
            }
            let list = ids
                .iter()
                .map(|i| i.to_string())
                .collect::<Vec<_>>()
                .join(",");
            sec_sql.push_str(&format!(
                " AND t.id IN (SELECT torrent_id FROM torrent_sections WHERE kind = '{kind}' AND dict_id = ANY(ARRAY[{list}]::bigint[]))"
            ));
        }
    }
    // 搜索范围分流（旧站口径）：0=标题+全字段(默认) 1=副标题/简介 3=发布者 4=IMDb
    let esc = if exact { "" } else { " ESCAPE chr(92)" };
    let search_pred = match filter.search_area.unwrap_or(0) {
        1 => format!("AND ($6::text IS NULL OR t.small_descr ILIKE $6{esc} OR t.descr ILIKE $6{esc})"),
        3 => format!("AND ($6::text IS NULL OR u.username ILIKE $6{esc})"),
        4 => format!("AND ($6::text IS NULL OR t.media_info->>'imdb' ILIKE $6{esc} OR t.descr ILIKE $6{esc})"),
        _ => format!("AND ($6::text IS NULL OR t.name ILIKE $6{esc}                OR t.small_descr ILIKE $6{esc}                OR t.descr ILIKE $6{esc}                OR t.id IN (SELECT torrent_id FROM files WHERE path ILIKE $6{esc}))"),
    };

    // 存活三态（0102）：alive 显式给出时覆盖 include_dead（0=全部 1=活种 2=断种）
    let alive_pred = match filter.alive {
        Some(0) => String::new(),
        Some(2) => " AND t.seeders = 0".into(),
        _ => {
            if filter.include_dead {
                String::new()
            } else {
                " AND t.seeders > 0".into()
            }
        }
    };
    // 审核状态（0102）：0=全部 1=通过（默认） 2=被拒（入口已按 see_banned 剥离）
    let approval_pred: String = match filter.approval {
        Some(0) | None => " AND t.approval_status = 1".into(),
        Some(2) => " AND t.approval_status IN (2, 3)".into(),
        Some(1) => " AND t.approval_status = 1".into(),
        Some(_) => " AND t.approval_status = 1".into(),
    };
    // 种子状态（0102，viewer 维度）：需要 snatches 存在性判断（viewer 由调用方注入 SQL 文本，参数化见 bind）
    let status_pred = match filter.status.as_deref() {
        Some("seeding") => " AND EXISTS(SELECT 1 FROM snatches s WHERE s.torrent_id = t.id AND s.user_id = {viewer} AND s.seeding)".to_string(),
        Some("leeching") => " AND EXISTS(SELECT 1 FROM snatches s WHERE s.torrent_id = t.id AND s.user_id = {viewer} AND s.leeching)".to_string(),
        Some("completed") => " AND EXISTS(SELECT 1 FROM snatches s WHERE s.torrent_id = t.id AND s.user_id = {viewer} AND s.completed_at IS NOT NULL)".to_string(),
        Some("incomplete") => " AND EXISTS(SELECT 1 FROM snatches s WHERE s.torrent_id = t.id AND s.user_id = {viewer} AND s.completed_at IS NULL AND (s.uploaded > 0 OR s.downloaded > 0))".to_string(),
        Some("notseeding") => " AND NOT EXISTS(SELECT 1 FROM snatches s WHERE s.torrent_id = t.id AND s.user_id = {viewer} AND s.seeding)".to_string(),
        _ => String::new(),
    }
    .replace("{viewer}", &viewer_sql);

    // 高级搜索增强：体积/时间/做种数/排除词/优惠/发布者/仅我发布（列表侧编号 $11..$20）
    // 0118 补齐：下载数/完成数区间 + 匿名发布（列表侧 $21..$25）
    let extra_sql = extra_preds(ExtraSlots {
        size_min: 11,
        size_max: 12,
        date_from: 13,
        date_to: 14,
        min_seeders: 15,
        max_seeders: 16,
        exclude: 17,
        promo: 18,
        owner: 19,
        mine: 20,
        min_leechers: 21,
        max_leechers: 22,
        min_completed: 23,
        max_completed: 24,
        anonymous: 25,
    });
    let sql = format!(
        r#"
        SELECT t.id, t.info_hash, t.name, t.small_descr, t.category_id, t.medium_id,
               t.grade_id, t.edition_id, t.size, t.seeders, t.leechers, t.times_completed,
               (SELECT count(*) FROM comments c WHERE c.torrent_id = t.id) AS comments,
               t.official_tag, t.anonymous, t.approval_status, t.sticky,
               CASE WHEN t.anonymous THEN NULL ELSE u.username END AS owner_name,
               (SELECT p.kind::text FROM promotions p
                  WHERE p.starts_at <= now() AND p.ends_at > now() AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id))))
                  ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion,
               (SELECT p.ends_at FROM promotions p
                  WHERE p.starts_at <= now() AND p.ends_at > now() AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id))))
                  ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion_ends_at,
               t.media_info->>'rating' AS rating,
               t.media_info->>'poster' AS poster,
               t.created_at
        FROM torrents t
        LEFT JOIN users u ON u.id = t.owner_id
        WHERE (t.approval_status = 1 OR $10::bool)
          AND ($1::int[] IS NULL OR t.category_id = ANY($1))
          AND ($2::int IS NULL OR t.medium_id = $2)
          AND ($3::int IS NULL OR t.grade_id = $3)
          AND ($4::int IS NULL OR t.edition_id = $4)
          AND ($5::bool IS NULL OR t.official_tag = $5)
          {alive_pred}
          {approval_pred}
          {status_pred}
          {search_pred}
          AND ($7::bigint IS NULL OR t.id < $7)
          AND ($9::int IS NULL OR t.id IN (SELECT torrent_id FROM tags WHERE tag_id = $9))
          {sec_sql}
          {extra_sql}
        ORDER BY {order}
        LIMIT $8
        "#
    );
    let rows = sqlx::query_as::<_, TorrentRow>(&sql)
        .bind(filter.category_id.clone())
        .bind(filter.medium_id)
        .bind(filter.grade_id)
        .bind(filter.edition_id)
        .bind(filter.official)
        .bind(&pattern)
        .bind(cursor)
        .bind(limit + 1)
        .bind(filter.tag_id)
        .bind(filter.include_unapproved)
        .bind(filter.size_min)
        .bind(filter.size_max)
        .bind(filter.date_from.as_deref())
        .bind(filter.date_to.as_deref())
        .bind(filter.min_seeders)
        .bind(filter.max_seeders)
        .bind(&exclude_pat)
        .bind(filter.promo.as_deref())
        .bind(&owner_pat)
        .bind(filter.only_mine.then_some(viewer))
        .bind(filter.min_leechers)
        .bind(filter.max_leechers)
        .bind(filter.min_completed)
        .bind(filter.max_completed)
        .bind(filter.anonymous)
        .fetch_all(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 计数与列表同口径（0093 修复）：此前 count 只用「分类/媒介/学段/版本/官种/死种/标题搜索」，
    // 多维筛选、标签、搜索范围（副标题/发布者/IMDb）一律不计入 —— 页面会显示「共 N 个」却零行。
    // 计数同口径：额外谓词在计数侧编号 $9..$18（0118 补齐后 $19..$23）
    let extra_count_sql = extra_preds(ExtraSlots {
        size_min: 9,
        size_max: 10,
        date_from: 11,
        date_to: 12,
        min_seeders: 13,
        max_seeders: 14,
        exclude: 15,
        promo: 16,
        owner: 17,
        mine: 18,
        min_leechers: 19,
        max_leechers: 20,
        min_completed: 21,
        max_completed: 22,
        anonymous: 23,
    });
    let count_sql = format!(
        "SELECT count(*) FROM torrents t LEFT JOIN users u ON u.id = t.owner_id \
         WHERE (t.approval_status = 1 OR $7::bool) \
         AND ($1::int[] IS NULL OR t.category_id = ANY($1)) AND ($2::int IS NULL OR t.medium_id = $2) \
         AND ($3::int IS NULL OR t.grade_id = $3) AND ($4::int IS NULL OR t.edition_id = $4) \
         AND ($5::bool IS NULL OR t.official_tag = $5) {alive_pred} {approval_pred} {status_pred} \
         {search_pred} \
         AND ($8::int IS NULL OR t.id IN (SELECT torrent_id FROM tags WHERE tag_id = $8)) \
         {sec_sql} {extra_count_sql}",
    );
    let total: i64 = sqlx::query_scalar(&count_sql)
        .bind(filter.category_id.clone())
        .bind(filter.medium_id)
        .bind(filter.grade_id)
        .bind(filter.edition_id)
        .bind(filter.official)
        .bind(&pattern)
        .bind(filter.include_unapproved)
        .bind(filter.tag_id)
        .bind(filter.size_min)
        .bind(filter.size_max)
        .bind(filter.date_from.as_deref())
        .bind(filter.date_to.as_deref())
        .bind(filter.min_seeders)
        .bind(filter.max_seeders)
        .bind(&exclude_pat)
        .bind(filter.promo.as_deref())
        .bind(&owner_pat)
        .bind(filter.only_mine.then_some(viewer))
        .bind(filter.min_leechers)
        .bind(filter.max_leechers)
        .bind(filter.min_completed)
        .bind(filter.max_completed)
        .bind(filter.anonymous)
        .fetch_one(db)
        .await
        .unwrap_or(0);

    let has_more = rows.len() as i64 > limit;
    let items = rows.into_iter().take(limit as usize).collect::<Vec<_>>();
    let next_cursor = has_more.then(|| items.last().map(|r| r.id.to_string()).unwrap_or_default());
    Ok(TorrentPage {
        items,
        next_cursor,
        total_estimate: total,
    })
}

pub async fn get_torrent(
    db: &PgPool,
    id: i64,
    reveal_owner: bool,
    viewer: Option<(i64, bool)>, // (user_id, is_staff)：G7 暂缓种仅本人+staff 可见
) -> DomainResult<TorrentRow> {
    // torrent.view_anonymous：持权者可见匿名种子的真实发布者
    let owner_expr = if reveal_owner {
        "u.username AS owner_name"
    } else {
        "CASE WHEN t.anonymous THEN NULL ELSE u.username END AS owner_name"
    };
    // G7 POSTPONED：status=4 的种子只对发布者本人与 staff 开放；
    // staff（viewer.is_staff）或本人（owner_id = viewer.0）时放宽到 approval_status IN (1,4)
    // 审计修复（P1）：本人对 status=0 待审种子也应可见（上传后马上看详情不再 404）
    let vis = match viewer {
        Some((uid, is_staff)) => format!(
            "(t.approval_status = 1 OR ((t.approval_status = 4 OR t.approval_status = 0) AND ({is_staff} OR t.owner_id = {uid})))"
        ),
        None => "t.approval_status = 1".to_string(),
    };
    let page = sqlx::query_as::<_, TorrentRow>(
        &format!(r#"
        SELECT t.id, t.info_hash, t.name, t.small_descr, t.category_id, t.medium_id,
               t.grade_id, t.edition_id, t.size, t.seeders, t.leechers, t.times_completed,
               (SELECT count(*) FROM comments c WHERE c.torrent_id = t.id) AS comments,
               t.official_tag, t.anonymous, t.approval_status, t.sticky,
               {owner_expr},
               (SELECT p.kind::text FROM promotions p
                  WHERE p.starts_at <= now() AND p.ends_at > now() AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id))))
                  ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion,
               (SELECT p.ends_at FROM promotions p
                  WHERE p.starts_at <= now() AND p.ends_at > now() AND (
                    p.torrent_id = t.id
                    OR (p.torrent_id IS NULL AND (
                        p.scope = 'global'
                        OR (p.scope = 'official' AND t.official_tag)
                        OR (p.scope = 'non_official' AND NOT t.official_tag)
                        OR (p.scope = 'category' AND t.category_id = p.category_id))))
                  ORDER BY CASE p.kind::text WHEN 'x2free' THEN 6 WHEN 'x2half' THEN 5 WHEN 'x2' THEN 4 WHEN 'free' THEN 3 WHEN 'half' THEN 2 WHEN 'p30' THEN 1 ELSE 0 END DESC, p.id DESC LIMIT 1) AS promotion_ends_at,
               t.media_info->>'rating' AS rating,
               t.media_info->>'poster' AS poster,
               t.created_at
        FROM torrents t LEFT JOIN users u ON u.id = t.owner_id
        WHERE t.id = $1 AND {vis}
        "#)
    )
    .bind(id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    items_or_not_found(page, id)
}

fn items_or_not_found(mut v: Vec<TorrentRow>, id: i64) -> DomainResult<TorrentRow> {
    if v.is_empty() {
        Err(DomainError::NotFound(id))
    } else {
        Ok(v.remove(0))
    }
}

pub async fn get_torrent_detail(
    db: &PgPool,
    id: i64,
    viewer: i64,
) -> DomainResult<TorrentDetailRow> {
    let row = sqlx::query_as::<_, TorrentDetailRow>(
        r#"
        SELECT t.id, t.descr, t.numfiles, t.price, t.media_info->>'mediainfo' AS mediainfo,
               (t.owner_id = $2) AS is_owner,
               EXISTS(SELECT 1 FROM torrent_purchases p WHERE p.torrent_id = t.id AND p.user_id = $2) AS purchased,
               (SELECT count(*) FROM thanks th WHERE th.torrent_id = t.id) AS thanks_count,
               (SELECT count(*) FROM bookmarks b WHERE b.torrent_id = t.id) AS bookmark_count,
               GREATEST(
                   t.created_at,
                   COALESCE((SELECT max(s.completed_at) FROM snatches s WHERE s.torrent_id = t.id), t.created_at)
               ) AS last_action,
               (t.times_completed * 2 + 1)::bigint AS views,
               '{}'::jsonb AS sections
        FROM torrents t
        WHERE t.id = $1 AND t.approval_status = 1
        "#,
    )
    .bind(id)
    .bind(viewer)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut row = row.ok_or(DomainError::NotFound(id))?;
    // 0087：sections 附带维度显示名与排序（section_kinds.label），前端直接渲染
    let secs: Vec<(String, i64, String, String, i32)> = sqlx::query_as(
        "SELECT ts.kind, ts.dict_id, d.name, COALESCE(k.label, ts.kind) AS label, COALESCE(k.sort, 999) AS sort \
         FROM torrent_sections ts \
         JOIN section_dict d ON d.id = ts.dict_id \
         LEFT JOIN section_kinds k ON k.kind = ts.kind \
         WHERE ts.torrent_id = $1 ORDER BY sort, ts.kind",
    )
    .bind(id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mut m = serde_json::Map::new();
    for (kind, dict_id, name, label, sort) in secs {
        m.insert(
            kind,
            serde_json::json!({ "dict_id": dict_id, "name": name, "label": label, "sort": sort }),
        );
    }
    row.sections = serde_json::Value::Object(m);
    Ok(row)
}

pub async fn list_files(db: &PgPool, torrent_id: i64) -> DomainResult<Vec<FileRow>> {
    sqlx::query_as::<_, FileRow>(
        "SELECT file_index, path, size FROM files WHERE torrent_id = $1 ORDER BY file_index LIMIT 500",
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

pub async fn list_thanks(db: &PgPool, torrent_id: i64) -> DomainResult<Vec<ThankRow>> {
    sqlx::query_as::<_, ThankRow>(
        r#"
        SELECT u.username, th.created_at
        FROM thanks th LEFT JOIN users u ON u.id = th.user_id
        WHERE th.torrent_id = $1
        ORDER BY th.created_at DESC LIMIT 50
        "#,
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CommentRow {
    pub id: i64,
    pub torrent_id: i64,
    pub username: Option<String>,
    pub body: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list_comments(
    db: &PgPool,
    torrent_id: i64,
    limit: i64,
) -> DomainResult<Vec<CommentRow>> {
    sqlx::query_as::<_, CommentRow>(
        "SELECT c.id, c.torrent_id, u.username, c.body, c.created_at \
         FROM comments c LEFT JOIN users u ON u.id = c.user_id \
         WHERE c.torrent_id = $1 ORDER BY c.id DESC LIMIT $2",
    )
    .bind(torrent_id)
    .bind(limit.clamp(1, MAX_LIMIT))
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

pub async fn add_comment(
    db: &PgPool,
    torrent_id: i64,
    user_id: i64,
    body: &str,
) -> DomainResult<i64> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrents WHERE id = $1 AND approval_status = 1)",
    )
    .bind(torrent_id)
    .fetch_one(db)
    .await
    .unwrap_or(false);
    if !exists {
        return Err(DomainError::NotFound(torrent_id));
    }
    if body.trim().is_empty() {
        return Err(DomainError::Validation("评论不能为空".into()));
    }
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO comments (torrent_id, user_id, body) VALUES ($1, $2, $3) RETURNING id",
    )
    .bind(torrent_id)
    .bind(user_id)
    .bind(body)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(id)
}

pub async fn thank(db: &PgPool, torrent_id: i64, user_id: i64) -> DomainResult<()> {
    let res = sqlx::query(
        "INSERT INTO thanks (torrent_id, user_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(torrent_id)
    .bind(user_id)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if res.rows_affected() == 0 {
        return Err(DomainError::AlreadyThanked);
    }
    Ok(())
}

pub async fn bookmark(db: &PgPool, torrent_id: i64, user_id: i64, on: bool) -> DomainResult<()> {
    if on {
        sqlx::query(
            "INSERT INTO bookmarks (user_id, torrent_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
        )
        .bind(user_id)
        .bind(torrent_id)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    } else {
        sqlx::query("DELETE FROM bookmarks WHERE user_id = $1 AND torrent_id = $2")
            .bind(user_id)
            .bind(torrent_id)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    Ok(())
}

/// 种子编辑（NP takeedit.php 作者口径）：name/small_descr/descr/anonymous/category/medium/grade/edition
/// 修改后回退到待审核（approval_status=0），走审核流重新过审。
pub struct TorrentEdit<'a> {
    pub name: Option<&'a str>,
    pub small_descr: Option<&'a str>,
    pub descr: Option<&'a str>,
    pub anonymous: Option<bool>,
    pub category_id: Option<i32>,
    pub medium_id: Option<i32>,
    pub grade_id: Option<i32>,
    pub edition_id: Option<i32>,
}

pub async fn edit_torrent(
    db: &PgPool,
    torrent_id: i64,
    editor: (i64, i16), // (user_id, class_id)：作者本人或 staff（>=90）
    e: &TorrentEdit<'_>,
) -> DomainResult<()> {
    let owner: Option<i64> = sqlx::query_scalar("SELECT owner_id FROM torrents WHERE id = $1")
        .bind(torrent_id)
        .fetch_optional(db)
        .await
        .map_err(|err| DomainError::Internal(err.into()))?
        .flatten();
    let Some(owner_id) = owner else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if editor.1 < 90 && owner_id != editor.0 {
        return Err(DomainError::Forbidden);
    }
    let n = sqlx::query(
        r#"
        UPDATE torrents SET
            name = COALESCE($2, name),
            small_descr = COALESCE($3, small_descr),
            descr = COALESCE($4, descr),
            anonymous = COALESCE($5, anonymous),
            category_id = COALESCE($6, category_id),
            medium_id = COALESCE($7, medium_id),
            grade_id = COALESCE($8, grade_id),
            edition_id = COALESCE($9, edition_id),
            approval_status = 0,
            mtime = now()
        WHERE id = $1
        "#,
    )
    .bind(torrent_id)
    .bind(e.name)
    .bind(e.small_descr)
    .bind(e.descr)
    .bind(e.anonymous)
    .bind(e.category_id)
    .bind(e.medium_id)
    .bind(e.grade_id)
    .bind(e.edition_id)
    .execute(db)
    .await
    .map_err(|err| DomainError::Internal(err.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(torrent_id));
    }
    Ok(())
}

/// 种子软删除（NP delete.php 口径）：staff 或作者本人（未过审的可直接删；已过审的作者删除需 staff）
pub async fn delete_torrent(db: &PgPool, torrent_id: i64, actor: (i64, i16)) -> DomainResult<()> {
    let row: Option<(Option<i64>, i16)> =
        sqlx::query_as("SELECT owner_id, approval_status FROM torrents WHERE id = $1")
            .bind(torrent_id)
            .fetch_optional(db)
            .await
            .map_err(|err| DomainError::Internal(err.into()))?;
    let Some((owner_id, approval)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    let is_staff = actor.1 >= 90;
    let is_owner = owner_id == Some(actor.0);
    // staff 任意删；作者只能删自己未过审（pending/rejected）的种子
    if !is_staff && !(is_owner && approval != 1) {
        return Err(DomainError::Forbidden);
    }
    // 软删 + 清理关联促销（审计修复：促销残留会被计费/H&R 豁免回查误命中）
    let mut tx = db
        .begin()
        .await
        .map_err(|err| DomainError::Internal(err.into()))?;
    sqlx::query("UPDATE torrents SET approval_status = 3, mtime = now() WHERE id = $1")
        .bind(torrent_id)
        .execute(&mut *tx)
        .await
        .map_err(|err| DomainError::Internal(err.into()))?;
    sqlx::query("DELETE FROM promotions WHERE torrent_id = $1")
        .bind(torrent_id)
        .execute(&mut *tx)
        .await
        .map_err(|err| DomainError::Internal(err.into()))?;
    tx.commit()
        .await
        .map_err(|err| DomainError::Internal(err.into()))?;
    Ok(())
}

/// 恢复软删种子（approval_status 3 → 0 待审）：此前误删后只能直连数据库手工修数。
pub async fn restore_torrent(db: &PgPool, torrent_id: i64) -> DomainResult<()> {
    let n = sqlx::query(
        "UPDATE torrents SET approval_status = 0, mtime = now() WHERE id = $1 AND approval_status = 3",
    )
    .bind(torrent_id)
    .execute(db)
    .await
    .map_err(|err| DomainError::Internal(err.into()))?
    .rows_affected();
    if n == 0 {
        return Err(DomainError::NotFound(torrent_id));
    }
    Ok(())
}

/// 下载/做种记录（NP viewsnatches.php 口径）：snatches 联 users，活跃状态由 seeding/leeching 标记
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct SnatchRow {
    pub user_id: i64,
    pub username: String,
    pub uploaded: i64,
    pub downloaded: i64,
    pub seeded_seconds: i32,
    pub completed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub seeding: bool,
    pub leeching: bool,
    /// BT 客户端 UA（0098 viewsnatches「客户端」列）
    pub agent: String,
    /// 下载进度，万分比 0-10000（0098；做种恒 10000）
    pub progress: i32,
}

pub async fn list_snatches(db: &PgPool, torrent_id: i64) -> DomainResult<Vec<SnatchRow>> {
    sqlx::query_as::<_, SnatchRow>(
        "SELECT s.user_id, u.username, s.uploaded, s.downloaded, s.seeded_seconds, \
                s.completed_at, s.seeding, s.leeching, s.agent, s.progress \
         FROM snatches s JOIN users u ON u.id = s.user_id \
         WHERE s.torrent_id = $1 \
         ORDER BY s.completed_at DESC NULLS LAST LIMIT 100",
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))
}

/// NFO（NP viewnfo.php 口径）：纯文本返回。
/// 种子存在但 nfo 为 NULL 时返回 Ok(None)（无 NFO），而非 404——
/// 修复前 fetch_optional 展平后把「行存在列空」也当 NotFound。
pub async fn get_nfo(db: &PgPool, torrent_id: i64) -> DomainResult<Option<String>> {
    let row: Option<Option<String>> =
        sqlx::query_scalar("SELECT nfo FROM torrents WHERE id = $1 AND approval_status = 1")
            .bind(torrent_id)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    row.ok_or(DomainError::NotFound(torrent_id))
}

/// 请求补种（NP takereseed.php 口径）：
/// 仅限 seeders=0 的"死种"；向所有完成过下载的用户群发 PM；900 秒限频。
/// 返回通知人数。
pub async fn request_reseed(
    db: &PgPool,
    torrent_id: i64,
    requester: (i64, String),
) -> DomainResult<usize> {
    let row: Option<(i32, Option<chrono::DateTime<chrono::Utc>>, String)> = sqlx::query_as(
        "SELECT seeders, last_reseed, name FROM torrents WHERE id = $1 AND approval_status = 1",
    )
    .bind(torrent_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((seeders, last_reseed, name)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if seeders > 0 {
        return Err(DomainError::Validation("该种子仍有人做种，无需补种".into()));
    }
    if let Some(lr) = last_reseed {
        if chrono::Utc::now() - lr < chrono::Duration::seconds(900) {
            return Err(DomainError::Validation(
                "15 分钟内已发起过补种请求，请稍候".into(),
            ));
        }
    }
    // 完成过下载的用户（含发布者）
    let receivers: Vec<i64> = sqlx::query_scalar(
        "SELECT DISTINCT user_id FROM snatches WHERE torrent_id = $1 AND completed_at IS NOT NULL \
         UNION SELECT owner_id FROM torrents WHERE id = $1 AND owner_id IS NOT NULL",
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let subject = "【补种请求】".to_string();
    let body = format!(
        "用户 {} 请求补种：{}（/torrent/{}）。如果你保留了文件，欢迎重新做种，谢谢！",
        requester.1, name, torrent_id
    );
    for uid in &receivers {
        sqlx::query(
            "INSERT INTO messages (sender_id, receiver_id, subject, body) VALUES ($1, $2, $3, $4)",
        )
        .bind(requester.0)
        .bind(uid)
        .bind(&subject)
        .bind(&body)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query("UPDATE torrents SET last_reseed = now() WHERE id = $1")
        .bind(torrent_id)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(receivers.len())
}

/// 种子标签（T-04）：列出字典 + 该种子已打的标签
pub async fn list_tags(db: &PgPool, torrent_id: i64) -> DomainResult<serde_json::Value> {
    let dict: Vec<(i32, String, String)> =
        sqlx::query_as("SELECT id, name, kind FROM tag_dict ORDER BY id")
            .fetch_all(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    let mine: Vec<i32> =
        sqlx::query_scalar("SELECT tag_id FROM tags WHERE torrent_id = $1 ORDER BY tag_id")
            .bind(torrent_id)
            .fetch_all(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(serde_json::json!({ "dict": dict, "mine": mine }))
}

/// 打/去标签（作者或 staff；官种标签仅 staff 可打）
pub async fn tag_torrent(
    db: &PgPool,
    torrent_id: i64,
    actor: (i64, i16),
    tag_id: i32,
    on: bool,
) -> DomainResult<()> {
    let row: Option<(Option<i64>, String)> = sqlx::query_as(
        "SELECT owner_id, kind FROM torrents t JOIN tag_dict d ON d.id = $2 WHERE t.id = $1",
    )
    .bind(torrent_id)
    .bind(tag_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((owner_id, kind)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if actor.1 < 90 && owner_id != Some(actor.0) {
        return Err(DomainError::Forbidden);
    }
    if kind == "official" && actor.1 < 90 {
        return Err(DomainError::Forbidden); // 官种/官方标签仅 staff
    }
    if on {
        sqlx::query("INSERT INTO tags (torrent_id, tag_id) VALUES ($1, $2) ON CONFLICT DO NOTHING")
            .bind(torrent_id)
            .bind(tag_id)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    } else {
        sqlx::query("DELETE FROM tags WHERE torrent_id = $1 AND tag_id = $2")
            .bind(torrent_id)
            .bind(tag_id)
            .execute(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    Ok(())
}

/// 付费下载扣费（0086，NP 价格 口径）：下载前调用。
/// 免费/发布者本人/已购 → 直接放行；否则原子扣款 → 发布者得 (100-税)% → 税入当月魔法池。
/// 全程单事务，余额不足返回校验错误。
/// 审计修复（P0 错账）：旧版直接 UPDATE users.spark_balance、不写 spark_ledger ——
/// 买方扣款与发布者入账都被每小时「余额=流水重算」回滚（下载变免费/收益被抹除），
/// 且 /admin/spark-logs 完全看不到这类变动。改为流水驱动（幂等键绑定 torrent+user）。
pub async fn charge_for_download(db: &PgPool, user_id: i64, torrent_id: i64) -> DomainResult<()> {
    let row: Option<(i64, Option<i64>)> = sqlx::query_as(
        "SELECT price, owner_id FROM torrents WHERE id = $1 AND approval_status = 1",
    )
    .bind(torrent_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((price, owner_id)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if price <= 0 || owner_id == Some(user_id) {
        return Ok(());
    }
    let purchased: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM torrent_purchases WHERE user_id = $1 AND torrent_id = $2)",
    )
    .bind(user_id)
    .bind(torrent_id)
    .fetch_one(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if purchased {
        return Ok(());
    }
    let tax: i32 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value::int FROM site_settings WHERE name = 'upload_price_tax'), 30)",
    )
    .fetch_one(db)
    .await
    .unwrap_or(30)
    .clamp(0, 90);
    let net = price * (100 - tax as i64) / 100;
    let tax_amount = price - net;
    let month = chrono::Utc::now().format("%Y-%m").to_string();

    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 余额校验（锁行）：不足直接拦下，不发流水
    let balance: i64 =
        sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    if balance < price {
        return Err(DomainError::Validation(format!(
            "魔力不足：该种子为付费种子（{price} 魔力），请先充值或签到攒魔力"
        )));
    }
    // 买方扣款流水（幂等键含 torrent：同一种子只扣一次，重放安全）
    sqlx::query(
        "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
         VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'torrent_buy', 'torrent', $3, $4, $5)",
    )
    .bind(user_id)
    .bind(-price)
    .bind(torrent_id)
    .bind(format!("torrent-buy:{user_id}:{torrent_id}"))
    .bind(balance - price)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 发布者入账流水（owner_id 经 torrents FK 保证非空语义；净得 = 价 - 税）
    if let Some(owner) = owner_id {
        let obal: i64 =
            sqlx::query_scalar("SELECT spark_balance FROM users WHERE id = $1 FOR UPDATE")
                .bind(owner)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?
                .unwrap_or(0);
        sqlx::query(
            "INSERT INTO spark_ledger (id, user_id, amount, kind, ref_type, ref_id, idempotency_key, balance_after) \
             VALUES (nextval('spark_ledger_id_seq'), $1, $2, 'torrent_sell', 'torrent', $3, $4, $5)",
        )
        .bind(owner)
        .bind(net)
        .bind(torrent_id)
        .bind(format!("torrent-sell:{user_id}:{torrent_id}"))
        .bind(obal + net)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let _ = obal; // 余额快照由下方 UPDATE / 小时级重算收敛
    }
    if tax_amount > 0 {
        sqlx::query(
            "INSERT INTO magic_pool (month, donated_total) VALUES ($1, $2) \
             ON CONFLICT (month) DO UPDATE SET donated_total = magic_pool.donated_total + $2",
        )
        .bind(&month)
        .bind(tax_amount)
        .execute(&mut *tx)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sqlx::query(
        "INSERT INTO torrent_purchases (user_id, torrent_id, price) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(torrent_id)
    .bind(price)
    .execute(&mut *tx)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}

/// 站点统计（M10 概览，旧站首页口径）
pub async fn site_stats(db: &PgPool) -> DomainResult<serde_json::Value> {
    let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users WHERE status < 2")
        .fetch_one(db)
        .await
        .unwrap_or(0);
    let torrents: i64 =
        sqlx::query_scalar("SELECT count(*) FROM torrents WHERE approval_status = 1")
            .fetch_one(db)
            .await
            .unwrap_or(0);
    let dead: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM torrents WHERE approval_status = 1 AND seeders = 0",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    let seed_size: i64 = sqlx::query_scalar(
        "SELECT COALESCE(sum(t.size), 0) FROM torrents t \
         JOIN (SELECT DISTINCT torrent_id FROM snatches WHERE seeding) s ON s.torrent_id = t.id",
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    Ok(serde_json::json!({
        "users": users, "torrents": torrents, "dead": dead, "seed_size": seed_size
    }))
}
