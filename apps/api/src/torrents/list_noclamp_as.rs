//! 免钳制列表核心（M02 管理面）：noclamp_as 直查（含全部谓词构造）。
//! 从 torrents/list_noclamp.rs 按域拆出。

use super::cursor;
use super::list::{esc_like, extra_preds, ExtraSlots};
use super::promo;
use super::types::{ListCursor, TorrentFilter, TorrentPage, TorrentRow};
use crate::errors::{DomainError, DomainResult};
use sqlx::PgPool;
/// 审计修复（P1）：list_torrents_as 此前把 viewer 转成字符串后丢弃，内部又硬编码
/// viewer_sql="0"，status=seeding/leeching/... 全部按「无人」匹配恒空。现在把
/// viewer 一路传到 SQL 构造（i64 内插无注入面）。
pub async fn list_torrents_noclamp_as(
    db: &PgPool,
    filter: &TorrentFilter,
    cursor: Option<ListCursor>,
    limit: i64,
    viewer: i64,
) -> DomainResult<TorrentPage> {
    let limit = limit.max(1);
    let viewer_sql = viewer.to_string();
    // 匹配模式（旧站 torrents.php 口径）：0/缺省 = AND 模糊；2 = 精确等值（不带通配）
    let exact = filter.search_mode == Some(2);
    let mk_pat = |s: &str| {
        if exact {
            s.to_string()
        } else {
            format!("%{}%", esc_like(s))
        }
    };
    let pattern = filter.search.as_deref().map(mk_pat);
    // 排除词/发布者：与关键字同口径（模糊 + 转义）
    let fuzz = |v: &Option<String>| {
        v.as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(|s| format!("%{}%", esc_like(s)))
    };
    let exclude_pat = fuzz(&filter.exclude);
    let owner_pat = fuzz(&filter.owner);
    // 标题必须命中（0267）：仅当调用方显式给了 title_like 才生成谓词
    let title_pat = fuzz(&filter.title_like);

    // 置顶口径（0063/0089 二级置顶）：pos_state 1=一级 2=二级（until 到期回落），
    // 旧列 sticky 仍被官种联动——「任一生效即置顶」，一级 > 二级 > 普通置顶。
    // 拆出无方向 sticky_calc：游标谓词要比较它、SELECT 也要输出（sticky_rank）。
    let sticky_calc = "(GREATEST(t.sticky::int, CASE WHEN t.pos_state IN (1, 2) AND (t.pos_state_until IS NULL OR t.pos_state_until > now()) THEN CASE t.pos_state WHEN 1 THEN 2 WHEN 2 THEN 1 ELSE 0 END ELSE 0 END))";
    // 0170 双向翻页：dir=prev 时排序方向整体反转（置顶表达式含方向，一并反转）
    let sticky_dir = if filter.reverse { "ASC" } else { "DESC" };
    let sticky_expr = format!("{sticky_calc} {sticky_dir}");
    // 表头排序（NP colhead 口径）：asc 前缀反转方向；comments = 评论数
    let (key, asc) = match filter.sort.as_deref() {
        Some(s) if s.ends_with("_asc") => (&s[..s.len() - 4], true),
        other => (other.unwrap_or(""), false),
    };
    // 0170：反向页的有效排序方向 = 原方向 XOR reverse（ORDER BY / 游标谓词同反）
    let asc_eff = asc != filter.reverse;
    // 用户显式排序不掺置顶 + 二元 keyset 游标：构造细节见 cursor.rs
    let (order, cursor_col) = cursor::sort_of(key, asc_eff, &sticky_expr);
    // 标签筛选（0159 P1 多选）：谓词按参数槽位生成（列表 $9 / 计数 $8）。
    let tag_pred =
        super::tag_pred::tag_pred(&filter.tag_ids, filter.tag_all, 9);
    let tag_pred_count =
        super::tag_pred::tag_pred(&filter.tag_ids, filter.tag_all, 8);
    let sec_sql =
        super::section_pred::section_where(db, &filter.sections).await; // 搜索范围分流（旧站口径）：0=标题+全字段(默认) 1=副标题/简介 3=发布者 4=IMDb
                                                                        // 搜索谓词 + 同义词扩展（E9）：合成收在 syn_search（含白名单防注入说明）
    let search_pred = super::syn_search::search_pred_with_syn(
        db,
        filter.search.as_deref(),
        filter.search_area,
        exact,
        if exact { "" } else { " ESCAPE chr(92)" },
    )
    .await;

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
    // 审核状态（0102 + 0170 站点开关）：默认视图 = 已通过；开关放行待审/被拒
    // （列表带徽标）。集合由白名单枚举拼接常量文本，无注入面。
    // 被拒视图含软删 3 维持原口径；外层闸门与计数侧同口径（见 approval_gate）。
    let allowed: Vec<i16> = match filter.approval {
        Some(2) => vec![2, 3],
        Some(1) => vec![1],
        _ => {
            let mut set = vec![1];
            if filter.show_pending {
                set.insert(0, 0);
            }
            if filter.show_rejected {
                set.push(2);
            }
            set
        }
    };
    let in_list = allowed
        .iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    let approval_pred = format!(" AND t.approval_status IN ({in_list})");
    // 外层闸门：include_unapproved（staff see_banned）放行全部之外，开关放行的
    // 默认集合也须放行 —— 否则开关开了仍被 (status=1 OR ...) 滤掉。
    // 列表侧外层参数是 $10，计数侧是 $7（两处 bind 序号不同，分别生成）。
    let approval_gate =
        format!("(t.approval_status IN ({in_list}) OR $10::bool)");
    let approval_gate_count =
        format!("(t.approval_status IN ({in_list}) OR $7::bool)");
    // 种子状态 + 书签（viewer 维度谓词，阶段三筛选粒度收编 viewer_preds）
    let status_pred =
        super::viewer_preds::status_pred(filter.status.as_deref(), &viewer_sql);
    let bookmark_pred = if filter.bookmarked {
        super::viewer_preds::bookmark_pred(&viewer_sql)
    } else {
        String::new()
    };

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
        // 标题谓词排在游标排序键（$26）之后，避免改动 cursor.rs 的固定编号
        title_like: 27,
        // 评分下界（0283 P1-5）：列表侧最后一位
        rating_min: 28,
    });
    // 促销两列（kind + 到期）：一次 LATERAL 取「命中的最高优先级促销」，
    // 替代此前两条除 SELECT 列外完全相同的 correlated 子查询（方案 P0-5，每行省一次 promotions 扫描）。
    let promo_lateral = promo::lateral_latest();
    // 旧三列筛选走双路：只写了 sections 的种子也要能被「按媒介/学段/版本」筛到
    // （legacy_filter_dual 的返回串不含花括号，可安全嵌进 format!）
    let medium_pred = super::section_pred::legacy_filter_dual("media", "$2")
        .unwrap_or_else(|| "($2::int IS NULL OR t.medium_id = $2)".into());
    let grade_pred = super::section_pred::legacy_filter_dual("grades", "$3")
        .unwrap_or_else(|| "($3::int IS NULL OR t.grade_id = $3)".into());
    let edition_pred =
        super::section_pred::legacy_filter_dual("editions", "$4")
            .unwrap_or_else(|| "($4::int IS NULL OR t.edition_id = $4)".into());
    // 游标谓词（方案批次二）：$7 = 游标 id，$26 = 排序键值（文本进、按列 cast 比较）。
    // 二元 keyset「(排序列, id)」与 ORDER BY 完全同序，非默认排序翻页不再丢行；
    // 旧格式游标（val=None，历史链接）sortval 为 NULL → 退化为回到第一页。
    let sortval = cursor.as_ref().and_then(|c| c.val.clone());
    let cursor_pred = match cursor.as_ref() {
        Some(_) => {
            cursor::predicate(&cursor_col, asc_eff, sticky_calc, filter.reverse)
        }
        None => String::new(),
    };
    let sql = format!(
        r#"
        SELECT t.id, t.info_hash, t.pieces_hash, t.name, t.small_descr, t.category_id, t.medium_id,
               t.grade_id, t.edition_id, t.size, t.seeders, t.leechers, t.times_completed,
               (SELECT count(*) FROM comments c WHERE c.torrent_id = t.id) AS comments,
               t.official_tag, t.anonymous, t.approval_status, t.sticky,
               CASE WHEN t.anonymous THEN NULL ELSE u.username END AS owner_name,
               pr.promotion,
               pr.promotion_ends_at,
               t.media_info->>'rating' AS rating,
               t.media_info->>'poster' AS poster,
               t.imdb_id,
               t.created_at,
               (SELECT COALESCE(json_agg(json_build_object(
                    'id', d.id, 'name', d.name, 'kind', d.kind,
                    'bg_color', d.bg_color, 'color', d.color)
                    ORDER BY d.sort DESC, d.id), '[]'::json)
                FROM tags tg JOIN tag_dict d ON d.id = tg.tag_id
                WHERE tg.torrent_id = t.id) AS tags,
               -- B2：自由值行（dict_id IS NULL）不在 section_dict 里，内连接会把
               -- 它们从列表里静默丢掉；形状不变，自由值取 `#>> '{{}}'` 标量原文
               -- （`::text` 会把 JSON 字符串连引号吐出），存量消费点零改动。
               (SELECT COALESCE(json_agg(
                        COALESCE(x.name, ts.value #>> '{{}}')
                        ORDER BY k.sort, k.kind, ts.ordinal), '[]'::json)
                FROM torrent_sections ts
                LEFT JOIN section_dict x ON x.id = ts.dict_id
                LEFT JOIN section_kinds k ON k.kind = ts.kind
                WHERE ts.torrent_id = t.id) AS sec_names,
               {sticky_calc} AS sticky_rank
        FROM torrents t
        LEFT JOIN users u ON u.id = t.owner_id
        {promo_lateral}
        WHERE {approval_gate}
          AND ($1::int[] IS NULL OR t.category_id = ANY($1))
          AND {medium_pred}
          AND {grade_pred}
          AND {edition_pred}
          AND ($5::bool IS NULL OR t.official_tag = $5)
          {alive_pred}
          {approval_pred}
          {status_pred}
          {bookmark_pred}
          {search_pred}
         {cursor_pred}
          {tag_pred}
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
        .bind(cursor.as_ref().map(|c| c.id))
        .bind(limit + 1)
        .bind(filter.tag_ids.clone())
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
        .bind(sortval)
        // $27：标题谓词（extra_sql 里 title_like 槽位；排在 sortval 之后）
        .bind(&title_pat)
        // $28：评分下界（0283 P1-5）
        .bind(filter.rating_min)
        .fetch_all(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;

    // 计数与列表同口径（0093）：此前 count 不计多维筛选/标签/搜索范围——
    // 页面会显示「共 N 个」却零行。计数侧额外谓词编号 $9..$18（0118 后 $19..$23）
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
        // 计数侧（$24 起；0283 P1-5 评分下界再 +1）
        title_like: 24,
        rating_min: 25,
    });
    // 计数降级（方案 P0-3）：此前每次列表请求都对「筛选后全量集合」做精确 count(*)，
    // 搜索/深筛选下是一次无上界聚合。现在封顶采样：内层 LIMIT 10001，
    // 筛选后 ≤10000 条时仍为精确值，超过则计到 10000（前端展示「约」口径）——
    // 把 count 的代价上界从 O(筛选集) 压到 O(10001 行扫描)。
    let count_sql = format!(
        "SELECT count(*) FROM (SELECT 1 FROM torrents t LEFT JOIN users u ON u.id = t.owner_id \
         WHERE {approval_gate_count} \
         AND ($1::int[] IS NULL OR t.category_id = ANY($1)) AND {medium_pred} \
         AND {grade_pred} AND {edition_pred} \
         AND ($5::bool IS NULL OR t.official_tag = $5) {alive_pred} {approval_pred} {status_pred} \
         {bookmark_pred} {search_pred} \
         {tag_pred_count} \
         {sec_sql} {extra_count_sql} LIMIT 10001) sub",
    );
    let total: i64 = sqlx::query_scalar(&count_sql)
        .bind(filter.category_id.clone())
        .bind(filter.medium_id)
        .bind(filter.grade_id)
        .bind(filter.edition_id)
        .bind(filter.official)
        .bind(&pattern)
        .bind(filter.include_unapproved)
        .bind(filter.tag_ids.clone())
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
        // $24：标题谓词（与 extra_count_sql 的 title_like 槽位对应）
        .bind(&title_pat)
        // $25：评分下界（0283 P1-5，计数侧）
        .bind(filter.rating_min)
        .fetch_one(db)
        .await
        .unwrap_or_else(|e| {
            // 计数失败此前静默成 0（页面「共 0 个」却有行）；给日志留痕
            tracing::error!(error = %e, "列表计数查询失败，降级 total=0");
            0
        });

    let has_more = rows.len() as i64 > limit;
    let mut items: Vec<TorrentRow> =
        rows.into_iter().take(limit as usize).collect();
    // 0170 反向页：行序反转回原方向输出；游标锚点随方向取首/末行 ——
    // 正向页锚末行（下一页），反向页锚首行（继续往回翻）
    if filter.reverse {
        items.reverse();
    }
    let next_cursor = has_more.then(|| {
        let anchor = if filter.reverse {
            items.first()
        } else {
            items.last()
        };
        anchor
            .map(|r| cursor::encode_next(&cursor_col, r))
            .unwrap_or_default()
    });
    Ok(TorrentPage {
        items,
        next_cursor,
        total_estimate: total,
    })
}
