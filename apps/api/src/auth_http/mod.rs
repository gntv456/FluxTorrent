//! 认证与用户自助（M01）：注册/登录/登出、me 系列、公开主页、抓取列表、设置。
//! 从 http.rs 机械外移（审查路线图第 4 周「拆上帝文件」第四段）。
//! 鉴权设施本体（require_auth/client_ip/UserStatusCache/throttle/ip_banned）
//! 仍留在 http.rs（全仓共用）。
//! 按域拆分（300 行门禁）：注册 register.rs / 登录 login.rs / 登出 logout.rs /
//! me 基础面 me.rs（perms/me/logins）/ me 安全面 me_security.rs（passkey/密码）/
//! 公开主页 profile.rs + user_torrentlist.rs / me 总览与设置 me_settings.rs + me_settings_kv.rs。

mod login;
mod login_types;
mod logout;
mod me;
mod me_history;
mod me_security;
mod me_settings;
mod me_settings_kv;
mod profile;
mod profile_types;
mod register;
mod user_torrentlist;

pub use login::*;
pub use logout::*;
pub use me::*;
pub use me_history::*;
pub use me_security::*;
pub use me_settings::*;
pub use me_settings_kv::*;
pub use profile::*;
pub use register::*;
pub use user_torrentlist::*;
