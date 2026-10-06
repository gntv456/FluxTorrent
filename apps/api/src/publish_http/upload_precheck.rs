//! 发种前置校验（0288）。
//!
//! 根因背景（实测审计 P0-4，报告 `_doc/发种审种动线实测审计-2026-10-05.md`）：
//! 标签、推荐位、价格三类校验原本发生在 `INSERT torrents` **之后**，报错时种子已入库
//! —— 实测「带官方标签」403、「标签不存在」400、「推荐位越权」403 三种情况都会留下
//! 一枚待审残种，而用户重试同一 .torrent 永远撞「种子重复」，等于永久锁死这条内容。
//! `sections` 在更早的批次里已前置（upload.rs 里那段注释），本模块把剩下的补齐。
//!
//! 站方可配的最低内容标准默认**全部关闭**（新装与存量站行为不变），站长按站开启。

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use crate::bencode::ParsedTorrent;
use crate::errors::{DomainError, DomainResult};
use crate::http::AuthUser;

use super::ptgen::UploadForm;

/// 文本上限。超限必须出声——过去 `descr` 的实际上限由 HTTP 层决定（走 query string
/// 时 8192 汉字即 400 空响应、16384 字 431），发种员看不到任何有效信息。
pub(super) const NAME_MAX: usize = 500;
pub(super) const SMALL_DESCR_MAX: usize = 255;
pub(super) const DESCR_MAX: usize = 60_000;
/// 付费种子价格上限（原为静默 clamp）
pub(super) const PRICE_MAX: i64 = 1_000_000;

async fn setting_str(db: &PgPool, name: &str, default: &str) -> String {
    sqlx::query_scalar(
        r#"SELECT COALESCE((SELECT value FROM site_settings
        WHERE name = $1), $2)"#,
    )
    .bind(name)
    .bind(default)
    .fetch_one(db)
    .await
    .unwrap_or_else(|_| default.to_string())
}

async fn setting_int(db: &PgPool, name: &str, default: i64) -> i64 {
    sqlx::query_scalar(
        r#"SELECT COALESCE((SELECT value::bigint FROM site_settings
        WHERE name = $1), $2)"#,
    )
    .bind(name)
    .bind(default)
    .fetch_one(db)
    .await
    .unwrap_or(default)
}

/// `.torrent` 体积上限（字节）。默认 4 MiB，允许站长配到 1–64 MiB。
/// 大合集（10 TB @ 16 MiB 分片 ≈ 13 MB）在原 4 MiB 硬编码下根本发不出来。
pub(super) async fn torrent_cap_bytes(db: &PgPool) -> usize {
    let v = setting_int(db, "upload_torrent_max_bytes", 4 * 1024 * 1024).await;
    (v.max(1024 * 1024).min(64 * 1024 * 1024)) as usize
}

/// 文本长度闸门（含精确报错：哪一项、当前多少、上限多少）。
pub(super) fn check_lengths(form: &UploadForm) -> DomainResult<()> {
    let cases = [
        ("种子名称", form.name.as_deref(), NAME_MAX),
        ("简短说明", form.small_descr.as_deref(), SMALL_DESCR_MAX),
        ("简介正文", form.descr.as_deref(), DESCR_MAX),
    ];
    for (label, val, max) in cases {
        let n = val.map(|s| s.chars().count()).unwrap_or(0);
        if n > max {
            return Err(DomainError::Validation(format!(
                "{label}过长：{n} 字符，上限 {max} 字符"
            )));
        }
    }
    Ok(())
}

/// 定价：越界直接 400。过去 `clamp(0, 1_000_000)` 会让「发 2000000」静默变成
/// 「1000000」，发布者以为设置成功（与本项目「静默改写即缺陷」的既有口径一致）。
pub(super) fn check_price(raw: Option<i64>) -> DomainResult<i64> {
    let p = raw.unwrap_or(0);
    if !(0..=PRICE_MAX).contains(&p) {
        return Err(DomainError::Validation(
            "付费下载价格需在 0–1000000 之间".into(),
        ));
    }
    Ok(p)
}

/// 标签校验：与 `torrents::check_tag_ids` 同一判据，但发生在建种子行之前。
/// 只判不写（写由 store_sections_tags 负责）——重要的是越权/不存在的标签
/// 必须在这里就拦下，否则种子已入库、同一 .torrent 永久判重。
pub(super) async fn check_tags(
    db: &PgPool,
    form: &UploadForm,
    auth: &AuthUser,
) -> DomainResult<()> {
    let Some(json) = form
        .tags
        .as_deref()
        .map(str::trim)
        .filter(|j| !j.is_empty())
    else {
        return Ok(());
    };
    let ids: Vec<i32> = serde_json::from_str(json)
        .map_err(|_| DomainError::Validation("tags 需为 JSON 数组".into()))?;
    crate::torrents::check_tag_ids(db, &ids, auth.class_id >= 90).await
}

/// 置顶/推荐（0089 挑选 口径）的校验结果。
pub(super) struct PromoSet {
    pub pos: i16,
    pub pick: i16,
    pub until: Option<DateTime<Utc>>,
}

/// 推荐位：权限 + 取值 + 时间格式全部前置校验。
pub(super) fn check_promo(
    form: &UploadForm,
    auth: &AuthUser,
) -> DomainResult<Option<PromoSet>> {
    let until_raw = form
        .pos_state_until
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let touched = form.pos_state.unwrap_or(0) != 0
        || form.pick_type.unwrap_or(0) != 0
        || until_raw.is_some();
    if !touched {
        return Ok(None);
    }
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
    let until = until_raw
        .map(|s| {
            DateTime::parse_from_rfc3339(s)
                .map(|dt| dt.with_timezone(&Utc))
                .map_err(|_| {
                    DomainError::Validation("置顶截止时间格式无效".into())
                })
        })
        .transpose()?;
    Ok(Some(PromoSet { pos, pick, until }))
}

/// 站方可配的最低内容标准。默认全关：`0` / `no` / 空正则即不拦。
pub(super) async fn check_quality(
    db: &PgPool,
    form: &UploadForm,
    parsed: &ParsedTorrent,
    screenshots: usize,
) -> DomainResult<()> {
    let min_descr =
        setting_int(db, "upload_min_descr_len", 0).await.max(0) as usize;
    let descr_len = form
        .descr
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .chars()
        .count();
    if min_descr > 0 && descr_len < min_descr {
        return Err(DomainError::Validation(format!(
            "简介正文至少 {min_descr} 字符（当前 {descr_len}）"
        )));
    }
    let min_shots = setting_int(db, "upload_require_screenshots", 0)
        .await
        .max(0) as usize;
    if min_shots > 0 && screenshots < min_shots {
        return Err(DomainError::Validation(format!(
            "简介至少需要 {min_shots} 张截图（当前 {screenshots}）"
        )));
    }
    if setting_str(db, "upload_require_mediainfo", "no").await == "yes"
        && form
            .mediainfo
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .is_none()
    {
        return Err(DomainError::Validation("本站要求提交 MediaInfo".into()));
    }
    let pattern = setting_str(db, "upload_title_pattern", "").await;
    if !pattern.trim().is_empty() {
        let re = regex::Regex::new(&pattern).map_err(|e| {
            DomainError::Validation(format!(
                "站点命名正则非法，请联系站长：{e}"
            ))
        })?;
        if !re.is_match(&parsed.name) {
            return Err(DomainError::Validation(format!(
                "种子名称不符合本站命名规范（要求：{pattern}）"
            )));
        }
    }
    Ok(())
}

/// 重复发布判定结果。
pub(super) enum DupVerdict {
    /// 放行（默认口径：只回 same_source 提示，不拦）
    Allow,
    /// 自动并入既有聚合组
    AutoGroup(i64),
}

/// `upload_dup_policy`：`suggest`（默认，等于现状）/ `block` / `group`。
/// 判据用 `pieces_hash`（同内容不同分片大小的重打包也能命中），不用标题。
pub(super) async fn dup_policy(
    db: &PgPool,
    pieces_hash: &str,
) -> DomainResult<DupVerdict> {
    if pieces_hash.is_empty() {
        return Ok(DupVerdict::Allow);
    }
    let policy = setting_str(db, "upload_dup_policy", "suggest").await;
    if policy == "suggest" {
        return Ok(DupVerdict::Allow);
    }
    let hit: Option<(i64, String, Option<i64>)> = sqlx::query_as(
        "SELECT id, name, group_id FROM torrents WHERE pieces_hash = $1 \
         AND pieces_hash <> '' AND approval_status <> 3 ORDER BY id LIMIT 1",
    )
    .bind(pieces_hash)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    let Some((id, name, gid)) = hit else {
        return Ok(DupVerdict::Allow);
    };
    if policy == "group" {
        // 命中已有组就锁进去；没组的旧种就地建组，避免「策略要求并组但无处可并」
        let gid = match gid {
            Some(g) => g,
            None => {
                let id64: i64 = sqlx::query_scalar(
                    r#"INSERT INTO torrent_groups (name, created_at)
                    VALUES ($1, now()) RETURNING id"#,
                )
                .bind(&name)
                .fetch_one(db)
                .await
                .map_err(|e| DomainError::Internal(e.into()))?;
                sqlx::query("UPDATE torrents SET group_id = $2 WHERE id = $1")
                    .bind(id)
                    .bind(id64)
                    .execute(db)
                    .await
                    .map_err(|e| DomainError::Internal(e.into()))?;
                id64
            }
        };
        return Ok(DupVerdict::AutoGroup(gid));
    }
    Err(DomainError::Validation(format!(
        "同内容种子已存在（#{id} {name}）：本站重复发布策略为「拦下」，请改做种或并组，不要重发"
    )))
}

/// 免审连击门槛：连击数只由**审核员过审**累加（`review_decide`），
/// 发种侧的免审通道不再给自己加分（P1-8 自激根因：免审也 +1 ⇒ 一旦过 5 就事实永久免审）。
pub(super) const STREAK_SKIP_MIN: i32 = 5;
