//! 种子管理（M02/M03）：编辑/删除/恢复/抓取列表/NFO/求续种/标签。
//! 从 torrents.rs 按域拆出。

use serde::Serialize;
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

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

pub async fn list_snatches(
    db: &PgPool,
    torrent_id: i64,
) -> DomainResult<Vec<SnatchRow>> {
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
/// 0288：准入从「写死 approval_status=1」改为 `visibility_sql` 同口径。
/// 旧写法让**审核员也读不到待审种子的 NFO**（实测 404），
/// 而 NFO 恰恰是判断压制来源/版本/是否盗版的核心依据——审核只能凭标题猜。
pub async fn get_nfo(
    db: &PgPool,
    torrent_id: i64,
    viewer: (i64, bool),
) -> DomainResult<Option<String>> {
    let vis = super::visibility::visibility_sql(Some(viewer));
    let row: Option<Option<String>> = sqlx::query_scalar(&format!(
        "SELECT nfo FROM torrents t WHERE t.id = $1 AND {vis}"
    ))
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
    let row: Option<(i32, Option<chrono::DateTime<chrono::Utc>>, String)> =
        sqlx::query_as(
            "SELECT seeders, last_reseed, \
         name FROM torrents WHERE id = $1 AND approval_status = 1",
        )
        .bind(torrent_id)
        .fetch_optional(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((seeders, last_reseed, name)) = row else {
        return Err(DomainError::NotFound(torrent_id));
    };
    if seeders > 0 {
        return Err(DomainError::Validation(
            "该种子仍有人做种，无需补种".into(),
        ));
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
            "INSERT INTO messages (sender_id, receiver_id, \
             subject, body) VALUES ($1, $2, $3, $4)",
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

/// 种子标签（T-04）：列出字典 + 该种子已打的标签（0138：字典只出种子域 scope=torrent）
/// 0288 / P1-1：可见性闸门长在取数里（原来各端点自己判，漏一处就是一条口子）
pub async fn list_tags(
    db: &PgPool,
    torrent_id: i64,
    viewer: (i64, bool),
) -> DomainResult<serde_json::Value> {
    super::visibility::assert_visible(db, torrent_id, viewer).await?;
    let dict: Vec<(
        i32,
        String,
        String,
        Option<String>,
        Option<String>,
        String,
    )> = sqlx::query_as(
        "SELECT id, name, \
         kind, nullif(bg_color, ''), nullif(color, ''), tag_group \
         FROM tag_dict WHERE scope = 'torrent' ORDER BY sort DESC, id",
    )
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let mine: Vec<i32> = sqlx::query_scalar(
        "SELECT tag_id FROM tags WHERE torrent_id = $1 ORDER BY tag_id",
    )
    .bind(torrent_id)
    .fetch_all(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // dict 行带样式列（0159 P1 渲染统一）：旧客户端按元组取前三项仍兼容，
    // 新客户端读 bg_color/color（nullif 空串→null，未配色回落默认）；
    // 0160 P2 再带 tag_group（详情页标签云按组分区）
    let dict_json: Vec<serde_json::Value> = dict
        .into_iter()
        .map(|(id, name, kind, bg, color, group)| {
            serde_json::json!({ "id": id, "name": name, "kind": kind,
                                "bg_color": bg, "color": color,
                                "group": group })
        })
        .collect();
    Ok(serde_json::json!({ "dict": dict_json, "mine": mine }))
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
        "SELECT owner_id, kind FROM torrents t JOIN tag_dict d ON d.id = $2 \
         WHERE t.id = $1 AND d.scope = 'torrent'",
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
        sqlx::query(
            "INSERT INTO tags (torrent_id, tag_id) VALUES ($1, \
         $2) ON CONFLICT DO NOTHING",
        )
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
    sync_official_tag(db, torrent_id).await?;
    Ok(())
}

/// 标签合法性单一判据（0288 抽出）：存在 + scope=torrent + enabled + 官种仅 staff。
/// `apply_torrent_tags` 与发种前置校验共用同一份，杜绝「先入库再报标签错」。
pub async fn check_tag_ids(
    db: &PgPool,
    tag_ids: &[i32],
    is_staff: bool,
) -> DomainResult<()> {
    if tag_ids.len() > 12 {
        return Err(DomainError::Validation("标签最多选择 12 个".into()));
    }
    for tid in tag_ids {
        let row: Option<(String, bool)> = sqlx::query_as(
            "SELECT kind, COALESCE(enabled, TRUE) FROM tag_dict \
             WHERE id = $1 AND scope = 'torrent'",
        )
        .bind(tid)
        .fetch_optional(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        let Some((kind, enabled)) = row else {
            return Err(DomainError::Validation(format!("标签 {tid} 不存在")));
        };
        if !enabled {
            return Err(DomainError::Validation(format!("标签 {tid} 已停用")));
        }
        if kind == "official" && !is_staff {
            return Err(DomainError::Forbidden); // 官种/官方标签仅 staff
        }
    }
    Ok(())
}

/// 统一打标入口（0159 标签 P0）：发布 / 详情页 / 管理批量共用。
/// 校验口径与 tag_torrent 一致（存在 + enabled + scope=torrent + official
/// 仅 staff），官方标签联动 official_tag 物化列——三个入口不再各写各的。
pub async fn apply_torrent_tags(
    db: &PgPool,
    torrent_id: i64,
    tag_ids: &[i32],
    actor: (i64, i16),
) -> DomainResult<()> {
    check_tag_ids(db, tag_ids, actor.1 >= 90).await?;
    for tid in tag_ids {
        sqlx::query(
            "INSERT INTO tags (torrent_id, tag_id) VALUES ($1, $2) \
             ON CONFLICT DO NOTHING",
        )
        .bind(torrent_id)
        .bind(tid)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
    }
    sync_official_tag(db, torrent_id).await?;
    Ok(())
}

/// official_tag 物化列同步（0159）：列=「挂有 official-kind 标签」的纯派生值，
/// 任何写 tags 的路径收口后调这里，杜绝列与关联表漂移（0044 双口径事故根因）。
/// official-kind 标签缺失时列回落 false——回落即「免费」促销口径外的促销
/// 判定不变，仅官种语义跟随标签。
async fn sync_official_tag(db: &PgPool, torrent_id: i64) -> DomainResult<()> {
    sqlx::query(
        "UPDATE torrents SET official_tag = EXISTS ( \
           SELECT 1 FROM tags tg JOIN tag_dict d ON d.id = tg.tag_id \
           WHERE tg.torrent_id = $1 AND d.kind = 'official') \
         WHERE id = $1",
    )
    .bind(torrent_id)
    .execute(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(())
}
