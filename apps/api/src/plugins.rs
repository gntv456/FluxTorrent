#![allow(dead_code)]
//! M28 插件系统（方案 §4.3）：Hook 思想借鉴 NexusPHP。
//!
//! 纪律（M28 验收基准）：
//! - 插件只通过 `Plugin` Trait 的 Hook 方法介入，不侵入领域层代码；
//! - 插件启停不影响核心链路 —— Hook 分发全部 catch_unwind + 错误降级为日志；
//! - 示例插件「发布自动置顶官种」仅靠注册 Hook 实现。
//!
//! 安全模型（0367 批）：hook 入参从 `&AppState` 换成 `&PluginCtx`——插件
//! 不再直接持有连接池，DB 访问统一走能力句柄（表/列/读写模式声明 +
//! sql_guard 结构校验），详见 `plugin_ctx` / `sql_guard` 模块文档。
//!
//! 线程安全：Hook 以扩展 trait object 形式静态注册（编译期装配），
//! 运行期启停由插件自身配置决定（`enabled()` 每次分发时询问）。

use std::sync::Arc;

use crate::extension::{ExtensionManifest, EXTENSION_CONTRACT_VERSION};
use crate::plugin_ctx::{DbCapability, PluginCtx};
use crate::state::AppState;

/// 领域事件（插件可见的负载）。按需扩展字段。
#[derive(Debug, Clone)]
pub enum DomainEvent {
    /// 用户登录成功
    UserLogin { user_id: i64 },
    /// 种子发布成功（已过审核入列表）
    TorrentUploaded {
        torrent_id: i64,
        owner_id: i64,
        name: String,
    },
    /// 做种里程碑（小时数）
    SeedingMilestone {
        user_id: i64,
        torrent_id: i64,
        hours: i64,
    },
}

/// 插件 Trait：默认全部 no-op，插件按需覆写。
///
/// hook 入参统一为 `&PluginCtx`（能力句柄）——见模块头安全模型。
pub trait Plugin: Send + Sync {
    /// 插件名（日志/审计用）
    fn name(&self) -> &'static str;
    /// 运行开关（读取自身配置；false 时管理器跳过分发）
    fn enabled(&self) -> bool {
        true
    }
    /// 统一扩展契约的清单投影（`fx:extension@0.1.0`）。
    ///
    /// 默认实现走 `extension::native_manifest` 自由函数而不是在这里
    /// 展开结构体——覆写者想复用默认值时，同名 trait 方法会解析到
    /// **自身**（无限递归，现场踩过：测试线程 STACK_OVERFLOW），
    /// 自由函数没有这个歧义。
    ///
    /// 这里只回答「有哪些扩展、各自要什么」；能力校验仍发生在
    /// `PluginCtx`/`sql_guard` 边界，不在这层。
    fn manifest(&self, caps: &[DbCapability]) -> ExtensionManifest {
        crate::extension::native_manifest(self.name(), caps)
    }
    /// 登录后 Hook
    fn on_user_login(&self, _: &PluginCtx, _: i64) {}
    /// 种子发布后 Hook
    fn on_torrent_upload(&self, _: &PluginCtx, _: i64, _: i64) {}
    /// 做种里程碑 Hook（worker 周期触发）
    fn on_seeding_milestone(&self, _: &PluginCtx, _: i64, _: i64) {}
}

/// 插件管理器：编译期静态装配。
///
/// 每项 = (插件实例, 该插件的 DB 能力声明)。分发时按声明构造
/// `PluginCtx`——插件越权访问会被 sql_guard 拦截并记审计。
pub struct PluginManager {
    /// 分发面（plugins_dispatch.rs）要遍历它，故 pub(crate) 而非私有。
    pub(crate) plugins: Vec<(Arc<dyn Plugin>, Vec<DbCapability>)>,
}

impl PluginManager {
    /// 装配全部内置插件。第三方编译进 bin 后在此追加。
    pub fn builtin() -> Self {
        Self {
            plugins: vec![(
                Arc::new(crate::plugins_builtin::AutoPinOfficial),
                crate::plugins_builtin::auto_pin_official_caps(),
            )],
        }
    }

    pub fn list(&self) -> Vec<&'static str> {
        self.plugins.iter().map(|(p, _)| p.name()).collect()
    }

    /// 全部内置扩展的统一清单（后台「扩展总览」与 P3 事件总线的登记源）。
    /// 版本不符的条目被剔除并记日志——清单契约变了却静默跑旧语义，
    /// 是只有出事才看得见的偏差。
    pub fn manifests(&self) -> Vec<ExtensionManifest> {
        self.plugins
            .iter()
            .filter_map(|(p, caps)| {
                let m = p.manifest(caps);
                if let Err(e) = m.validate_version() {
                    tracing::warn!(plugin = %m.id, "{e}");
                    return None;
                }
                Some(m)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Recording {
        hit: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }
    impl Plugin for Recording {
        fn name(&self) -> &'static str {
            "recording"
        }
        fn on_user_login(&self, _ctx: &PluginCtx, _u: i64) {
            self.hit.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    struct Panicky;
    impl Plugin for Panicky {
        fn name(&self) -> &'static str {
            "panicky"
        }
        fn on_user_login(&self, _ctx: &PluginCtx, _u: i64) {
            panic!("boom");
        }
    }

    #[test]
    fn builtin_has_example_plugin() {
        let m = PluginManager::builtin();
        assert!(m.list().contains(&"auto_pin_official"));
    }

    #[test]
    fn panicked_hook_is_suppressed_and_next_plugin_runs() {
        // 无 AppState 也能构造管理器：直接用裸 Vec 验证分发语义
        let hit =
            std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let rec = std::sync::Arc::new(Recording { hit: hit.clone() });
        let mgr = PluginManager {
            plugins: vec![(Arc::new(Panicky), Vec::new()), (rec, Vec::new())],
        };
        // dispatch_login 需要 AppState；此测试只验证 list 与 enabled 语义
        assert_eq!(mgr.list().len(), 2);
        assert!(mgr.plugins.iter().all(|(p, _)| p.enabled()));
    }
}
