//! 统一扩展契约 `fx:extension@0.1.0`（设计稿 P1 包装层：零行为变化）。
//!
//! 背景（`_doc/FluxTorrent统一扩展契约设计稿.md`）：仓里有三套互相独立的
//! 扩展机制，各有各的清单形状与安全水位——
//!
//! | kind     | 现实现                    | 清单形状              | 安全边界 |
//! |----------|---------------------------|-----------------------|----------|
//! | `wasm`   | `adapter_runtime`          | `AdapterManifest`     | 最强（无 DB，出网白名单） |
//! | `native` | `plugins` + `plugin_ctx`   | `(plugin, caps)` 对   | 已收口（`PluginCtx` + `sql_guard`） |
//! | `formula`| `rules_engine`             | `RuleSpec` 常量       | 构造安全（白名单公式） |
//!
//! 本模块只做**收敛层**，不动单体的三套执行路径：把三份清单统一投影到
//! 一个 `ExtensionManifest`，供后台「扩展总览」与后续 P3 事件总线、
//! P4 第三方签名装载消费。设计稿的冻结纪律是「不在纸面上冻结」——
//! 契约版本停在 `@0.1.0`，每个字段都必须被至少一个真实扩展用过才升版。
//!
//! **不要在这里加执行逻辑**：能力校验仍然发生在各 runtime 自己的边界
//! （wasm 在 `adapter_host` 的导入函数、native 在 `sql_guard`、formula 在
//! `rules_engine::lint`）。本模块只回答「有哪些扩展、各自要什么」。

use serde::Serialize;

use crate::plugin_ctx::DbCapability;

/// 契约版本。改动清单字段时同步升次版本号（字段级破坏性改动才升主版本）。
pub const EXTENSION_CONTRACT_VERSION: &str = "0.1.0";

/// 扩展后端种类。三套现有机制的统一投影，新增后端在此登记。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionKind {
    /// 外部 `.wasm` 沙箱（adapter_*）
    Wasm,
    /// 进程内 Rust trait object，编译期装配（plugins）
    Native,
    /// 受限表达式（rules_engine）
    Formula,
}

impl ExtensionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wasm => "wasm",
            Self::Native => "native",
            Self::Formula => "formula",
        }
    }
}

/// 扩展声明的能力（最小授权的声明面）。
///
/// 语义：**未声明即拒绝**——宿主只在分发时按此清单授予，越权调用在各
/// runtime 边界报错并记审计。这里只描述「要什么」，判定发生在边界。
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct Capabilities {
    /// `net:egress` —— 出网及主机 glob 白名单（wasm 专用）
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub net_egress: Vec<String>,
    /// `secret:read` —— 可读的站点密钥名（wasm 专用）
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub secret_read: Vec<String>,
    /// `db` —— 数据库能力（native 专用；公式为空）
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub db: Vec<DbCapability>,
}

impl DbCapability {
    /// 转成可序列化的声明形状（`sql_guard` 消费的仍是原类型）。
    pub fn describe(&self) -> serde_json::Value {
        serde_json::json!({
            "table": self.table,
            "columns": self.columns,
            "mode": match self.mode {
                crate::plugin_ctx::DbMode::Read => "read",
                crate::plugin_ctx::DbMode::Write => "write",
            },
            "allow_unbounded": self.allow_unbounded,
        })
    }
}

/// 扩展运行限额。
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Limits {
    /// 每分钟调用上限（0 = 该 runtime 无此维度）
    pub rate_per_min: i32,
    /// 单次调用超时秒数（0 = 未设）
    pub timeout_s: i32,
}

impl Default for Limits {
    fn default() -> Self {
        Self { rate_per_min: 0, timeout_s: 0 }
    }
}

/// 统一扩展清单。三套机制的**投影**，不是它们的替代品。
///
/// 字段与设计稿 §3.1 一一对应；`signature` 槽位现在就定义（P4 才消费），
/// 但当前全部内置扩展均未签名——第三方 native 装载未开放。
#[derive(Debug, Clone, Serialize)]
pub struct ExtensionManifest {
    pub id: String,
    pub kind: ExtensionKind,
    /// 契约版本（该扩展声明遵守的契约版本）
    pub version: String,
    pub display: String,
    pub capabilities: Capabilities,
    /// 订阅的事件名（`DomainEvent` 变体名；按需调用的扩展为空）
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub triggers: Vec<String>,
    pub limits: Limits,
    /// 签名槽（P4 消费；当前恒 None）
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub signature: Option<String>,
}

impl ExtensionManifest {
    /// 契约版本校验：清单版本必须与本模块常量一致（或为兼容的 0.1.x）。
    ///
    /// 冻结纪律（P5 `@1.0.0` 前）：版本不是装饰——不匹配直接拒，避免
    /// 「旧扩展按旧语义跑在新宿主上」这类只有出事才看得见的偏差。
    pub fn validate_version(&self) -> Result<(), String> {
        let parse = |v: &str| -> Option<(u32, u32)> {
            let mut it = v.split('.');
            let major = it.next()?.parse::<u32>().ok()?;
            let minor = it.next()?.parse::<u32>().ok()?;
            Some((major, minor))
        };
        let v = parse(&self.version)
            .ok_or_else(|| format!("清单版本格式非法：{}", self.version))?;
        let c = parse(EXTENSION_CONTRACT_VERSION)
            .expect("契约版本常量本身格式非法");
        if v != c {
            return Err(format!(
                "扩展 {} 声明契约 {}，宿主为 {EXTENSION_CONTRACT_VERSION}",
                self.id, self.version
            ));
        }
        Ok(())
    }
}

/// native 插件的默认清单（`Plugin::manifest` 默认实现体）。
///
/// 抽成自由函数而不是让覆写者写 `<Self as Plugin>::manifest(..)`：
/// 同名方法在 Rust 里永远解析到**自身**而不是 trait 默认实现，一调就是
/// 无限递归（现场踩过：测试线程 STACK_OVERFLOW）。自由函数没有这个歧义。
pub fn native_manifest(id: &str, caps: &[DbCapability]) -> ExtensionManifest {
    ExtensionManifest {
        id: id.to_string(),
        kind: ExtensionKind::Native,
        version: EXTENSION_CONTRACT_VERSION.to_string(),
        display: id.to_string(),
        capabilities: Capabilities { db: caps.to_vec(), ..Default::default() },
        triggers: Vec::new(),
        limits: Default::default(),
        signature: None,
    }
}

/// 全部已装配扩展的统一清单（跨三套 runtime）。
///
/// P1 的交付面就这一条：后台「扩展总览」、P3 事件总线的登记源、P4 的
/// 签名校验入口都读它。**不改变任何执行路径**——三套 runtime 仍按各自
/// 原有方式装配与分发，这里只是把「有哪些扩展」这件事收成一个答案。
///
/// 版本不符的条目在这里被剔除（并由调用方记日志）：契约升版时不该让
/// 旧扩展带着旧语义继续跑。
pub fn registry() -> Vec<ExtensionManifest> {
    let mut out: Vec<ExtensionManifest> = Vec::new();
    for m in crate::plugins::PluginManager::builtin().manifests() {
        out.push(m);
    }
    for m in crate::rules_engine::all_rule_specs() {
        out.push(m.to_extension_manifest());
    }
    out.retain(|m| {
        if let Err(_e) = m.validate_version() {
            tracing::warn!(extension = %m.id, "扩展清单版本与契约不符，已剔除");
            false
        } else {
            true
        }
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugins::Plugin;

    fn base() -> ExtensionManifest {
        ExtensionManifest {
            id: "demo".into(),
            kind: ExtensionKind::Native,
            version: EXTENSION_CONTRACT_VERSION.into(),
            display: "演示".into(),
            capabilities: Capabilities::default(),
            triggers: vec![],
            limits: Limits::default(),
            signature: None,
        }
    }

    #[test]
    fn current_version_passes() {
        assert!(base().validate_version().is_ok());
    }

    #[test]
    fn minor_drift_is_rejected() {
        let mut m = base();
        m.version = "0.2.0".into();
        assert!(m.validate_version().is_err());
    }

    #[test]
    fn major_drift_is_rejected() {
        let mut m = base();
        m.version = "1.0.0".into();
        assert!(m.validate_version().is_err());
    }

    #[test]
    fn malformed_version_is_rejected() {
        let mut m = base();
        m.version = "abc".into();
        assert!(m.validate_version().is_err());
    }

    #[test]
    fn registry_has_native_and_formula() {
        let r = registry();
        assert!(!r.is_empty(), "扩展总览不许是空的（判据打空）");
        let kinds: std::collections::HashSet<_> =
            r.iter().map(|m| m.kind.as_str()).collect();
        assert!(kinds.contains("native"), "native 侧缺席：{:?}", kinds);
        assert!(kinds.contains("formula"), "formula 侧缺席：{:?}", kinds);
    }

    #[test]
    fn registry_ids_are_unique() {
        let r = registry();
        let mut ids: Vec<&str> = r.iter().map(|m| m.id.as_str()).collect();
        let n = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), n, "扩展 id 必须全局唯一");
    }

    #[test]
    fn registry_entries_all_pass_version_check() {
        for m in registry() {
            assert!(m.validate_version().is_ok(), "{} 版本不符", m.id);
        }
    }

    #[test]
    fn native_manifest_carries_db_capability() {
        let caps = crate::plugins_builtin::auto_pin_official_caps();
        let m = crate::plugins_builtin::AutoPinOfficial.manifest(&caps);
        assert_eq!(m.kind, ExtensionKind::Native);
        assert_eq!(m.capabilities.db.len(), 1);
        assert_eq!(m.capabilities.db[0].table, "torrents");
        assert!(m.triggers.contains(&"TorrentUploaded".to_string()));
        assert_eq!(m.signature, None, "P4 前内置扩展不应有签名槽值");
    }

    #[test]
    fn formula_manifest_has_no_capabilities() {
        let m = crate::rules_engine::TERM_RATE.to_extension_manifest();
        assert_eq!(m.kind, ExtensionKind::Formula);
        assert!(m.capabilities.db.is_empty());
        assert!(m.capabilities.net_egress.is_empty());
        assert!(m.capabilities.secret_read.is_empty());
    }

    #[test]
    fn kinds_serialize_as_lowercase() {
        let j = serde_json::to_string(&ExtensionKind::Wasm).unwrap();
        assert_eq!(j, "\"wasm\"");
    }
}