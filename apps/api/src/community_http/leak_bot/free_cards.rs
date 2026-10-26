//! 喊话盒 `/free` 的种子卡数据（从 leak_bot.rs 按域拆出）。
//!
//! 命中口径必须走 `crate::torrents::promo`（全站促销判定的唯一来源）与
//! `crate::torrents::visibility_sql`（种子可见性唯一口径），不能自己从
//! promotions 直挂 JOIN 种子表数一遍。
//!
//! 实测（批二百四十二，真库事务内挂夹具后 A/B，闸门 JA）：夹具齐时旧写法只播
//! 出 1 张卡——且那张正是会员点不开的种；同一时刻按共用层口径数得出 165 条
//! 会员可见的免费种 ⇒ 全站免费周期间会员打 `/free` 得到「当前没有免费促销的
//! 种子」，而列表每一行都挂着免费角标。
//! 这与后台筛选的 P-121 是同一个洞的第二个出口（后台那处已修，注释写着
//! 「旧写法只数 `p.torrent_id = t.id` 直挂促销，全站/官种/分类促销期间的种
//! 在这里一律算『没促销』」）。

use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

/// /free 回话的种子卡片数据（0420 播报带种卡）：与论坛 !torrent 引用卡
/// 同族，随回话直接下发（零新端点）；promotion 取合并后的有效档位。
#[derive(sqlx::FromRow, serde::Serialize, Default)]
pub(super) struct FreeCard {
    pub(super) id: i64,
    pub(super) name: String,
    pub(super) small_descr: Option<String>,
    pub(super) size: i64,
    pub(super) seeders: i32,
    pub(super) leechers: i32,
    pub(super) poster: Option<String>,
    pub(super) promotion: Option<String>,
    /// P-551：促销「免费」只免上传量扣费，不免种子定价——charge_for_download
    /// 从不看促销。有价种进播报时必须把价格一起给出去，否则会员按播报
    /// 去下载才发现要扣魔力（下载口有 priceHint 提示，播报卡此前没有）。
    pub(super) price: i64,
}

/// /free 纯文本回话；种卡由 cards[] 并行下发，文本保留供纯文本消费方。
pub(super) fn free_reply(cards: &[FreeCard]) -> String {
    if cards.is_empty() {
        "当前没有免费促销的种子。".to_string()
    } else {
        format!(
            "当前免费：{}",
            cards
                .iter()
                .map(|c| format!("#{} {}", c.id, c.name))
                .collect::<Vec<_>>()
                .join("；")
        )
    }
}

/// 取数 SQL：`{free 档存在性} ∧ {对本访客可见}`，档位取多维合并后的有效标签。
///
/// 行序按「合成效果下一次改变的时刻」（`effective_ends`，多条促销里最早到期
/// 那条）再按 id，保证快到期的先播报且顺序稳定。
fn sql_for(uid: i64, is_staff: bool) -> String {
    format!(
        "SELECT t.id, t.name, t.small_descr, t.size, t.seeders, \
         t.leechers, t.price, t.media_info->>'poster' AS poster, \
         {} AS promotion \
         FROM torrents t \
         WHERE {} AND {} \
         ORDER BY {} NULLS LAST, t.id LIMIT 5",
        crate::torrents::promo::effective_kind(),
        crate::torrents::promo::exists_clause(Some("'free','x2free'")),
        crate::torrents::visibility_sql(Some((uid, is_staff))),
        crate::torrents::promo::effective_ends(),
    )
}

/// 当前对本访客可见的免费种（至多 5 张卡）。
pub(super) async fn free_cards(
    db: &PgPool,
    uid: i64,
    is_staff: bool,
) -> DomainResult<Vec<FreeCard>> {
    let rows: Vec<FreeCard> = sqlx::query_as(&sql_for(uid, is_staff))
        .fetch_all(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(id: i64, name: &str) -> FreeCard {
        FreeCard {
            id,
            name: name.to_string(),
            promotion: Some("free".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn free_reply_text() {
        // 无促销一句话；有种列「#id 名称」分号相连
        assert_eq!(free_reply(&[]), "当前没有免费促销的种子。");
        let cards = vec![card(7, "甲"), card(9, "乙")];
        assert_eq!(free_reply(&cards), "当前免费：#7 甲；#9 乙");
    }

    /// cards 序列化含前端紧凑卡所需字段（缺字段 = 静默少渲染一角）
    #[test]
    fn free_cards_serialize() {
        let v = serde_json::to_value(&card(7, "甲")).unwrap();
        let s = v.to_string();
        for k in [
            "id",
            "name",
            "size",
            "seeders",
            "promotion",
            "poster",
            "price",
        ] {
            assert!(s.contains(&format!("\"{k}\":")), "cards 缺字段 {k}");
        }
        assert_eq!(v["promotion"], "free");
    }

    /// 取数必须走共用口径：四种作用域一个都不能少（P-121 同洞第二出口），
    /// 且不得再有「只数种子级直挂」的内联写法。
    #[test]
    fn sql_covers_all_promo_scopes() {
        let s = sql_for(7, false);
        for frag in [
            "p.torrent_id = t.id",
            "p.scope = 'global'",
            "p.scope = 'official'",
            "p.scope = 'non_official'",
            "p.scope = 'category'",
        ] {
            assert!(s.contains(frag), "促销作用域缺片段 {frag}");
        }
        assert!(
            !s.contains("JOIN torrents t ON t.id = p.torrent_id"),
            "退回只数直挂促销的旧写法"
        );
        // 档位取合并口径（含倍率合成），不是原始 p.kind
        assert!(
            s.contains("x.du = 0 AND x.mu = 2 THEN 'x2free'"),
            "档位没走多维合并"
        );
        assert!(
            s.contains("p.kind::text IN ('free','x2free')"),
            "档位筛选丢失"
        );
        // 占位符必须都被替换掉（format! 少一个参数是编译错，少一片口径是静默）
        assert!(!s.contains("{}"), "SQL 里还留着未替换的占位符");
        assert!(s.ends_with("LIMIT 5"), "播报上限丢了");
        // P-551：免费促销不免种子定价，播报必须把定价一起带出去
        assert!(s.contains("t.price"), "cards 没带种子定价");
    }

    /// 可见性走唯一口径：待审/被拒/暂缓种不得进广播（上传期就会自动挂促销）
    #[test]
    fn sql_filters_invisible_for_member() {
        assert!(
            sql_for(7, false).contains("t.approval_status = 1"),
            "会员视角应只看过审种"
        );
        // staff 视角放 0/2/4（口径由 visibility_sql 给，本处只验真的带上）
        assert!(
            sql_for(7, true).contains("t.approval_status IN (0, 2, 4)"),
            "staff 视角没走可见性口径"
        );
    }
}
