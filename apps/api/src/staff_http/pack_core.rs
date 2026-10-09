//! 站型包应用核心（物化事务体）。
//! 从 pack_apply.rs 拆出：setup 向导与后台 apply 端点共用同一条物化链路
//! （二审 G6：向导此前只应用 extras 一段，分类/模块/维度/标签/字幕口径全部不落地）。

use super::pack_types::is_ascii_kind;
use super::sitetype::SiteTypePack;
use crate::errors::{DomainError, DomainResult};
use sqlx::PgPool;

/// home_sections 段的键白名单（H10/0322）：与首页排版保存端点同一清单
/// （http/home_layout.rs 的 HOME_SECTIONS）——包声明的键必须在此内，
/// 否则整条跳过（非法键不进 home_layout）。
fn is_known_home_key(k: &str) -> bool {
    [
        "news",
        "attendance",
        "shoutbox",
        "funbox",
        "resource_stats",
        "site_data",
        "lucky_draw",
        "links",
        "latest",
    ]
    .contains(&k)
}

/// 应用一个站型包的完整物化（事务体 + 提交后回调）。
/// 返回 (categories 数, extras 应用清单)。
///
/// mode 语义与既有 apply 端点一致：replace = 清空分类重建（有种子时拒绝）；
/// merge = 保留现有分类，同 key（0317 起跨包唯一；无 key 载荷回落同 id）
/// 覆盖、新 key 追加；restore = 回滚快照重放。
/// 注意：tagline 不在此处重置（0145 覆盖语义——站长自定义值保留，见 pack_apply 调用方）。
pub(crate) async fn apply_pack_full(
    db: &PgPool,
    pack: &SiteTypePack,
    mode: &str,
) -> DomainResult<(i64, Vec<(String, i64)>)> {
    let mut tx = db
        .begin()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let cats = pack.categories.as_array().cloned().unwrap_or_default();
    if mode == "replace" {
        let used: i64 = sqlx::query_scalar("SELECT count(*) FROM torrents")
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(0);
        if used > 0 {
            // 有种子时禁止整表重建（避免悬挂引用）：提示改用 merge
            return Err(DomainError::Validation(
                "站点已有种子，replace 会悬挂引用；请使用 merge 模式（保留现有分类，追加新分类）"
                    .into(),
            ));
        }
        sqlx::query("DELETE FROM categories")
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    // key 行的 id 由应用侧分配（0317 H1）：包里的 id 只是「空表 replace 时的
    // 建议序号」，不能拿来直接 INSERT——跨包切换时旧行占着同号主键，会撞
    // categories_pkey。max+1 分配在事务内逐行递增；apply 是 sysop 级低频
    // 操作且 replace 已清表，无并发重号风险。
    let mut next_id: i32 =
        sqlx::query_scalar("SELECT COALESCE(max(id), 0) + 1 FROM categories")
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    for (i, c) in cats.iter().enumerate() {
        let id = c
            .get("id")
            .and_then(serde_json::Value::as_i64)
            .unwrap_or(i as i64 + 1) as i32;
        let name = c
            .get("name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("")
            .to_string();
        if name.is_empty() {
            continue;
        }
        // 跨包稳定身份（H1/0317）：声明了 key 的分类按 key 匹配——不同包的
        // 分类互不覆盖（key 全局唯一），切站型只会追加新行，不再把老分类
        // 原地改名偷换在用种子的语义；id 退化为内部主键。老载荷无 key 时
        // 保持同 id 覆盖的旧语义（restore 快照重放也走这条）。
        let key = c
            .get("key")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("");
        // 图标：包可带 icon_key；未带按 0166 模式表推断（现有非空优先；restore 快照优先）。
        // 层级/排序（G20）：包声明了才写（COALESCE 守旧值），载荷保证父先于子。
        let icon = c.get("icon_key").and_then(|v| v.as_str()).unwrap_or("");
        // 分类色（0183/0317 H6）：包声明了才写（merge 守站长改过的值；
        // restore 整段以快照为准覆盖）
        let bg = c.get("bg_color").and_then(|v| v.as_str()).unwrap_or("");
        // key 唯一性是表达式索引（NULLIF(key,'')，空串视同 NULL 不冲突），
        // ON CONFLICT 必须写表达式推断形式；无 key 载荷回落主键 id
        let conflict_target = if key.is_empty() {
            "id"
        } else {
            "((NULLIF(key, '')))"
        };
        // 对外分类号（批次 3a/0325）：Newznab/Torznab 出口按此归位，
        // 缺号全部回落 Other(8000)。merge 守站长改过的值；restore 按快照。
        let nz = c
            .get("newznab_id")
            .and_then(|v| v.as_i64())
            .map(|n| n as i32);
        let sql = format!(
            "INSERT INTO categories (id, key, name, icon_key, parent_id, sort, \
             bg_color, newznab_id) \
             VALUES (CASE WHEN $6 = '' THEN $1 ELSE $9 END, NULLIF($6, ''), $2, \
             COALESCE(NULLIF($3, ''), pick_category_icon($2)), $4, \
             COALESCE($5, 100), NULLIF($8, ''), $10) \
             ON CONFLICT ({conflict_target}) DO UPDATE SET name = EXCLUDED.name, \
             parent_id = COALESCE($4, categories.parent_id), \
             sort = COALESCE($5, categories.sort), icon_key = COALESCE( \
             NULLIF(CASE WHEN $7 THEN $3 ELSE categories.icon_key END, ''), \
             NULLIF(categories.icon_key, ''), NULLIF($3, ''), \
             pick_category_icon($2)), \
             bg_color = CASE WHEN $7 THEN EXCLUDED.bg_color \
             ELSE COALESCE(EXCLUDED.bg_color, categories.bg_color) END, \
             newznab_id = CASE WHEN $7 THEN EXCLUDED.newznab_id \
             ELSE COALESCE(categories.newznab_id, EXCLUDED.newznab_id) END"
        );
        sqlx::query(&sql)
            .bind(id)
            .bind(&name)
            .bind(icon)
            .bind(
                c.get("parent_id")
                    .and_then(|v| v.as_i64())
                    .map(|p| p as i32),
            )
            .bind(c.get("sort").and_then(|v| v.as_i64()).map(|s| s as i32))
            .bind(key)
            .bind(mode == "restore")
            .bind(bg)
            .bind(next_id)
            .bind(nz)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        next_id += 1;
    }
    // 计数用实际写入行数（H5）：载荷长度在「包行被跳过/合并」时虚报成功
    let added: i64 = cats
        .iter()
        .filter(|c| {
            c.get("name")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|n| !n.is_empty())
        })
        .count() as i64;
    // site_type + 品牌默认
    sqlx::query("INSERT INTO site_settings (name, value) VALUES \
     ('site_type', $1) ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, \
     updated_at = now()")
        .bind(&pack.code).execute(&mut *tx).await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // 站名守卫（H3）：brand 为空不写——site_name 是站名权威键（0214 收敛），
    // 11 个预置包 brand 全空串，无条件写会把整站抬头清成空。restore 回滚
    // 走 snapshot_to_pack 的 brand（快照里存的是当时的真实站名），同样适用。
    if !pack.brand.trim().is_empty() {
        sqlx::query("INSERT INTO site_settings (name, value) VALUES \
         ('site_name', $1) ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, \
         updated_at = now()")
            .bind(pack.brand.trim()).execute(&mut *tx).await
            .map_err(|e| DomainError::Internal(e.into()))?;
    }
    // 模块开关 → 站点设定键（textbooks 等）。
    // H4 覆盖语义：站长手工设置过的键登记在 pack_module_overrides（0318），
    // apply 跳过——「我只想做音乐站」的手工关闭不再被切站型无声复位。
    // restore（回滚快照重放）不受限：快照本身就是站长状态。
    if let Some(mods) = pack.modules.as_object() {
        for (k, v) in mods {
            let val = if v.as_bool().unwrap_or(false) {
                "yes"
            } else {
                "no"
            };
            if mode != "restore" {
                let overridden: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM pack_module_overrides \
                     WHERE module_key = $1)",
                )
                .bind(k)
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(false);
                if overridden {
                    continue;
                }
            }
            sqlx::query(
                "INSERT INTO site_settings (name, value) \
                 VALUES ($1, $2) ON CONFLICT (name) DO UPDATE SET value = \
                 EXCLUDED.value, updated_at = now()",
            )
            .bind(format!("module_{k}"))
            .bind(val)
            .execute(&mut *tx)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
        }
    }
    // 质量维度种子（0092）：包内定义的维度重建标签与选项。
    // 0101 修复：切换站型后旧站型的内置维度残留——**声明了 sections 的包**里，
    // 内置九维中未被本包定义的维度整体移除；站方自建维度原样保留；
    // 未声明 sections 的包不动任何维度。
    let builtin: std::collections::HashSet<String> = [
        "media",
        "grades",
        "editions",
        "codec",
        "audio_codec",
        "standard",
        "source",
        "processing",
        "team",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    let mut packed_kinds: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    // 包是否**声明了** sections：NULL = 本包不管质量维度。
    let pack_declares_sections = pack
        .sections
        .as_ref()
        .and_then(serde_json::Value::as_object)
        .is_some();
    if let Some(sections) = pack
        .sections
        .as_ref()
        .and_then(serde_json::Value::as_object)
    {
        if let Some(kinds) =
            sections.get("kinds").and_then(serde_json::Value::as_array)
        {
            for k in kinds {
                if let Some(kind) =
                    k.get("kind").and_then(serde_json::Value::as_str)
                {
                    packed_kinds.insert(kind.to_string());
                }
            }
        }
        // dict 里出现的维度同样是「本包已定义」——只看 kinds 会把它先删后建，
        // 级联清掉 torrent_sections 引用并换掉 dict id。
        if let Some(dict) =
            sections.get("dict").and_then(serde_json::Value::as_object)
        {
            for kind in dict.keys() {
                packed_kinds.insert(kind.clone());
            }
        }
    }
    for kind in &builtin {
        // 包压根没声明 sections 时，九维一个都不能删——否则切到 NULL 包，
        // 维度连同字典被级联清空，多维分类整体失效（0174 事故）。
        if pack_declares_sections && !packed_kinds.contains(kind) {
            // 引用中的维度直接删会级联清 torrent_sections —— 有种子的站点会丢筛选项，
            // 这里先检查是否被在用：被在用时跳过清理（宁残留不破坏）
            let in_use: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM torrent_sections WHERE kind = $1",
            )
            .bind(kind)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(0);
            if in_use == 0 {
                sqlx::query("DELETE FROM section_kinds WHERE kind = $1")
                    .bind(kind)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
            }
        }
    }
    if let Some(sections) = pack
        .sections
        .as_ref()
        .and_then(serde_json::Value::as_object)
    {
        if let Some(kinds) =
            sections.get("kinds").and_then(serde_json::Value::as_array)
        {
            super::pack_types::apply_pack_kinds(&mut tx, kinds).await?;
        }
        if let Some(dict) =
            sections.get("dict").and_then(serde_json::Value::as_object)
        {
            for (kind, names) in dict {
                if !is_ascii_kind(kind) {
                    continue;
                }
                // 维度可能未在包 kinds 中定义（自定义维度追加选项）：确保存在
                sqlx::query(
                    "INSERT INTO section_kinds (kind, \
                     label, sort) VALUES ($1, $1, 999) ON CONFLICT (kind) DO \
                     NOTHING",
                )
                .bind(kind)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                let packed: Vec<String> = names
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|n| n.as_str().map(|s| s.to_string()))
                    .collect();
                // 该维度被种子引用（无论引用的是包内还是包外选项——整维重建会
                // 换 dict_id，级联清掉 torrent_sections；二审 G7b：与生态商店
                // 导入路径同口径，退化追加而非重建。无引用时整维重建安全）。
                let in_use: i64 = sqlx::query_scalar(
                    "SELECT count(*) FROM torrent_sections ts \
                     JOIN section_dict sd ON sd.id = ts.dict_id \
                     WHERE sd.kind = $1",
                )
                .bind(kind)
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(0);
                if in_use > 0 {
                    for name in &packed {
                        sqlx::query(
                            "INSERT INTO section_dict (kind, name, sort) \
                             SELECT $1, $2, 999 WHERE NOT EXISTS (\
                               SELECT 1 FROM section_dict WHERE kind = $1 \
                               AND name = $2)",
                        )
                        .bind(kind)
                        .bind(name)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| DomainError::Internal(e.into()))?;
                    }
                    continue;
                }
                sqlx::query("DELETE FROM section_dict WHERE kind = $1")
                    .bind(kind)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                for (i, name) in names
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .enumerate()
                {
                    let Some(name) = name.as_str() else { continue };
                    sqlx::query(
                        "INSERT INTO section_dict \
                     (kind, name, sort) VALUES ($1, $2, $3)",
                    )
                    .bind(kind)
                    .bind(name)
                    .bind((i + 1) as i32)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                }
            }
        }
    }
    // ===== 标签数据节（0160 P2）：pack 层重建，global 层硬性不触碰 =====
    super::pack_tags::apply_pack_tags(&mut tx, pack).await?;
    // 首页排版（H10/0322）：包声明 home_sections 时写入 site_settings.home_layout。
    // 站长守卫与 tagline 0145 同口径——只改「空值或等于任一预置包默认」的现值，
    // 手工排版不被切站型复位；restore 回滚走快照重放不受限（快照存的就是
    // apply 前的排版）。
    if let Some(layout) = pack
        .home_sections
        .as_ref()
        .and_then(serde_json::Value::as_array)
    {
        if !layout.is_empty() && mode != "restore" {
            let norm: Vec<String> = layout
                .iter()
                .filter_map(|it| {
                    let key = it.get("key").and_then(|v| v.as_str())?;
                    let span =
                        it.get("span").and_then(|v| v.as_i64()).unwrap_or(0);
                    if !is_known_home_key(key) || !(0..=3).contains(&span) {
                        return None;
                    }
                    Some(format!(r#"{{"key":"{key}","span":{span}}}"#))
                })
                .collect();
            if !norm.is_empty() {
                let value = format!("[{}]", norm.join(","));
                sqlx::query(
                    "INSERT INTO site_settings (name, value, descr, grp) \
                     SELECT 'home_layout', $1, '首页板块排版（JSON 数组，\
                     空 = 默认布局）', 'main' \
                     WHERE NOT EXISTS (SELECT 1 FROM site_settings WHERE \
                     name = 'home_layout' AND value <> '' AND value NOT IN \
                     (SELECT COALESCE(home_sections::text, '') FROM \
                     site_type_packs WHERE jsonb_typeof(home_sections) = \
                     'array')) \
                     ON CONFLICT (name) DO UPDATE SET value = EXCLUDED.value, \
                     updated_at = now()",
                )
                .bind(&value)
                .execute(&mut *tx)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
            }
        }
    }
    tx.commit()
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    // U2（0108）：等级叙事/经济预设/元数据源（apply_pack_extras 过程内含合法键校验）
    let extras: Vec<(String, i64)> =
        sqlx::query_as("SELECT kind, applied FROM apply_pack_extras($1)")
            .bind(&pack.code)
            .fetch_all(db)
            .await
            .unwrap_or_default();
    // 字幕区口径（0146/0178）：包快照显式指定则按快照（custom_* 不再按 code 猜，
    // 二审 G7d）；未指定回落按 code 的既有规则。
    let is_lyric: Option<bool> = match pack.subtitle_kind.as_deref() {
        Some("lyric") => Some(true),
        Some("subtitle") => Some(false),
        _ => None,
    };
    match is_lyric {
        Some(v) => {
            let _ = sqlx::query("SELECT apply_subtitle_kind_explicit($1)")
                .bind(v)
                .execute(db)
                .await;
        }
        None => {
            let _ = sqlx::query("SELECT apply_subtitle_kind($1)")
                .bind(&pack.code)
                .execute(db)
                .await;
        }
    }
    Ok((added, extras))
}
