//! 站点设定（hxpt / NexusPHP settings.php 复刻）—— 方案 P0 后端。
//!
//! 与既有的 `GET/PUT /admin/settings`（admin_http.rs，键值对原型）并存，本模块提供
//! 「声明 + 分组批量 + 校验 + 审计 + 缓存失效」协议。
//! 按域拆分（300 行门禁）：元数据行与角色助手在 meta.rs，校验引擎在 engine.rs，
//! schema 端点在 schema.rs，单字段预校验在 validate.rs，分组保存与缓存失效在
//! groups.rs，历史在 history.rs，导出/导入在 export.rs；挂载留在此。

mod engine;
mod export;
mod groups;
mod history;
mod meta;
mod schema;
mod validate;

use export::*;
use groups::*;
use history::*;
use schema::*;
use validate::*;

pub fn mount_settings(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(settings_schema)
        .service(settings_groups_put)
        .service(settings_validate)
        .service(settings_history)
        .service(settings_export)
        .service(settings_import)
}
