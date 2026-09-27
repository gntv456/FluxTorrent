//! M21 绩效考核 + M22 任务中心 + M19 保种区前台 HTTP 接口。
//! 按域拆分（300 行门禁）：绩效考核在 jixiao.rs，任务中心在 tasks.rs，
//! 考核考试在 exams.rs，保种/插件总览/复活在 preserve.rs；挂载留在此。

mod exams;
mod jixiao;
mod jixiao_claim;
mod jixiao_compute;
mod onboarding;
mod preserve;
mod resurrections;
mod task_overview;
mod tasks;

use exams::*;
pub use jixiao::*;
use jixiao_claim::*;
pub use onboarding::*;
use preserve::*;
use resurrections::*;
use task_overview::*;
use tasks::*;

pub fn mount_ops(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        // M21 绩效考核
        .service(jixiao_types)
        .service(jixiao_me)
        .service(jixiao_claim)
        .service(jixiao_my)
        // M22 任务中心
        .service(task_list)
        .service(task_overview)
        .service(task_claim)
        // 考核引擎（0093）：我的考核进度（含当前值 vs 目标）
        .service(my_exams)
        // M19 保种区
        .service(preserve_list)
        .service(preserve_claim)
        // 复活任务（0073，U3D Graveyard 口径）
        .service(resurrection_list)
        .service(resurrection_claim)
        .service(resurrection_my)
        // M28 插件
        .service(plugins_overview)
        // C4 新手运营双模板（0226）
        .service(onboarding_status)
        .service(onboarding_apply)
}
