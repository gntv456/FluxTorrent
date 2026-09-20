//! 找回密码 + 邮件通道 + H&R 追责 + 等级自动升降 + 申诉 + 补签卡使用
//! （NexusPHP 对比缺口补齐，迁移 0020）。
//! 按域拆分（300 行门禁）：登录辅助在 login_aux.rs，邮件在 mail.rs，找回密码端点在 reset.rs，验证码在
//! captcha.rs，H&R 在 hr.rs，申诉在 appeals.rs，补签卡/等级/愿望单在 misc.rs。

mod appeals;
mod captcha;
mod hr;
mod login_aux;
mod mail;
mod misc;
mod reset;

use appeals::*;
pub use captcha::*;
use hr::*;
use login_aux::*;
pub use mail::*;
use misc::*;
use reset::*;

pub fn mount_gaps(scope: actix_web::Scope) -> actix_web::Scope {
    scope
        .service(ban_log)
        .service(confirm_resend)
        // 找回密码
        .service(password_forgot)
        .service(password_reset)
        // 验证码（注册用）
        .service(captcha_issue)
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
        // 补签卡使用
        .service(resub_use)
        // 等级
        .service(class_rules_list)
        .service(my_class_progress)
        .service(wishlist_list)
        .service(wishlist_add)
        .service(wishlist_remove)
}
