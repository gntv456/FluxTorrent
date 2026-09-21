//! 附件对象存储（0102）：S3 兼容协议的轻量实现 + 本地卷双后端。
//!
//! 设计取舍：不引 object_store/AWS SDK（拖入几十个传递依赖与 tokio 版本耦合），
//! 用 reqwest 手写 S3 PUT/GET 的最小面——附件场景只需内容寻址读写：
//!   PUT {bucket}/{sha[0..2]}/{sha[2..4]}/{sha}（与本地卷同目录结构，迁移时 rsync 即可）
//!   GET 同路径；AWS SigV4 签名（MinIO/OSS/COS/R2/B2 全兼容）。
//!
//! 后端选择（site_settings.storage_backend，运行时可切）：
//!   local（缺省）—— savedirectory 本地卷，零依赖开箱即用；
//!   s3           —— S3_ENDPOINT/S3_BUCKET/S3_KEY/S3_SECRET 环境变量 + 可选 S3_REGION。
//! 读取端兼容历史数据：优先当前后端，未命中回落另一后端（local→s3 迁移期无缝）。

use sha2::Digest;

#[derive(Clone, PartialEq, Debug)]
pub enum Backend {
    Local,
    S3,
}

/// 当前启用后端（每请求查库——与 site_settings 热生效口径一致；查询失败回落 local）
pub async fn current_backend(db: &sqlx::PgPool) -> Backend {
    let v: Option<String> = sqlx::query_scalar(
        "SELECT value FROM site_settings WHERE name = 'storage_backend'",
    )
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    match v.as_deref() {
        Some("s3") => Backend::S3,
        _ => Backend::Local,
    }
}

fn s3_cfg() -> Option<(String, String, String, String, String)> {
    let ep = std::env::var("S3_ENDPOINT")
        .ok()
        .filter(|v| !v.trim().is_empty())?;
    let bucket = std::env::var("S3_BUCKET")
        .ok()
        .filter(|v| !v.trim().is_empty())?;
    let key = std::env::var("S3_KEY")
        .ok()
        .filter(|v| !v.trim().is_empty())?;
    let secret = std::env::var("S3_SECRET")
        .ok()
        .filter(|v| !v.trim().is_empty())?;
    let region =
        std::env::var("S3_REGION").unwrap_or_else(|_| "us-east-1".into());
    Some((ep.trim().to_string(), bucket, key, secret, region))
}

fn object_key(sha: &str) -> String {
    format!("{}/{}/{}", &sha[..2], &sha[2..4], sha)
}

/// 本地卷路径（savedirectory 缺省 ./attachments）
pub async fn local_path(db: &sqlx::PgPool, sha: &str) -> String {
    let dir: String = sqlx::query_scalar(
        "SELECT COALESCE((SELECT value FROM site_settings WHERE name = \
         'savedirectory'), './attachments')",
    )
    .fetch_one(db)
    .await
    .unwrap_or_else(|_| "./attachments".into());
    format!("{}/{}", dir.trim_end_matches('/'), object_key(sha))
}

// ---- AWS SigV4（PUT/GET 最小面） ----

type HmacSha256 = hmac::Hmac<sha2::Sha256>;

fn sign_hmac(key: &[u8], msg: &[u8]) -> Vec<u8> {
    use hmac::Mac;
    let mut mac =
        <HmacSha256 as hmac::Mac>::new_from_slice(key).expect("hmac key");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

fn sha256_hex(data: &[u8]) -> String {
    let d = sha2::Sha256::digest(data);
    d.iter().map(|b| format!("{b:02x}")).collect()
}

fn uri_encode(s: &str, encode_slash: bool) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z'
            | b'a'..=b'z'
            | b'0'..=b'9'
            | b'-'
            | b'.'
            | b'_'
            | b'~' => out.push(b as char),
            b'/' if !encode_slash => out.push('/'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// S3 写入。返回 Ok(true)=已写；Ok(false)=未配置 S3（调用方回落 local）；Err=写入失败。
pub async fn s3_put(
    sha: &str,
    bytes: &[u8],
    mime: &str,
) -> anyhow::Result<bool> {
    let Some((ep, bucket, key, secret, region)) = s3_cfg() else {
        return Ok(false);
    };
    let host = ep
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    let path = format!("/{}/{}", bucket, uri_encode(&object_key(sha), false));
    let payload_hash = sha256_hex(bytes);
    let amz_date = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let date_short = chrono::Utc::now().format("%Y%m%d").to_string();

    // canonical request
    let canonical = format!(
        "PUT\n{path}\n\ncontent-type:{mime}\nhost:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n\ncontent-type;host;x-amz-content-sha256;x-amz-date\n{payload_hash}"
    );
    let scope = format!("{date_short}/{region}/s3/aws4_request");
    let sts = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical.as_bytes())
    );

    let k_date =
        sign_hmac(format!("AWS4{secret}").as_bytes(), date_short.as_bytes());
    let k_region = sign_hmac(&k_date, region.as_bytes());
    let k_service = sign_hmac(&k_region, b"s3");
    let k_signing = sign_hmac(&k_service, b"aws4_request");
    let signature = hex_of(&sign_hmac(&k_signing, sts.as_bytes()));

    let url = format!("{ep}{path}");
    let client = reqwest::Client::new();
    let resp = client
        .put(&url)
        .header("content-type", mime)
        .header("x-amz-date", &amz_date)
        .header("x-amz-content-sha256", &payload_hash)
        .header(
            "Authorization",
            format!("AWS4-HMAC-SHA256 Credential={key}/{scope}, SignedHeaders=content-type;host;x-amz-content-sha256;x-amz-date, Signature={signature}"),
        )
        .body(bytes.to_vec())
        .send()
        .await?;
    if !resp.status().is_success() && resp.status().as_u16() != 409 {
        anyhow::bail!("S3 PUT {} → {}", sha, resp.status());
    }
    Ok(true)
}

/// S3 读取。None=对象不存在或未配置。
pub async fn s3_get(sha: &str) -> Option<Vec<u8>> {
    let (ep, bucket, key, secret, region) = s3_cfg()?;
    let host = ep
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    let path = format!("/{}/{}", bucket, uri_encode(&object_key(sha), false));
    let payload_hash = sha256_hex(b"");
    let amz_date = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let date_short = chrono::Utc::now().format("%Y%m%d").to_string();

    let canonical = format!(
        "GET\n{path}\n\nhost:{host}\nx-amz-content-sha256:{payload_hash}\nx-amz-date:{amz_date}\n\nhost;x-amz-content-sha256;x-amz-date\n{payload_hash}"
    );
    let scope = format!("{date_short}/{region}/s3/aws4_request");
    let sts = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        sha256_hex(canonical.as_bytes())
    );
    let k_date =
        sign_hmac(format!("AWS4{secret}").as_bytes(), date_short.as_bytes());
    let k_region = sign_hmac(&k_date, region.as_bytes());
    let k_service = sign_hmac(&k_region, b"s3");
    let k_signing = sign_hmac(&k_service, b"aws4_request");
    let signature = hex_of(&sign_hmac(&k_signing, sts.as_bytes()));

    let url = format!("{ep}{path}");
    let client = reqwest::Client::new();
    let resp = client
        .get(&url)
        .header("x-amz-date", &amz_date)
        .header("x-amz-content-sha256", &payload_hash)
        .header(
            "Authorization",
            format!("AWS4-HMAC-SHA256 Credential={key}/{scope}, SignedHeaders=host;x-amz-content-sha256;x-amz-date, Signature={signature}"),
        )
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    resp.bytes().await.ok().map(|b| b.to_vec())
}

fn hex_of(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

/// 统一写入入口：按后端分发（s3 未配置自动回落 local）
pub async fn put(
    db: &sqlx::PgPool,
    sha: &str,
    bytes: &[u8],
    mime: &str,
) -> anyhow::Result<()> {
    match current_backend(db).await {
        Backend::S3 if s3_put(sha, bytes, mime).await? => Ok(()),
        _ => {
            let p = local_path(db, sha).await;
            if let Some(dir) = std::path::Path::new(&p).parent() {
                tokio::fs::create_dir_all(dir).await?;
            }
            tokio::fs::write(&p, bytes).await?;
            Ok(())
        }
    }
}

/// 统一读取入口：当前后端优先，未命中回落另一后端（迁移期无缝）
pub async fn get(db: &sqlx::PgPool, sha: &str) -> Option<Vec<u8>> {
    match current_backend(db).await {
        Backend::S3 => match s3_get(sha).await {
            Some(b) => Some(b),
            None => tokio::fs::read(local_path(db, sha).await).await.ok(),
        },
        Backend::Local => {
            match tokio::fs::read(local_path(db, sha).await).await {
                Ok(b) => Some(b),
                Err(_) => s3_get(sha).await,
            }
        }
    }
}
