//! 找回密码 + 邮件通道 + H&R 追责 + 等级自动升降 + 申诉 + 补签卡使用
//! （NexusPHP 对比缺口补齐，迁移 0020）。
//! 按域拆分（300 行门禁）：登录辅助在 login_aux.rs，邮件在 mail.rs，找回密码端点在 reset.rs，验证码在
//! captcha.rs，H&R 在 hr.rs，申诉在 appeals.rs，补签卡/等级在 misc.rs，愿望单在 wishlist.rs。

mod appeal_unban;
mod appeals;
mod applications;
mod captcha;
pub mod captcha_drivers;
mod cheat_appeal_review;
pub(crate) mod cheat_appeal_reverse;
mod cheat_appeals;
mod classes_public;
mod hr;
mod login_aux;
mod mail;
mod misc;
mod pow;
mod reg_quiz;
mod reset;
mod wishlist;

use appeals::*;
use applications::*;
pub use captcha::*;
use cheat_appeal_review::*;
use cheat_appeals::*;
use classes_public::*;
use hr::*;
use login_aux::*;
pub use mail::*;
use misc::*;
pub use pow::*;
pub use reg_quiz::*;
use reset::*;
use wishlist::*;

pub fn mount_gaps(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(ban_log)
        // 找回密码
        .service(password_forgot)
        .service(password_reset)
        // 验证码（注册用）
        .service(captcha_issue)
        // 登录 PoW 挑战（sha256-pow-v1，连续失败后触发）
        .service(pow_challenge)
        // 注册答题抽题（0372 M5，票据一次性）
        .service(quiz_challenge)
        // H&R
        .service(my_hr_status)
        .service(hr_pardon)
        .service(hr_pardon_batch)
        .service(hr_self_pardon)
        // 申诉
        .service(appeal_create)
        .service(appeal_my)
        .service(appeal_queue)
        .service(appeal_handle)
        // 作弊处置申诉（M3）：令牌入口 + 用户自助 + 员工复核
        .service(cheat_appeal_by_token)
        .service(cheat_appeal_submit)
        .service(cheat_appeal_my)
        .service(cheat_appeal_queue)
        .service(cheat_appeal_resolve)
        // 补签卡使用
        .service(resub_use)
        // 等级
        .service(class_rules_list)
        .service(my_class_progress)
        // 申请制入站（0227）
        .service(apply_submit)
        .service(apply_queue)
        .service(apply_decide)
        // P2 触点补齐（0226）：等级公开页 + 自助解封
        .service(classes_public)
        .service(self_unban)
        .service(wishlist_list)
        .service(wishlist_add)
        .service(wishlist_remove)
}
