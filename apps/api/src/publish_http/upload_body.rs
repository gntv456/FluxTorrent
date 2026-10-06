//! 发种请求体读取（0288 从 upload.rs 拆出，300 行门禁）。
//!
//! 三样东西：`.torrent` 字节、可选 NFO 字节、以及**文本 part 形式的元数据**
//! （`build_form` 会把它们与 query string 合并，见 upload_fields.rs）。

use actix_web::web::{Bytes, BytesMut};
use futures_util::StreamExt;
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};

use super::upload_precheck as precheck;

/// 单个文本字段的大小上限（防一个 part 塞满内存）
const FIELD_MAX: usize = 256 * 1024;
const NFO_MAX: usize = 1024 * 1024;

pub(super) async fn read_body(
    db: &PgPool,
    payload: &mut actix_multipart::Multipart,
) -> DomainResult<(Bytes, Option<Bytes>, Vec<(String, String)>)> {
    // 体积上限可配（默认 4 MiB）：原来写死 4 MiB，10 TB 级合集
    // （16 MiB 分片 ≈ 13 MB）根本发不出来，而文案还把上限说成硬规定。
    let cap = precheck::torrent_cap_bytes(db).await;
    let mut file_bytes: Option<Bytes> = None;
    let mut nfo_bytes: Option<Bytes> = None;
    let mut mp_fields: Vec<(String, String)> = Vec::new();
    while let Some(item) = payload.next().await {
        let mut field =
            item.map_err(|e| DomainError::Validation(e.to_string()))?;
        // 名称先取成拥有值：`field.name()` 的借用会横跨下面 `field.next()`
        // 的可变借用（文本分支要回用这个名字拼错误信息与字段表）。
        let fname = field.name().map(str::to_string);
        match fname.as_deref() {
            Some("file") => {
                let mut buf = BytesMut::new();
                while let Some(chunk) = field.next().await {
                    buf.extend_from_slice(
                        &chunk.map_err(|e| {
                            DomainError::Validation(e.to_string())
                        })?,
                    );
                    if buf.len() > cap {
                        return Err(DomainError::Validation(format!(
                            ".torrent 超过上限 {} MiB（站点设定可调）",
                            cap / (1024 * 1024)
                        )));
                    }
                }
                file_bytes = Some(buf.freeze());
            }
            // NFO 文件（NP upload.php nfo 口径）：文本解码后落 torrents.nfo
            Some("nfo") => {
                let mut buf = BytesMut::new();
                while let Some(chunk) = field.next().await {
                    buf.extend_from_slice(
                        &chunk.map_err(|e| {
                            DomainError::Validation(e.to_string())
                        })?,
                    );
                    if buf.len() > NFO_MAX {
                        return Err(DomainError::Validation(
                            "NFO 超过 1MiB 上限".into(),
                        ));
                    }
                }
                nfo_bytes = Some(buf.freeze());
            }
            // 文本 part = 发种元数据（0288 新增通道：长描述不再受
            // HTTP 请求行上限摆布）。
            Some(other) => {
                let mut buf = BytesMut::new();
                while let Some(chunk) = field.next().await {
                    buf.extend_from_slice(
                        &chunk.map_err(|e| {
                            DomainError::Validation(e.to_string())
                        })?,
                    );
                    if buf.len() > FIELD_MAX {
                        return Err(DomainError::Validation(
                            "发种字段超过 256 KiB".into(),
                        ));
                    }
                }
                let text = String::from_utf8_lossy(&buf).into_owned();
                mp_fields.push((other.to_string(), text));
            }
            // 无 name 的 part：读完丢弃（不消费会把后续 field 留在半截流上）
            None => {
                while let Some(chunk) = field.next().await {
                    chunk
                        .map_err(|e| DomainError::Validation(e.to_string()))?;
                }
            }
        }
    }
    let bytes = file_bytes
        .ok_or(DomainError::Validation("缺少 .torrent 文件".into()))?;
    Ok((bytes, nfo_bytes, mp_fields))
}
