//! MediaInfo 一致性比对的落地（0390）。
//!
//! 领域判定在 `domain::mediainfo`（解析 / 比对 / 脱敏都是纯函数），
//! 本文件负责：取声明 → 调领域 → 落结论。
//!
//! 【为什么不拦截发布】判定是**提示**不是闸门。解析器对少见格式会失准，
//! 把合法片源误判成标题党，比放过几个标题党更伤用户信任。结论写进
//! `torrents.mediainfo_check`，由审核台与管理组决定是否处置。

use crate::domain::mediainfo as mi;
use crate::errors::DomainResult;
use crate::state::AppState;

/// 从标题里猜出声明的规格（发布时用户不会单独填「我声明 4K」，
/// 规格信息就在标题里）。**猜不出就留空**——留空不是失败，
/// 上层据此判「无法判定」，而不是拿一个猜的值去比对。
pub fn declared_from_title(name: &str) -> mi::Declared {
    let low = name.to_ascii_lowercase();
    let resolution = ["2160p", "4k", "uhd", "1440p", "1080p", "720p", "480p"]
        .iter()
        .find(|p| low.contains(**p))
        .map(|p| p.to_string());
    let video_codec = ["x265", "h.265", "hevc", "x264", "h.264", "h264", "avc", "xvid"]
        .iter()
        .find(|p| low.contains(**p))
        .map(|p| p.to_string());
    // HDR：只在标题出现 HDR/DV/杜比视界 等词时才判「声明了」。
    // 没出现 → None（未声明），而不是 Some(false)（声明了不是）。
    // 标题里点分隔是常态（`Movie.Dolby.Vision.2160p`），而
    // 「dolby vision」中间是空格——两者对不上，早前版本因此漏判。
    // 做法：先按词逐个判，再补一条「点分词」的判据。
    let punct_norm = low.replace(['.', '_', '-'], " ");
    let hdr = if ["dolby vision", "dovi", "dv", "hdr10", "hdr", "hlg", "dovi"]
        .iter()
        .any(|p| {
            contains_word(&low, p)
                || contains_word(&punct_norm, p)
                || p.contains(' ') && punct_norm.contains(p)
        })
    {
        Some(true)
    } else {
        None
    };
    mi::Declared { resolution, video_codec, hdr }
}

/// 词边界匹配：`dv` 不该命中 `dvd`/`dvi`，`hdr` 不该命中 `hdr_look`。
fn contains_word(hay: &str, needle: &str) -> bool {
    hay.match_indices(needle).any(|(i, _)| {
        let before = hay[..i].chars().next_back();
        let after = hay[i + needle.len()..].chars().next();
        let ok_before = before.map(|c| !c.is_alphanumeric() && c != '_').unwrap_or(true);
        let ok_after = after.map(|c| !c.is_alphanumeric() && c != '_').unwrap_or(true);
        ok_before && ok_after
    })
}

/// 比对一个种子的 MediaInfo，落结论。返回结论（None = 无 MediaInfo 可比）。
pub async fn check_and_store(
    state: &AppState,
    torrent_id: i64,
    name: &str,
    mediainfo_text: Option<&str>,
) -> DomainResult<Option<mi::CompareResult>> {
    let Some(text) = mediainfo_text.filter(|t| !t.trim().is_empty()) else {
        // 没有 MediaInfo 就没有结论。**不写 NULL 之外的东西**——
        // 「未提供」与「已检查且一致」在巡检面上必须能区分。
        return Ok(None);
    };
    let summary = mi::parse(text);
    let declared = declared_from_title(name);
    let result = mi::compare(&declared, &summary);

    let payload = serde_json::json!({
        "checked_at": chrono::Utc::now().to_rfc3339(),
        "summary": summary,
        "mismatches": result.mismatches,
        "unverifiable": result.unverifiable,
        "mediainfo_sanitized": mi::sanitize(text),
    });
    sqlx::query("UPDATE torrents SET mediainfo_check = $2 WHERE id = $1")
        .bind(torrent_id)
        .bind(&payload)
        .execute(&state.repo.db)
        .await
        .map_err(|e| crate::errors::DomainError::Internal(e.into()))?;
    Ok(Some(result))
}

/// 巡检：不一致的种子清单（管理台用）。
pub async fn list_mismatches(
    state: &AppState,
    limit: i64,
) -> DomainResult<Vec<serde_json::Value>> {
    let rows: Vec<(i64, String, i64, String, serde_json::Value, i64)> =
        sqlx::query_as(
            "SELECT id, name, owner_id, approval_status::text, \
                    COALESCE(mediainfo_check->'mismatches', '[]'::jsonb), \
                    COALESCE(jsonb_array_length(mediainfo_check->'mismatches'), 0)::bigint \
             FROM torrents \
             WHERE mediainfo_check IS NOT NULL \
               AND jsonb_array_length(COALESCE(mediainfo_check->'mismatches', '[]'::jsonb)) > 0 \
             ORDER BY created_at DESC LIMIT $1",
        )
        .bind(limit.clamp(1, 200))
        .fetch_all(&state.repo.db)
        .await
        .map_err(|e| crate::errors::DomainError::Internal(e.into()))?;
    Ok(rows
        .into_iter()
        .map(|(id, name, owner, status, ms, n)| {
            serde_json::json!({
                "torrent_id": id, "name": name, "owner_id": owner,
                "approval_status": status, "mismatch_count": n,
                "mismatches": ms,
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_resolution_from_title() {
        let d = declared_from_title("Some.Movie.2160p.WEB-DL");
        assert_eq!(d.resolution.as_deref(), Some("2160p"));
        let d = declared_from_title("Movie.1080p.mkv");
        assert_eq!(d.resolution.as_deref(), Some("1080p"));
        let d = declared_from_title("Movie.4K.mkv");
        assert_eq!(d.resolution.as_deref(), Some("4k"));
    }

    #[test]
    fn reads_codec_from_title() {
        assert_eq!(
            declared_from_title("Movie.2160p.x265").video_codec.as_deref(),
            Some("x265")
        );
        assert_eq!(
            declared_from_title("Show.S01.1080p.H.264").video_codec.as_deref(),
            Some("h.264")
        );
    }

    #[test]
    fn reads_hdr_only_with_word_boundary() {
        assert_eq!(declared_from_title("Movie.2160p.HDR").hdr, Some(true));
        assert_eq!(declared_from_title("Movie.Dolby.Vision").hdr, Some(true));
        // 关键：dv 不该命中 dvd/dvi 这类无关词
        assert_eq!(declared_from_title("Movie.DVD.Rip").hdr, None);
        assert_eq!(declared_from_title("Movie.DVDRip").hdr, None);
    }

    #[test]
    fn absent_spec_is_none_not_false() {
        // 没写 HDR ≠ 声明了「不是 HDR」。用 Some(false) 会让「未声明」
        // 与「声明了 SDR」混为一谈。
        let d = declared_from_title("Movie.1080p.x264");
        assert_eq!(d.hdr, None);
    }

    #[test]
    fn undeclared_spec_is_unverifiable_not_mismatch() {
        // 标题什么都没写 → 无可比字段 → 不得判为标题党
        let s = mi::parse("Stream #0:0: Video: AVC\nWidth : 1920\nHeight : 1080\n");
        let r = mi::compare(&declared_from_title("Some.Release"), &s);
        assert!(!r.is_mismatch());
    }

    #[test]
    fn end_to_end_fake_4k_from_title() {
        // 标题宣称 4K HDR，MediaInfo 显示 1080p SDR → 必须抓到
        let s = mi::parse(
            "Stream #0:0: Video: AVC\nFormat : AVC\n\
             Width : 1920\nHeight : 1080\n\
             Pixel format : yuv420p\nColor space : YUV\n\
             Transfer characteristics : BT.709\n",
        );
        let d = declared_from_title("Big.Movie.2160p.HDR.x265");
        let r = mi::compare(&d, &s);
        assert!(r.is_mismatch());
        let fields: Vec<&str> =
            r.mismatches.iter().map(|m| m.field.as_str()).collect();
        assert!(fields.contains(&"resolution"), "{fields:?}");
        assert!(fields.contains(&"video_codec"), "{fields:?}");
        assert!(fields.contains(&"hdr"), "{fields:?}");
    }

    #[test]
    fn end_to_end_honest_release_is_clean() {
        let s = mi::parse(
            "Stream #0:0: Video: HEVC\nFormat : HEVC\n\
             Width : 3840\nHeight : 2160\n\
             Pixel format : yuv420p10le\n\
             Transfer characteristics : SMPTE ST 2084 (PQ)\n",
        );
        let d = declared_from_title("Big.Movie.2160p.HDR.x265");
        let r = mi::compare(&d, &s);
        assert!(!r.is_mismatch(), "{:?}", r.mismatches);
    }
}
