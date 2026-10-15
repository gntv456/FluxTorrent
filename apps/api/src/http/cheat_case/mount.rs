//! 反作弊案件路由挂载（0367 M1）。
//! 从 http/mod.rs 的 v1_scope 长链中拆出，守 300 行门禁；
//! 模式同 admin_p3_http/mount.rs。

use super::*;

pub fn mount_cheat_cases(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(cheat_case_open)
        .service(cheat_case_list)
        .service(cheat_case_detail)
        .service(cheat_case_claim)
        .service(cheat_case_note)
        // 0369 M3：处置（四眼 pending）+ 高危批准
        .service(cheat_case_resolve)
        .service(cheat_case_approve)
}
