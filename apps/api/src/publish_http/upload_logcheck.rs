//! 发种侧的抓轨日志闸门与落库（0312）。
//!
//! 三种力度（`logcheck_policy`）：
//! - `off`（默认）：不解析、不落库，行为与本功能上线前完全一致；
//! - `tag`：解析打分并落库、详情页标注，但**不拦发布**；
//! - `require`：命中 `logcheck_gate_site_types` 的站型必须交日志，且分数
//!   达 `logcheck_min_score`，否则 400 拒发。
//!
//! 闸门一律发生在 `INSERT torrents` **之前**（0288 立的规矩：入库后报错会留下
//! 待审残种，而重试同一 .torrent 永远撞「种子重复」，等于把这条内容永久锁死）。

use actix_web::web::Bytes;
use sqlx::PgPool;

use crate::errors::{DomainError, DomainResult};
use crate::logcheck::{self, LogCheck};

/// 已读取但尚未定分的日志 part。
pub(super) struct LogPart {
    pub filename: String,
    pub bytes: Bytes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    Off,
    Tag,
    Require,
}

pub(super) struct Policy {
    pub mode: Mode,
    pub min_score: i16,
    pub log_cap: usize,
    /// 强制站型清单；空 = 全站型强制
    pub gate_types: Vec<String>,
    pub site_type: String,
}

impl Policy {
    /// 本次上传是否要求「必须有达线日志」。
    fn required(&self) -> bool {
        self.mode == Mode::Require
            && (self.gate_types.is_empty()
                || self.gate_types.iter().any(|t| *t == self.site_type))
    }
}

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

/// 读闸门配置（含当前站型——站型是 site_settings 单行，无需缓存）。
pub(super) async fn load(db: &PgPool) -> Policy {
    let raw = setting_str(db, "logcheck_policy", "off").await;
    let mode = match raw.as_str() {
        "tag" => Mode::Tag,
        "require" => Mode::Require,
        _ => Mode::Off,
    };
    let min_score: i16 = setting_str(db, "logcheck_min_score", "100")
        .await
        .parse()
        .unwrap_or(100)
        .clamp(0, 100);
    let kib: i64 = setting_str(db, "logcheck_max_kib", "1024")
        .await
        .parse()
        .unwrap_or(1024);
    let gate_types =
        setting_str(db, "logcheck_gate_site_types", "music,lossless")
            .await
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect();
    let site_type = setting_str(db, "site_type", "general").await;
    Policy {
        mode,
        min_score,
        // 下限 16 KiB：小于一份 EAC 日志的常见体量，配置错了也不至于
        // 让所有日志「凭空消失」
        log_cap: (kib.max(16) * 1024) as usize,
        gate_types,
        site_type,
    }
}

/// 解码 + 定分后的日志（落库与判定共用一份，避免二次解码漂移）。
pub(super) struct DecodedLog {
    pub filename: String,
    pub size: usize,
    pub text: String,
    pub check: LogCheck,
}

/// 解析全部日志 + 按力度判定。
pub(super) fn run(
    policy: &Policy,
    parts: &[LogPart],
) -> DomainResult<Vec<DecodedLog>> {
    if policy.mode == Mode::Off {
        return Ok(Vec::new());
    }
    let logs: Vec<DecodedLog> = parts
        .iter()
        .map(|p| {
            let text = logcheck::to_text(&p.bytes);
            let check = logcheck::check_log(&text);
            DecodedLog {
                filename: p.filename.clone(),
                size: p.bytes.len(),
                text,
                check,
            }
        })
        .collect();
    let overall = overall(&logs);

    if policy.required() {
        if logs.is_empty() {
            return Err(DomainError::Validation(format!(
                "站型「{}」必须提交抓轨日志（EAC / CUETools / WHipper 等）",
                policy.site_type
            )));
        }
        let Some(worst) = overall else {
            return Err(DomainError::Validation(
                "提交的日志无法识别出抓轨工具，请上传抓轨软件生成的原始日志文本\
                 （不是曲目列表，也不是转换后的封面图）"
                    .into(),
            ));
        };
        if worst < policy.min_score {
            let first = &logs
                .iter()
                .find(|l| l.check.score == Some(worst))
                .and_then(|l| l.check.issues.first())
                .map(|i| i.msg.clone())
                .unwrap_or_else(|| "日志校验未通过".to_string());
            return Err(DomainError::Validation(format!(
                "日志分 {worst}% 未达本站底线 {}%：{first}",
                policy.min_score
            )));
        }
    }
    Ok(logs)
}

/// 多碟合成一个日志分：取**最低**（任一碟不到 100，整包就不是 100 的发行）。
/// 详情页徽标读的是落库后的各行，口径与此一致（前端不再算第二套）。
pub(super) fn overall(logs: &[DecodedLog]) -> Option<i16> {
    logcheck::overall(&logs.iter().map(|l| l.check.score).collect::<Vec<_>>())
}

/// 落库（复活路径会先清旧行，见 upload_revive）。
pub(super) async fn store(
    db: &PgPool,
    torrent_id: i64,
    logs: &[DecodedLog],
) -> DomainResult<usize> {
    let mut n = 0usize;
    for (ordinal, l) in logs.iter().enumerate() {
        let rows = sqlx::query(
            "INSERT INTO torrent_logs (torrent_id, ordinal, filename, engine, \
             log_score, tracks, issues, facts, size, body) \
             VALUES ($1, $2, $3, $4, $5, $6, $7::jsonb, $8::jsonb, $9, $10) \
             ON CONFLICT (torrent_id, ordinal) DO NOTHING",
        )
        .bind(torrent_id)
        .bind(ordinal as i32)
        .bind(&l.filename)
        .bind(l.check.engine.code())
        .bind(l.check.score)
        .bind(l.check.tracks)
        .bind(serde_json::to_string(&l.check.issues).unwrap_or("[]".into()))
        .bind(serde_json::to_string(&l.check.facts).unwrap_or("null".into()))
        .bind(l.size as i64)
        .bind(&l.text)
        .execute(db)
        .await
        .map_err(|e| DomainError::Internal(e.into()))?;
        n += rows.rows_affected() as usize;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy(mode: Mode, gate: &[&str]) -> Policy {
        Policy {
            mode,
            min_score: 100,
            log_cap: 1024 * 1024,
            gate_types: gate.iter().map(|s| s.to_string()).collect(),
            site_type: "lossless".into(),
        }
    }

    #[test]
    fn required_only_for_gated_site_type() {
        let p = policy(Mode::Require, &["music", "lossless"]);
        assert!(p.required());
        assert!(!policy(Mode::Tag, &["music"]).required());
        assert!(!policy(Mode::Off, &[]).required());
        // 清单外站型：仍解析（tag 效果），但不作为发种门槛
        let mut out = policy(Mode::Require, &["music"]);
        out.site_type = "general".into();
        assert!(!out.required());
        // 空清单 = 全站型强制
        assert!(policy(Mode::Require, &[]).required());
    }
}
