//! 内置自营适配器种子（二审 R10-3：adapters 表此前零行空货架）。
//! douban wasm 随核心发版（apps/api/adapters/douban），启动时幂等落库：
//! 编译预检失败（wasm 与宿主 ABI 漂移）仅告警不阻塞启动。
//! enabled 恒 FALSE：自营种子只「上架」，启停由站长在后台决定（与 0170
//! 的安装路径同语义——install 后也要手动 toggle）。

use crate::adapter_runtime::AdapterRuntime;
use sqlx::PgPool;

/// douban 适配器 wasm（构建产物已 force-add 入库——include_bytes 需要它在
/// 编译期存在；重建适配器后 `git add -f` 覆盖，checksum 变化触发种子更新）
const DOUBAN_WASM: &[u8] =
    include_bytes!("../adapters/douban/target/wasm32-unknown-unknown/release/adapter_douban.wasm");

fn sha256_hex(b: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b);
    h.finalize().iter().map(|x| format!("{x:02x}")).collect()
}

/// 启动幂等种子：内置自营适配器上架（不启用）。
/// 幂等口径：adapter_id 冲突时 checksum 相同则跳过、不同则更新字节。
pub async fn ensure_builtin_adapters(db: &PgPool) {
    let builtin: &[(&str, &str, &str, &str, &str, &[&str], i32, &[u8])] = &[(
        "builtin.douban",
        "豆瓣元数据（自营）",
        "1.2.0",
        "metadata",
        "movie.douban.com/subject 页面 og: 元数据（移动端路径）",
        // 四审 L3：纯域名前缀会把 https://m.douban.com.evil.tld 也放进
        // starts_with ——种子即高危示范；改为 /** 全域模式
        &["https://m.douban.com/**", "https://movie.douban.com/**"],
        30,
        DOUBAN_WASM,
    )];
    for (adapter_id, name, version, kind, descr, http_allow, rate, wasm) in
        builtin
    {
        // 编译预检：wasmtime 起不来的模块不上架（与 install 端点同标准）
        if let Err(e) =
            wasmtime::Module::new(&AdapterRuntime::global().engine, wasm)
        {
            tracing::warn!(
                adapter_id,
                error = %e,
                "内置适配器编译预检失败，跳过上架"
            );
            continue;
        }
        let checksum = sha256_hex(wasm);
        let r = sqlx::query(
            "INSERT INTO adapters (adapter_id, kind, name, version, \
             core_compat, http_allow, secrets_read, rate_limit_per_min, \
             wasm, checksum, enabled) \
             VALUES ($1, $2, $3, $4, '*', $5, '[]'::jsonb, $6, $7, $8, \
             FALSE) \
             ON CONFLICT (adapter_id) DO UPDATE SET \
             wasm = EXCLUDED.wasm, checksum = EXCLUDED.checksum, \
             version = EXCLUDED.version, name = EXCLUDED.name, \
             http_allow = EXCLUDED.http_allow \
             WHERE adapters.checksum <> EXCLUDED.checksum",
        )
        .bind(adapter_id)
        .bind(kind)
        .bind(name)
        .bind(version)
        .bind(serde_json::json!(http_allow))
        .bind(rate)
        .bind(wasm)
        .bind(&checksum)
        .execute(db)
        .await;
        match r {
            Ok(res) if res.rows_affected() > 0 => tracing::info!(
                adapter_id,
                "内置适配器已上架（默认停用，可在后台启用）"
            ),
            Ok(_) => {}
            Err(e) => tracing::warn!(
                adapter_id,
                error = %e,
                "内置适配器上架失败（不阻塞启动）"
            ),
        }
    }
}
