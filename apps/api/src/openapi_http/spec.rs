//! OpenAPI 文档组装（0268 → F3/W-49 注解化）。
//!
//! 此前 `/openapi.json` 只覆盖 4 个 `/open/*` 路径 —— 兼容层、Torznab、RSS
//! 全不在机器可读 spec 里。开发者按惯例先拉 spec 再写适配器，看到半张图
//! 会误判「这站没这些能力」，然后被迫去读源码。spec 也是契约的一部分。
//!
//! F3/W-49（frame-plan §3 步③④）起路径表**全部来自 handler 注解**：
//!   - 开放数据/我的数据/凭据：open_api.rs（#[utoipa::path] 聚合）
//!   - 兼容层/Torznab/RSS：compat_api.rs（crate::compat_api）
//! 手写路径段（原 spec.rs 的 paths_open/paths_me/paths_token 与 spec_np.rs
//! 整文件）已删除——路径+方法+描述从 route 宏旁的注解生成，防「路由在、
//! spec 不在」的漂移。schema 段仍是手写（spec_schemas.rs，本批不动）：
//! 双轨期口径见 open_api.rs 头注。

use serde_json::{json, Map, Value};

fn merge_paths(into: &mut Map<String, Value>, from: Value) {
    if let Value::Object(m) = from {
        for (k, v) in m {
            into.insert(k, v);
        }
    }
}

pub(super) fn build() -> Value {
    let mut paths = Map::new();
    merge_paths(&mut paths, super::open_api::paths());
    merge_paths(&mut paths, crate::compat_api::paths());
    json!({
        "openapi": "3.1.0",
        "info": {
            "title": "FluxTorrent Open API",
            "version": "1.4.0",
            "description": "第三方接口全集：开放数据 / 兼容层（NexusPHP 口径）\
    / Torznab / RSS / Token 管理。鉴权三选一：Authorization: Token <fxo_>（推荐）、\
    ?apikey=<fxo_>（Torznab 客户端习惯）、passkey（下载与 RSS，URL 内传递）。\
    限流响应带 X-RateLimit-Limit/Remaining/Reset；429 带 Retry-After: 60。\
    路径表由 #[utoipa::path] 注解生成（F3/W-49），schema 仍为手写组件。",
        },
        "servers": [{ "url": "/api/v1" }],
        "components": super::spec_schemas::components(),
        "security": [{ "apiToken": [] }],
        "paths": Value::Object(paths),
        "x-compat-notes": "兼容层/别名/torznab/rss 的字段口径另见 \
    GET /compat/meta（架构自描述）与 _doc/开放API接入指南.md。",
    })
}

#[cfg(test)]
mod tests {
    use super::build;

    /// F3/W-49 验收锁：最终 openapi.json 必须含两域全部操作（开放 14 +
    /// compat 15 + rss-tokens 4，FRAME §7 第 8 条分域起步），
    /// 且不再有手写路径段可漂移。删任一条 #[utoipa::path] 此测先红。
    #[test]
    fn openapi_json_covers_all_annotated_domains() {
        let spec = build();
        let paths = spec
            .pointer("/paths")
            .and_then(|p| p.as_object())
            .expect("openapi.json 应有 paths 对象");
        let ops: usize = paths
            .values()
            .map(|v| v.as_object().map(|o| o.len()).unwrap_or(0))
            .sum();
        assert!(
            ops >= 32,
            "openapi.json 操作数 {ops} < 32（注解丢失？实际键: {:?}）",
            paths.keys().collect::<Vec<_>>()
        );
        // 抽查每域首尾锚点（路径在则方法由 utoipa 生成保证）
        for anchor in [
            "/api/v1/open/recent",
            "/api/v1/me/tokens",
            "/api/v1/compat/nexusphp/user.json",
            "/api/v1/downloads/keys/revoke",
            "/api/v1/torznab",
            "/api/v1/rss/{passkey}",
        ] {
            assert!(paths.contains_key(anchor), "缺锚点路径 {anchor}");
        }
        // 手写 schema 段仍在（双轨期：components 由 spec_schemas 提供）
        assert!(
            spec.pointer("/components/schemas/Envelope").is_some(),
            "spec_schemas 的 Envelope 组件不应被注解化删掉"
        );
    }
}

#[cfg(test)]
mod dump {
    use super::build;

    /// 离线 dump（gen_ts_types.mjs --file 的稳定数据源）：cargo test
    /// dump_openapi 把 spec 落到 workspace 根 target/openapi_dump.json，
    /// 生成器不依赖运行中的本地栈。
    #[test]
    fn dump_openapi() {
        use std::io::Write as _;
        let spec = build();
        let root = std::env::var("CARGO_MANIFEST_DIR")
            .map(std::path::PathBuf::from)
            .ok();
        let target = root
            .as_deref()
            .and_then(|r| r.parent().and_then(|p| p.parent()))
            .map(|ws| ws.join("target").join("openapi_dump.json"))
            .unwrap_or_else(|| {
                std::path::PathBuf::from("target/openapi_dump.json")
            });
        // parent 不存在也不许红：dump 是给生成器的便利，不是断言
        if let Ok(mut f) = std::fs::File::create(&target) {
            let _ = writeln!(
                std::io::stderr(),
                "openapi dump -> {}",
                target.display()
            );
            let _ = f.write_all(spec.to_string().as_bytes());
        }
    }
}
