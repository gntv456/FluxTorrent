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
    /// IMDB id（0148 C1：种子页字幕面板按此合并同片字幕）
    pub imdb_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    /// 置顶权重（默认排序的游标键，方案批次二）：仅列表 SELECT 输出，
    /// 详情等其它用本行的查询不选该列 → sqlx(default) 落 0，互不影响。
    #[sqlx(default)]
    pub sticky_rank: i32,
    /// 行内标签徽标（0159 P1）：随行 json_agg 的 [{id,name,kind,bg_color,color}]，
    /// 按 sort DESC, id 排序；其它查询不选该列 → sqlx(default) 落空数组
    #[sqlx(default)]
    pub tags: serde_json::Value,
}

/// 列表游标（方案批次二）：id 之外携带排序键值，修复「非默认排序翻页丢行」。
///
/// 旧实现翻页只比 `t.id < cursor`，而排序键是「置顶 + 排序列 + id」——
/// 按做种数排序时 id 更大但排得更前的种子永远翻不出来。
/// 编码：新格式 `{hex(sortval)}~{id}`；旧格式（纯数字 id，历史链接/书签）兼容解析为
/// `val=None`，此时谓词退化为回到第一页（诚实行为：旧游标在排序语义下本就不可靠）。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ListCursor {
    pub id: i64,
    pub val: Option<String>,
}

impl ListCursor {
    pub fn encode(val: Option<&str>, id: i64) -> String {
        match val {
            Some(v) => {
                let hex: String =
                    v.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
                format!("{hex}~{id}")
            }
            None => id.to_string(),
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        if let Some((hex, id)) = s.split_once('~') {
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
                .collect::<Option<Vec<u8>>>()?;
            let val = String::from_utf8(bytes).ok()?;
            if val.is_empty() {
                return None;
            }
            Some(Self {
                id: id.parse().ok()?,
                val: Some(val),
            })
        } else {
            // 旧格式：纯数字 = 只有 id（旧行为的默认排序游标）
            s.parse::<i64>().ok().map(|id| Self { id, val: None })
        }
    }
}

/// 详情页扩展字段（简介/文件数/感谢数；列表不需要，独立查询）
/// Deserialize：aggregate 共享段 Redis 缓存反序列化（2.5 详情对象缓存）
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
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
/// Deserialize：aggregate 共享段缓存反序列化用
#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
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
    /// 标签筛选（0159 P1 起多选）：tag_dict.id 数组，any=任一命中 / all=全部命中；
    /// 旧单值 tag_id 在入口并入此数组（any 语义）
    pub tag_ids: Option<Vec<i32>>,
    /// 多选匹配模式：true = all（每个标签都要命中）；false/缺省 = any
    pub tag_all: bool,
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
    /// 优惠筛选：free=免费(含 2x 免费) x2=2倍 half=半价 any=任意优惠 none=无优惠；
    /// 阶段三起支持多选（逗号串如 `free,x2`，各档 OR）
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
    /// 仅看我书签收藏的种（viewer 维度，阶段三筛选粒度补齐）
    #[serde(default)]
    pub bookmarked: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TorrentPage {
    pub items: Vec<TorrentRow>,
    pub next_cursor: Option<String>,
    pub total_estimate: i64,
}

/// 单页上限（方案阶段二：列表支持 20/50/100 每页可选，故上限从 50 提到 100；
/// 深翻页的安全阀由游标 + 首页条数决定，不再靠压低上限）
pub(super) const MAX_LIMIT: i64 = 100;

#[cfg(test)]
mod cursor_tests {
    use super::ListCursor;

    /// 编解码 round-trip：排序键值（含中文/冒号/等号）不能丢，id 不能丢。
    #[test]
    fn encode_parse_roundtrip() {
        for val in [
            "5",
            "2026-09-22T04:00:00.123456+00:00",
            "物理 实验",
            "a=b:c",
        ] {
            let c = ListCursor::encode(Some(val), 42);
            assert!(c.contains('~'), "新格式必须带分隔符: {c}");
            let back = ListCursor::parse(&c).expect("解析失败");
            assert_eq!(
                back,
                ListCursor {
                    id: 42,
                    val: Some(val.to_string())
                }
            );
        }
    }

    /// 旧格式（纯数字 id，历史链接/书签）兼容解析为 val=None，不得报错。
    #[test]
    fn legacy_cursor_parses_to_none_val() {
        let c = ListCursor::parse("98765").expect("旧格式解析失败");
        assert_eq!(
            c,
            ListCursor {
                id: 98765,
                val: None
            }
        );
    }

    /// 非法输入一律 None（handler 转成 400），空键值不算合法游标。
    #[test]
    fn invalid_inputs_rejected() {
        assert!(ListCursor::parse("abc").is_none());
        assert!(ListCursor::parse("~12").is_none(), "空键值应拒绝");
        assert!(ListCursor::parse("6162~notanumber").is_none());
        assert!(ListCursor::parse("zz~12").is_none(), "非十六进制应拒绝");
    }

    /// keyset 语义自检：编码值按字典序（hex）不可直接比大小不重要——
    /// 谓词在 SQL 侧按列 cast 比较，这里只锁「同值同码」的确定性。
    #[test]
    fn encoding_is_deterministic() {
        let a = ListCursor::encode(Some("5"), 1);
        let b = ListCursor::encode(Some("5"), 1);
        assert_eq!(a, b);
        assert_ne!(
            ListCursor::encode(Some("5"), 1),
            ListCursor::encode(Some("6"), 1)
        );
    }
}
