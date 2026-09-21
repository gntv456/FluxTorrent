//! 种子行类型与过滤器（M02）：TorrentRow/DetailRow/FileRow/ThankRow/TorrentFilter/TorrentPage。
//! 从 torrents.rs 按域拆出。

use serde::{Deserialize, Serialize};

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

pub(super) const MAX_LIMIT: i64 = 50;
