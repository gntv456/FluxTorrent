//! 域名库文件导入（multipart，P-638 续账）：大清单的正解，不受 JSON 墙约束。
//! `POST /admin/email-domains/import-file`，字段 `file`（.txt / .csv）。
//!
//! 上限得自己数：读的过程中同时盯字节数与条数，任一越界就停（JSON 口那套
//! 由墙反推的 IMPORT_MAX = 8176 条装不下整张公共清单）。事务粒度 = 批：一批
//! `BATCH` 条一个事务（`repo::import_domains` 自带
//! begin/commit）。中途失败时前面几批**已经提交**，所以错误文案必须带已导入
//! 条数，别让站长在「以为没导」和「导了一半」之间猜（同 increment_bulk 的
//! 先例）；审计也按真实落库数记，部分提交也要先落账。
//!
//! 解析是流式的：只留「未遇分隔符的尾巴」，不缓存整个文件；分隔符与 JSON
//! 口的 text 字段同一套（换行/CR/逗号/分号/空格/Tab）。

use std::collections::HashSet;

use actix_multipart::Multipart;
use actix_web::{post, web, HttpRequest, HttpResponse};
use futures_util::StreamExt;
use serde_json::{json, Value};

use crate::dto::ok;
use crate::errors::{DomainError, DomainResult};
use crate::repo::email_domains as domain_repo;
use crate::state::AppState;

use super::email_domains::normalize;
use super::guard::staff;

/// 单文件字节上限：自己数，不依赖 Content-Length（分块上传没有它）。
/// 32MiB 是病态输入的天花板；正常清单先撞到下面的条数上限。
const FILE_MAX_BYTES: usize = 32 * 1024 * 1024;
/// 单文件条数上限：逐条 INSERT，条数直接决定请求时长（一批一个事务）。
/// 比这更大的清单请拆分——这个数是**读取过程中数出来的**，不是声明一个
/// 送不到的数（P-638 的教训）。
const MAX_DOMAINS: usize = 100_000;
/// 一批 = 一个事务：中途炸掉只丢这一批。
const BATCH: usize = 2_000;
/// 单条 token 上限：`normalize` 最长收 253 字符，留余量；超过就不可能是
/// 域名，直接记无效并丢到下一个分隔符，不把内存拖爆。
const TOKEN_MAX: usize = 1024;
/// 坏行明细只回样本（整份文件的坏行可能有几万条）。
const REJECT_SAMPLES: usize = 50;

fn is_sep(b: u8) -> bool {
    matches!(b, b'\n' | b'\r' | b',' | b';' | b' ' | b'\t')
}

/// 流式累加器。语义提醒：`total` 是**输入条数**（含无效行），`unique` 是
/// 去重后的合法域名数（含库里已存在的），`added` 才是真落库的行数；
/// `skipped = unique - added`。
#[derive(Default)]
struct Acc {
    /// 未遇到分隔符的尾巴
    tail: Vec<u8>,
    /// 尾巴已超长：丢到下一个分隔符为止
    dropping: bool,
    /// 已凑出的 token，等 drain 去归一化落批
    tokens: Vec<String>,
    total: usize,
    bytes: usize,
    unique: i64,
    batch: Vec<String>,
    seen: HashSet<String>,
    added: i64,
    duplicates: i64,
    invalid: i64,
    rejected: Vec<Value>,
    batches: usize,
}

impl Acc {
    /// 喂一段字节：凑够一条就入 `tokens`（落库由调用方的 drain 做）。
    fn feed(&mut self, chunk: &[u8]) {
        for &b in chunk {
            if is_sep(b) {
                self.close_token();
            } else if self.dropping {
                // 已经判定超长，丢到下一个分隔符
            } else if self.tail.len() >= TOKEN_MAX {
                self.tail.clear();
                self.dropping = true;
                self.reject(format!("单条超过 {TOKEN_MAX} 字节"));
            } else {
                self.tail.push(b);
            }
        }
    }

    /// 收尾当前 token（字段边界也调它：末尾没换行的最后一条不能丢）。
    fn close_token(&mut self) {
        if self.dropping {
            self.dropping = false;
            return;
        }
        if self.tail.is_empty() {
            return;
        }
        // 分隔符都是 ASCII，而 UTF-8 多字节字符不含这些字节 ⇒ 整条解码
        // 不会截断字符
        self.tokens
            .push(String::from_utf8_lossy(&self.tail).into_owned());
        self.tail.clear();
    }

    fn reject(&mut self, reason: String) {
        self.total += 1;
        self.invalid += 1;
        if self.rejected.len() < REJECT_SAMPLES {
            self.rejected
                .push(json!({"index": self.total - 1, "reason": reason}));
        }
    }
}

fn abort(acc: &Acc, what: &str, limit: usize) -> DomainError {
    DomainError::Validation(format!(
        "文件超过{what}上限 {limit}：已导入前 {} 条（分批提交不回滚，\
         0 表示本次未落库），请拆分后继续",
        acc.added
    ))
}

fn parse_err(e: impl std::fmt::Display) -> DomainError {
    DomainError::Validation(format!("文件解析失败：{e}"))
}

/// 归一化一条并攒批；一批满了就独立提交。
async fn ingest(
    state: &AppState,
    acc: &mut Acc,
    raw: &str,
) -> DomainResult<()> {
    if acc.total >= MAX_DOMAINS {
        return Err(abort(acc, "条数", MAX_DOMAINS));
    }
    acc.total += 1;
    match normalize(raw) {
        Ok(d) => {
            if !acc.seen.insert(d.clone()) {
                // 同一份文件里重复出现：不是错，但也不是新增
                acc.duplicates += 1;
                return Ok(());
            }
            acc.unique += 1;
            acc.batch.push(d);
            if acc.batch.len() >= BATCH {
                flush(state, acc).await?;
            }
            Ok(())
        }
        Err(w) => {
            acc.invalid += 1;
            if acc.rejected.len() < REJECT_SAMPLES {
                acc.rejected
                    .push(json!({"index": acc.total - 1, "reason": w}));
            }
            Ok(())
        }
    }
}

/// 把当前批落库（`import_domains` 内部 begin/commit ⇒ 一批一个事务）。
async fn flush(state: &AppState, acc: &mut Acc) -> DomainResult<()> {
    if acc.batch.is_empty() {
        return Ok(());
    }
    let added = domain_repo::import_domains(&state.repo, &acc.batch)
        .await
        .map_err(|e| {
            DomainError::Internal(anyhow::anyhow!(
                "域名库文件导入第 {} 批失败（此前已提交 {} 条）：{e}",
                acc.batches + 1,
                acc.added
            ))
        })?;
    acc.added += added;
    acc.batches += 1;
    acc.batch.clear();
    Ok(())
}

/// 处理已凑出的 token（每读完一段就清空，内存只随单段增长）。
async fn drain(state: &AppState, acc: &mut Acc) -> DomainResult<()> {
    let tokens = std::mem::take(&mut acc.tokens);
    for t in tokens {
        ingest(state, acc, &t).await?;
    }
    Ok(())
}

fn safe_name(field: &actix_multipart::Field) -> String {
    let name = field
        .content_disposition()
        .and_then(|d| d.get_filename().map(str::to_string))
        .unwrap_or_default();
    name.chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '-' | '_'))
        .take(120)
        .collect()
}

/// 流式收口：不缓存整份文件，只留未闭合的尾巴。
async fn read_file(
    state: &AppState,
    payload: &mut Multipart,
    acc: &mut Acc,
    filename: &mut String,
    got_file: &mut bool,
) -> DomainResult<()> {
    while let Some(item) = payload.next().await {
        let mut field = item.map_err(parse_err)?;
        if field.name() != Some("file") {
            // 其余字段（备注之类）只排空，不计入
            while let Some(chunk) = field.next().await {
                chunk.map_err(parse_err)?;
            }
            continue;
        }
        *got_file = true;
        if filename.is_empty() {
            *filename = safe_name(&field);
        }
        while let Some(chunk) = field.next().await {
            let chunk = chunk.map_err(parse_err)?;
            acc.bytes += chunk.len();
            if acc.bytes > FILE_MAX_BYTES {
                return Err(abort(acc, "文件字节数", FILE_MAX_BYTES));
            }
            acc.feed(&chunk);
            drain(state, acc).await?;
        }
        // 字段边界也是分隔符：末尾没换行的最后一条不能丢
        acc.close_token();
        drain(state, acc).await?;
    }
    Ok(())
}

/// POST /admin/email-domains/import-file：multipart 字段 `file`，流式导入。
#[post("/admin/email-domains/import-file")]
async fn email_domains_import_file(
    req: HttpRequest,
    state: web::Data<std::sync::Arc<AppState>>,
    mut payload: Multipart,
) -> DomainResult<HttpResponse> {
    let auth = staff(&req, &state).await?;
    crate::authz::require_perm(
        &state,
        &auth,
        crate::authz::perm::SETTINGS_MANAGE,
    )
    .await?;
    let mut acc = Acc::default();
    let mut filename = String::new();
    let mut got_file = false;
    let outcome = read_file(
        &state,
        &mut payload,
        &mut acc,
        &mut filename,
        &mut got_file,
    )
    .await;
    // 最后不足一批的余量也要落库；出错时保留已提交的账
    let outcome = match outcome {
        Ok(()) => flush(&state, &mut acc).await,
        Err(e) => Err(e),
    };
    if got_file {
        state
            .repo
            .audit(
                Some(auth.id),
                "emaildomain.import_file",
                Some(acc.added),
            )
            .await;
    }
    outcome?;
    if !got_file {
        return Err(DomainError::Validation(
            "缺少 file 字段（multipart 字段名 file）".into(),
        ));
    }
    if acc.total == 0 {
        return Err(DomainError::Validation("没有可导入的域名".into()));
    }
    Ok(ok(json!({
        "file": filename,
        "added": acc.added,
        "skipped": acc.unique - acc.added,
        "duplicates": acc.duplicates,
        "invalid": acc.invalid,
        "total": acc.total,
        "batches": acc.batches,
        "rejected": acc.rejected,
    })))
}
