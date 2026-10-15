//! 内置示例插件（自 plugins.rs 拆出，守 300 行门禁）：
//! 官种发布后自动置顶——仅靠注册 Hook + 声明式 DB 能力实现，
//! 不侵入 torrents 领域层，也不再持 `&AppState` 跑裸 SQL。

use crate::plugin_ctx::{Bind, DbCapability, DbMode, PluginCtx};
use crate::plugins::Plugin;

/// 官种（official_tag=TRUE）发布后自动置顶（sticky=TRUE）。
pub(crate) struct AutoPinOfficial;

/// auto_pin_official 的 DB 能力：只写 `torrents.sticky`，读 WHERE 用到的
/// `id` / `official_tag`；禁止无界写。其它表/列一概碰不到。
pub(crate) fn auto_pin_official_caps() -> Vec<DbCapability> {
    vec![DbCapability {
        table: "torrents".into(),
        columns: ["sticky", "id", "official_tag"]
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        mode: DbMode::Write,
        allow_unbounded: false,
    }]
}

impl Plugin for AutoPinOfficial {
    fn name(&self) -> &'static str {
        "auto_pin_official"
    }

    fn on_torrent_upload(
        &self,
        ctx: &PluginCtx,
        torrent_id: i64,
        _owner_id: i64,
    ) {
        // hook 已在 blocking 线程；db_exec 内部 block_on（同旧版语义），
        // 且先过 sql_guard（本句：UPDATE torrents SET sticky WHERE id/official_tag，
        // 全部在声明能力内）。
        match ctx.db_exec(
            "UPDATE torrents SET sticky = TRUE WHERE id = $1 AND official_tag",
            &[Bind::I64(torrent_id)],
        ) {
            Ok(r) if r.rows_affected() > 0 => {
                ctx.log_info("official torrent pinned");
            }
            Ok(_) => {}
            Err(e) => {
                tracing::error!(
                    plugin = "auto_pin_official",
                    ?e,
                    "pin update failed"
                );
            }
        }
    }
}
