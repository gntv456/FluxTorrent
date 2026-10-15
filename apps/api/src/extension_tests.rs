//! 统一扩展契约的单元测试（拆出以守 300 行门禁）。

use super::*;
use crate::plugins::Plugin;

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
    fn llm_routes_default_to_no_capability() {
        // 「没声明就没有」——与 net_egress / secret_read / db 同一纪律。
        // 能力最小化的第一课：默认关闭，不是默认开放。
        let c = Capabilities::default();
        assert!(c.llm_routes.is_empty());
        assert!(c.net_egress.is_empty());
        assert!(c.db.is_empty());
    }

    #[test]
    fn wasm_manifest_maps_llm_routes() {
        // 契约层要能看到 wasm 侧的 LLM 能力声明（registry/后台总览读它）
        let m = crate::adapter_runtime::AdapterManifest {
            adapter_id: "llm-demo".into(),
            kind: "metadata".into(),
            http_allow: vec![],
            secrets_read: vec![],
            rate_limit_per_min: 10,
            llm_routes: vec!["anthropic/claude-sonnet".into()],
        };
        let em = m.to_extension_manifest();
        assert_eq!(
            em.capabilities.llm_routes,
            vec!["anthropic/claude-sonnet".to_string()]
        );
    }

    #[test]
    fn kinds_serialize_as_lowercase() {
        let j = serde_json::to_string(&ExtensionKind::Wasm).unwrap();
        assert_eq!(j, "\"wasm\"");
    }
}
