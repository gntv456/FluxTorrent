//! 发种请求体读取（0288 从 upload.rs 拆出，300 行门禁）。
//!
//! 四样东西：`.torrent` 字节、可选 NFO 字节、可选**抓轨日志** part（0312）、
//! 以及文本 part 形式的元数据（`build_form` 会把它们与 query string 合并，
//! 见 upload_fields.rs）。
//!
//! 日志必须是**独立的二进制通道**：走文本通道会被 `from_utf8_lossy` 洗掉
//! GBK 码页的轨道名，还会被 256 KiB 的文本字段上限截断——截断的日志
//! 尾部「No errors occurred / End of status report」必然丢失，判分就成了误判。

use actix_web::web::{Bytes, BytesMut};
use futures_util::StreamExt;
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

use super::upload_artifacts as artifacts;
use super::upload_logcheck as logcheck;
use super::upload_precheck as precheck;

/// 单个文本字段的大小上限（防一个 part 塞满内存）
const FIELD_MAX: usize = 256 * 1024;
const NFO_MAX: usize = 1024 * 1024;

/// 已读取的发种请求体。
pub(super) struct UploadBody {
    pub torrent: Bytes,
    pub nfo: Option<Bytes>,
    pub fields: Vec<(String, String)>,
    /// 抓轨日志 part（多碟就有多个，顺序即 ordinal）
    pub logs: Vec<logcheck::LogPart>,
    /// 本次生效的日志闸门（含站型）——读一次，判定与落库共用
    pub logcheck: logcheck::Policy,
    /// 种子工件 part（0323：校验和清单/更新日志，顺序即提交序）
    pub artifacts: Vec<artifacts::ArtifactPart>,
}

pub(super) async fn read_body(
    db: &PgPool,
    payload: &mut actix_multipart::Multipart,
) -> DomainResult<UploadBody> {
    // 体积上限可配（默认 4 MiB）：原来写死 4 MiB，10 TB 级合集
    // （16 MiB 分片 ≈ 13 MB）根本发不出来，而文案还把上限说成硬规定。
    let cap = precheck::torrent_cap_bytes(db).await;
    let policy = logcheck::load(db).await;
    // 闸门关闭时不收集日志，但**仍按 name 认领**日志 part：
    // 落到文本通道会冒出一个 `log` 元数据字段，语义上是「未知表单字段」
    let collect_logs = policy.mode != logcheck::Mode::Off;
    let mut file_bytes: Option<Bytes> = None;
    let mut nfo_bytes: Option<Bytes> = None;
    let mut mp_fields: Vec<(String, String)> = Vec::new();
    let mut logs: Vec<logcheck::LogPart> = Vec::new();
    let mut arts: Vec<artifacts::ArtifactPart> = Vec::new();
    while let Some(item) = payload.next().await {
        let mut field =
            item.map_err(|e| DomainError::Validation(e.to_string()))?;
        // 名称先取成拥有值：`field.name()` 的借用会横跨下面 `field.next()`
        // 的可变借用（文本分支要回用这个名字拼错误信息与字段表）。
        let fname = field.name().map(str::to_string);
        let ffile = field
            .content_disposition()
            .as_ref()
            .and_then(|cd| cd.get_filename())
            .map(str::to_string);
        match fname.as_deref() {
            Some("file") => {
                file_bytes = Some(drain(&mut field, cap, ".torrent").await?);
            }
            // NFO 文件（NP upload.php nfo 口径）：文本解码后落 torrents.nfo
            Some("nfo") => {
                nfo_bytes = Some(drain(&mut field, NFO_MAX, "NFO").await?);
            }
            // 文本 part = 发种元数据（0288 新增通道：长描述不再受
            // HTTP 请求行上限摆布）。例外：确属抓轨日志的一律走日志通道。
            Some(other) => {
                let is_log =
                    matches!(other, "log" | "logs") || is_log_filename(&ffile);
                // 种子工件（0323）：checksums/changelog/license 文本 part。
                // name=artifact 或文件名命中工件后缀都认领；超量拒收。
                let is_artifact = matches!(other, "artifact" | "artifacts")
                    || matches!(
                        ffile.as_deref().map(|f| {
                            f.to_ascii_lowercase()
                        }),
                        Some(f)
                        if f.ends_with(".sfv")
                            || f.ends_with(".md5")
                            || f.ends_with(".sha1")
                            || f.ends_with(".sha256")
                            || f.contains("changelog")
                    );
                if is_log && collect_logs {
                    let bytes =
                        drain(&mut field, policy.log_cap, "日志").await?;
                    let n = logs.len() + 1;
                    logs.push(logcheck::LogPart {
                        filename: ffile
                            .filter(|f| !f.trim().is_empty())
                            .unwrap_or_else(|| format!("log-{n}")),
                        bytes,
                    });
                } else if is_log {
                    // 闸门关闭：读完丢弃（不消费会把后续 field 留在半截流上）
                    drain(&mut field, usize::MAX, "日志").await?;
                } else if is_artifact {
                    if arts.len() >= artifacts::ARTIFACT_MAX_COUNT {
                        return Err(DomainError::Validation(format!(
                            "工件至多 {} 件（校验和/更新日志等）",
                            artifacts::ARTIFACT_MAX_COUNT
                        )));
                    }
                    let buf =
                        drain(&mut field, artifacts::ARTIFACT_MAX, "工件")
                            .await?;
                    let text = String::from_utf8_lossy(&buf).into_owned();
                    let fname =
                        ffile.filter(|f| !f.trim().is_empty()).unwrap_or_else(
                            || format!("artifact-{}", arts.len() + 1),
                        );
                    arts.push(artifacts::ArtifactPart {
                        kind: artifacts::infer_kind(&fname).to_string(),
                        filename: fname,
                        body: text,
                    });
                } else {
                    let buf = drain(&mut field, FIELD_MAX, "发种字段").await?;
                    let text = String::from_utf8_lossy(&buf).into_owned();
                    mp_fields.push((other.to_string(), text));
                }
            }
            // 无 name 的 part：读完丢弃（不消费会把后续 field 留在半截流上）
            None => {
                drain(&mut field, usize::MAX, "无名 part").await?;
            }
        }
    }
    let torrent = file_bytes
        .ok_or(DomainError::Validation("缺少 .torrent 文件".into()))?;
    Ok(UploadBody {
        torrent,
        nfo: nfo_bytes,
        fields: mp_fields,
        logs,
        logcheck: policy,
        artifacts: arts,
    })
}

/// 读满一个 part（超过 `cap` 立即出声，不静默截断）。
async fn drain(
    field: &mut actix_multipart::Field,
    cap: usize,
    what: &str,
) -> DomainResult<Bytes> {
    let mut buf = BytesMut::new();
    while let Some(chunk) = field.next().await {
        buf.extend_from_slice(
            &chunk.map_err(|e| DomainError::Validation(e.to_string()))?,
        );
        if buf.len() > cap {
            return Err(DomainError::Validation(format!(
                "{what} 超过上限 {} KiB（站点设定可调）",
                cap / 1024
            )));
        }
    }
    Ok(buf.freeze())
}

/// part 文件名是否像抓轨日志（浏览器表单里字段名由前端决定，
/// 但 `<input type=file>` 一定带原始文件名，扩展名是最稳的判据）。
fn is_log_filename(ffile: &Option<String>) -> bool {
    ffile
        .as_deref()
        .map(|f| f.to_ascii_lowercase().ends_with(".log"))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_filename_detected() {
        assert!(is_log_filename(&Some("EAC LOG - Album.log".into())));
        assert!(is_log_filename(&Some("DISC1.LOG".into())));
        assert!(!is_log_filename(&Some("cover.jpg".into())));
        assert!(!is_log_filename(&None));
    }
}
