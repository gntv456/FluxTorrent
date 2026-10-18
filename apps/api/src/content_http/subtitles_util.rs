//! 字幕上传链路的共用辅助（0146）：站点设定读取、白名单/阈值、release 归一化、
//! 上传请求体与元数据校验。按域拆出 subtitles.rs（300 行门禁）。

use serde::Deserialize;

use crate::errors::{DomainError, DomainResult};

#[derive(Deserialize)]
pub(super) struct SubtitleUploadReq {
    pub(super) torrent_id: i64,
    pub(super) title: String,
    #[serde(default)]
    pub(super) lang: Option<String>,
    /// 真实文件的附件 sha256（前端先调 POST /attachments 上传拿到）。
    /// 审计修复（P1 空壳链路）：旧版 file_ref 是客户端任意字符串或后端伪造的
    /// s3:// UUID——下载只回 JSON 引用，全链路无文件本体。现统一走 attachments
    /// 存储（本地 savedirectory 卷 + sha256 内容寻址 + 配额），file_ref 记
    /// attach://<sha>；历史外部引用（http(s):// 链接）仍按原样展示。
    /// 注意：sha256 是**服务端 SHA3-256** 摘要（attachment_http.rs 计算），
    /// 不是客户端本地算的 SHA2——取值必须来自上传响应，不可自算（0285 P2）。
    #[serde(default)]
    pub(super) file_sha: Option<String>,
    /// 兼容字段：外部字幕站直链（http/https），与本地附件二选一
    #[serde(default)]
    pub(super) file_ref: Option<String>,
    /// 0146 元数据（均可选；P1-1）
    #[serde(default)]
    pub(super) fps: Option<f64>,
    #[serde(default)]
    pub(super) machine_translated: Option<bool>,
    #[serde(default)]
    pub(super) hearing_impaired: Option<bool>,
    #[serde(default)]
    pub(super) foreign_parts_only: Option<bool>,
    #[serde(default)]
    pub(super) source: Option<String>,
    #[serde(default)]
    pub(super) producer: Option<String>,
    #[serde(default)]
    pub(super) proofreader: Option<String>,
    #[serde(default)]
    pub(super) author_name: Option<String>,
    #[serde(default)]
    pub(super) release_name: Option<String>,
    #[serde(default)]
    pub(super) anon: Option<bool>,
}

/// 上传元数据校验结果（SubtitleUploadReq 的 defaulted 视图）
pub(super) struct SubtitleUploadMeta {
    pub(super) fps: Option<f64>,
    pub(super) machine_translated: bool,
    pub(super) hearing_impaired: bool,
    pub(super) foreign_parts_only: bool,
    pub(super) source: Option<String>,
    pub(super) producer: Option<String>,
    pub(super) proofreader: Option<String>,
    pub(super) author_name: Option<String>,
    pub(super) release_name: Option<String>,
    pub(super) anon: bool,
}

impl SubtitleUploadMeta {
    pub(super) fn validate(body: &SubtitleUploadReq) -> DomainResult<Self> {
        if body.title.trim().chars().count() > 200 {
            return Err(DomainError::Validation("标题过长（≤200 字）".into()));
        }
        if let Some(fps) = body.fps {
            if !(0.0..=240.0).contains(&fps) {
                return Err(DomainError::Validation(
                    "FPS 需在 0-240 之间".into(),
                ));
            }
        }
        Ok(Self {
            fps: body.fps,
            machine_translated: body.machine_translated.unwrap_or(false),
            hearing_impaired: body.hearing_impaired.unwrap_or(false),
            foreign_parts_only: body.foreign_parts_only.unwrap_or(false),
            source: trim_opt(&body.source),
            producer: trim_opt(&body.producer),
            proofreader: trim_opt(&body.proofreader),
            author_name: trim_opt(&body.author_name),
            release_name: trim_opt(&body.release_name),
            anon: body.anon.unwrap_or(false),
        })
    }
}

pub(super) fn trim_opt(v: &Option<String>) -> Option<String> {
    v.as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// 站点设定读取（字幕域共用；键缺失回默认值）
pub(super) async fn subtitle_setting(
    db: &sqlx::PgPool,
    name: &str,
    default: &str,
) -> DomainResult<String> {
    let v: Option<String> =
        sqlx::query_scalar("SELECT value FROM site_settings WHERE name = $1")
            .bind(name)
            .fetch_optional(db)
            .await
            .map_err(|e| DomainError::Internal(e.into()))?;
    Ok(v.filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string()))
}

/// 交付相关性判据（批四十五 P-177）：两条交付路（旧直交付 `fulfill` 与工作流
/// 交稿 `deliver`）共用的唯一一把尺 —— 交的东西必须与这一单要的是**同一件事**。
///
/// 原先两边都只查「字幕是你本人上传且已过审」，语言与种子一个都不比。而默认
/// 站型 `subtitle_workflow=0` 时 `fulfill` 是唯一活着的结算口、连认领都不必
/// ⇒ 任何人拿任意一条自己的字幕就能把别人凑出来的悬赏池整池领走。
pub(super) async fn ensure_req_match(
    db: &sqlx::PgPool,
    rid: i64,
    subtitle_id: i64,
) -> DomainResult<()> {
    // (请求语言, 请求种子, 字幕语言, 字幕字典语言 id, 字幕种子)
    type Row = (
        String,
        Option<i64>,
        Option<String>,
        Option<i16>,
        Option<i64>,
    );
    let row: Option<Row> = sqlx::query_as(
        "SELECT r.lang, r.torrent_id, s.lang, s.lang_id, s.torrent_id \
         FROM subtitle_requests r JOIN subtitles s ON s.id = $2 \
         WHERE r.id = $1",
    )
    .bind(rid)
    .bind(subtitle_id)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    // 请求或字幕其中一条不在：由调用方原有的 NotFound/Validation 口径去报
    let Some((rl, rtid, sl, slab, stid)) = row else {
        return Ok(());
    };
    if let Some(want_tid) = rtid {
        if stid != Some(want_tid) {
            return Err(DomainError::Validation(format!(
                "交稿字幕挂在种子 {}，与本单指定的种子 {} 不是同一个",
                stid.map(|v| v.to_string())
                    .unwrap_or_else(|| "未关联种子".into()),
                want_tid
            )));
        }
    }
    let want = rl.trim().to_lowercase();
    let got = sl.unwrap_or_default().trim().to_lowercase();
    if want.is_empty() || got == want {
        return Ok(());
    }
    // 两侧都按字典 code 写（chs/eng/jpn…），但历史行可能只有 lang_id
    let dict: Option<i16> = sqlx::query_scalar(
        "SELECT id FROM subtitle_langs WHERE code = lower($1)",
    )
    .bind(&rl)
    .fetch_optional(db)
    .await
    .map_err(|e| DomainError::Internal(e.into()))?;
    if dict.is_none() || slab != dict {
        return Err(DomainError::Validation(format!(
            "交稿字幕语言为 {}，与本单要求的 {} 不一致",
            if got.is_empty() { "未标注" } else { &got },
            rl
        )));
    }
    Ok(())
}

/// 扩展名白名单：subtitle_ext_whitelist（kind 决定默认值，站长可改）
pub(super) async fn subtitle_ext_whitelist(
    db: &sqlx::PgPool,
) -> DomainResult<Vec<String>> {
    let raw = subtitle_setting(
        db,
        "subtitle_ext_whitelist",
        "srt,ass,ssa,sup,idx,sub,cue,zip,rar,7z",
    )
    .await?;
    Ok(raw
        .split(',')
        .map(|s| s.trim().to_ascii_lowercase())
        .filter(|s| !s.is_empty())
        .collect())
}

/// 坏字幕自动隐藏阈值（默认 3；≤0 = 不自动隐藏）
pub(super) async fn subtitle_bad_threshold(
    db: &sqlx::PgPool,
) -> DomainResult<i32> {
    Ok(subtitle_setting(db, "subtitle_bad_threshold", "3")
        .await?
        .parse()
        .unwrap_or(3))
}

/// release name 归一化（匹配用）：去非字母数字 + 小写
pub(super) fn normalize_release(name: &str) -> String {
    name.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
}
