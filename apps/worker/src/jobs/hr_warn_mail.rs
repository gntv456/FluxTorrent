//! H&R 警告阶梯的站内信正文（0388）。
//!
//! 从 hr_warn_ladder.rs 按域拆出：阶梯主逻辑已接近 300 行门禁，
//! 而「第 1 档只警告」的全部可见效果就是这封信。

use chrono::{DateTime, Utc};

/// 阶梯变动时发给用户的站内信正文。
///
/// 第 1 档不动任何权限、也没有任何读口：没有这封信，「警告」在用户侧
/// 完全不存在（他不知道被警告、不知道为什么、更不知道何时恢复）。
pub(super) fn warn_mail(
    open: i32,
    lvl: i32,
    restrict_download: bool,
    block_upload: bool,
    block_invite: bool,
    expires_at: DateTime<Utc>,
    hit_cap: bool,
) -> String {
    let mut blocked = Vec::new();
    if restrict_download {
        blocked.push("下载");
    }
    if block_upload {
        blocked.push("发布新种");
    }
    if block_invite {
        blocked.push("生成邀请码");
    }
    let action = if blocked.is_empty() {
        "本次仅警告，暂不限制任何权限".to_string()
    } else {
        format!("已暂停：{}", blocked.join("、"))
    };
    let mut body = format!(
        "您当前有 {open} 条未解决的 H&R 违规，站点已按警告阶梯记为第 \
         {lvl} 档（共 4 档）。\n本次处置：{action}。\n\
         本档警告到期时间：{}（UTC）。\n\
         恢复方式：补足做种时长达标后违规会自动消除；也可在「我的 H&R」\
         用魔力自助免罪。未解决违规全部处理完毕且本档警告到期后，\
         权限自动恢复。",
        expires_at.format("%Y-%m-%d %H:%M")
    );
    if hit_cap {
        body.push_str("\n本次顺延已达站点上限，管理组将人工复核您的账号。");
    }
    body
}
