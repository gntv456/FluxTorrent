//! 11 封邮件的正文内容函数：只描述「说什么」，骨架/样式全在母版。
//!
//! 每个函数接收已查好的事实（用户名、种子名、理由、有效期…），
//! 不做数据库访问、不决定发给谁——触发时机与收件人仍由调用方掌握。
//! 用户可控字段一律原样传入，转义由母版 [`super::esc`] 统一处理。

use super::{Block, Content, Cta, Lang, Row, Tier, Tone};

/// 渲染上下文：站点名 + 绝对基址 + 收件人语言。
pub struct Ctx<'a> {
    pub site: &'a str,
    /// 无尾斜杠绝对基址；空串表示未配置，链接降级为纯文本提示。
    pub base: &'a str,
    pub lang: Lang,
}

impl<'a> Ctx<'a> {
    fn url(&self, path: &str) -> Option<String> {
        if self.base.is_empty() {
            None
        } else {
            Some(format!("{}{}", self.base, path))
        }
    }

    /// 三语文案选择（cn / tw / en）。
    fn t(&self, cn: &str, tw: &str, en: &str) -> String {
        match self.lang {
            Lang::ZhCn => cn,
            Lang::ZhTw => tw,
            Lang::En => en,
        }
        .to_string()
    }
}

fn blank(c: &Ctx, tier: Tier, tone: Tone) -> Content {
    Content {
        lang: c.lang,
        site: c.site.to_string(),
        base: c.base.to_string(),
        tier,
        tone,
        seal: String::new(),
        category: String::new(),
        title: String::new(),
        preheader: String::new(),
        intro: String::new(),
        result: None,
        info: Vec::new(),
        body: Vec::new(),
        bullets: None,
        cta: None,
        cta_note: None,
        safety: None,
        footer: c.t(
            "本邮件由系统自动发出，回复不会被读取；如需帮助，请通过站内「联系管理组」反馈。",
            "本郵件由系統自動發出，回覆不會被讀取；如需協助，請透過站內「聯絡管理組」回報。",
            "This message was sent automatically and replies are not monitored. \
For help, use Contact Staff on the site.",
        ),
    }
}

fn row(label: impl Into<String>, value: impl Into<String>, chip: bool) -> Row {
    Row {
        label: label.into(),
        value: value.into(),
        chip,
    }
}

/// 链接兜底：按钮点不了时，纯文本里给出完整 URL。
fn copy_hint(c: &Ctx, url: &str) -> String {
    c.t(
        &format!("如果按钮无法点击，请把下面这个完整链接复制到浏览器打开：\n{}", url),
        &format!("如果按鈕無法點擊，請把下面這個完整連結複製到瀏覽器開啟：\n{}", url),
        &format!("If the button does not work, copy this full link into your browser:\n{}", url),
    )
}

fn footer_security(c: &Ctx, cn: &str, tw: &str, en: &str) -> Block {
    Block {
        heading: c.t("安全提示", "安全提示", "Security note"),
        body: c.t(cn, tw, en),
    }
}

/// 1. 邀请注册（第一印象档 + 金线 + 朱砂印）
pub fn invite(c: &Ctx, inviter: &str, code: &str, hours: i64) -> super::Mail {
    let mut m = blank(c, Tier::Hero, Tone::Gold);
    m.seal = c.t("邀", "邀", "I");
    m.category = c.t("邀 请 注 册", "邀 請 註 冊", "INVITATION");
    m.title = c.t(
        &format!("{} 邀请你加入 {}", inviter, c.site),
        &format!("{} 邀請你加入 {}", inviter, c.site),
        &format!("{} invited you to join {}", inviter, c.site),
    );
    m.preheader = c.t(
        &format!("{} 的邀请码随邮件附上，{} 小时内有效。", inviter, hours),
        &format!("{} 的邀請碼隨郵件附上，{} 小時內有效。", inviter, hours),
        &format!(
            "{}'s invite code is inside, valid for {} hours.",
            inviter, hours
        ),
    );
    m.intro = c.t(
        &format!(
            "你收到这封邮件，是因为 {} 通过 {} 的邀请功能，把这个邮箱填进了邀请表单。",
            inviter, c.site
        ),
        &format!(
            "你收到這封郵件，是因為 {} 透過 {} 的邀請功能，把這個信箱填進了邀請表單。",
            inviter, c.site
        ),
        &format!(
            "You are receiving this because {} entered this address in the invite form on {}.",
            inviter, c.site
        ),
    );
    m.body.push(c.t(
        "山海之间，总有一条路通向同好。这里的人一起做种、一起把冷门资源留在网上。",
        "山海之間，總有一條路通向同好。這裡的人一起做種、一起把冷門資源留在網上。",
        "Every path through the mountains leads to someone who shares your taste. \
People here seed together and keep rare files alive.",
    ));
    m.info = vec![
        row(c.t("邀请人", "邀請人", "Invited by"), inviter, false),
        row(
            c.t("有效期", "有效期", "Valid for"),
            c.t(
                &format!("{} 小时", hours),
                &format!("{} 小時", hours),
                &format!("{} hours", hours),
            ),
            false,
        ),
        row(c.t("邀请码", "邀請碼", "Invite code"), code, true),
    ];
    let link = c.url(&format!("/register?invite={}", code));
    if let Some(u) = &link {
        m.cta = Some(Cta {
            label: c.t(
                "接受邀请并注册",
                "接受邀請並註冊",
                "Accept and sign up",
            ),
            url: u.clone(),
        });
        m.cta_note = Some(copy_hint(c, u));
    }
    m.bullets = Some(Block {
        heading: c.t("注册步骤", "註冊步驟", "How to join"),
        body: c.t(
            "用本邮件收到的邮箱地址注册\n填写邀请码并设置密码\n完成邮箱验证后即可进入站点",
            "用本郵件收到的信箱地址註冊\n填寫邀請碼並設定密碼\n完成信箱驗證後即可進入站點",
            "Sign up with the address that received this mail\nEnter the invite code and set a password\nVerify your email to enter the site",
        ),
    });
    m.safety = Some(footer_security(
        c,
        &format!(
            "如果你不认识 {}，或从未期待这封邮件，直接忽略即可：邀请码不会被他人取用，也不会影响你的邮箱。",
            inviter
        ),
        &format!(
            "如果你不認識 {}，或從未期待這封郵件，直接忽略即可：邀請碼不會被他人取用，也不會影響你的信箱。",
            inviter
        ),
        &format!(
            "If you do not know {} or did not expect this mail, simply ignore it. \
The code stays unused and your address is not affected.",
            inviter
        ),
    ));
    let title = m.title.clone();
    super::build_content(&title, m)
}

/// 2. 找回密码（第一印象档）
pub fn password_reset(c: &Ctx, token: &str, minutes: i64) -> super::Mail {
    let mut m = blank(c, Tier::Hero, Tone::Gold);
    m.seal = c.t("钥", "鑰", "K");
    m.category = c.t("密 码 重 置", "密 碼 重 置", "PASSWORD RESET");
    m.title = c.t(
        &format!("重置你的 {} 密码", c.site),
        &format!("重設你的 {} 密碼", c.site),
        &format!("Reset your {} password", c.site),
    );
    m.preheader = c.t(
        "重置链接只能使用一次。",
        "重設連結只能使用一次。",
        "The reset link can be used only once.",
    );
    m.intro = c.t(
        &format!(
            "你收到这封邮件，是因为有人在 {} 的找回密码页面提交了这个邮箱。",
            c.site
        ),
        &format!(
            "你收到這封郵件，是因為有人在 {} 的找回密碼頁面提交了這個信箱。",
            c.site
        ),
        &format!(
            "You are receiving this because someone submitted this address on the \
password reset page of {}.",
            c.site
        ),
    );
    m.body.push(c.t(
        "山海之间，丢一把钥匙不算大事，换一把就好。点下面的按钮，设置新密码。",
        "山海之間，丟一把鑰匙不算大事，換一把就好。點下面的按鈕，設定新密碼。",
        "Losing a key in the mountains is no disaster; make a new one. \
Use the button below to set a new password.",
    ));
    m.info = vec![row(
        c.t("有效期", "有效期", "Valid for"),
        c.t(
            &format!("{} 分钟", minutes),
            &format!("{} 分鐘", minutes),
            &format!("{} minutes", minutes),
        ),
        false,
    )];
    let link = c.url(&format!("/reset?token={}", token));
    if let Some(u) = &link {
        m.cta = Some(Cta {
            label: c.t("设置新密码", "設定新密碼", "Set a new password"),
            url: u.clone(),
        });
        m.cta_note = Some(copy_hint(c, u));
    }
    m.safety = Some(footer_security(
        c,
        "如果你没有申请重置，忽略这封邮件即可，密码不会被改动。若担心账号安全，请登录后检查登录记录并联系管理组。",
        "如果你沒有申請重設，忽略這封郵件即可，密碼不會被改動。若擔心帳號安全，請登入後檢查登入紀錄並聯絡管理組。",
        "If you did not request a reset, ignore this mail; your password stays unchanged. \
If you are unsure about your account, sign in and review your login history.",
    ));
    let title = m.title.clone();
    super::build_content(&title, m)
}

/// 3. 邮箱激活（第一印象档 + 石青待办）
pub fn email_verify(
    c: &Ctx,
    username: &str,
    token: &str,
    hours: i64,
) -> super::Mail {
    let mut m = blank(c, Tier::Hero, Tone::Pending);
    m.seal = c.t("验", "驗", "V");
    m.category = c.t("邮 箱 激 活", "信 箱 啟 用", "EMAIL VERIFY");
    m.title = c.t(
        "请验证你的邮箱",
        "請驗證你的信箱",
        "Verify your email address",
    );
    m.preheader = c.t(
        "验证后账号才能正常收信。",
        "驗證後帳號才能正常收信。",
        "Verify to receive account mail.",
    );
    m.intro = c.t(
        &format!("你收到这封邮件，是因为账号 {} 在 {} 注册时留了这个邮箱。", username, c.site),
        &format!("你收到這封郵件，是因為帳號 {} 在 {} 註冊時留了這個信箱。", username, c.site),
        &format!(
            "You are receiving this because account {} used this address when signing up on {}.",
            username, c.site
        ),
    );
    m.body.push(c.t(
        "完成验证后，审核结果、申诉通知和安全提醒才会正常送达。",
        "完成驗證後，審核結果、申訴通知和安全提醒才會正常送達。",
        "Once verified, review results, appeal notices and security alerts can reach you.",
    ));
    m.info = vec![
        row(c.t("账号", "帳號", "Account"), username, false),
        row(
            c.t("有效期", "有效期", "Valid for"),
            c.t(
                &format!("{} 小时", hours),
                &format!("{} 小時", hours),
                &format!("{} hours", hours),
            ),
            false,
        ),
    ];
    let link = c.url(&format!("/email-verify?token={}", token));
    if let Some(u) = &link {
        m.cta = Some(Cta {
            label: c.t("验证邮箱", "驗證信箱", "Verify email"),
            url: u.clone(),
        });
        m.cta_note = Some(copy_hint(c, u));
    }
    m.safety = Some(footer_security(
        c,
        "如果这不是你注册的账号，忽略即可，邮箱不会被绑定。",
        "如果這不是你註冊的帳號，忽略即可，信箱不會被綁定。",
        "If you did not create this account, ignore this mail; the address will not be linked.",
    ));
    let title = m.title.clone();
    super::build_content(&title, m)
}

/// 4. 种子审核通过（日常档 + 石绿）
pub fn review_approved(
    c: &Ctx,
    torrent_id: i64,
    title: &str,
    subject_override: Option<&str>,
) -> super::Mail {
    let mut m = blank(c, Tier::Daily, Tone::Pass);
    m.seal = c.t("过", "過", "P");
    m.category = c.t("种 子 审 核", "種 子 審 核", "TORRENT REVIEW");
    let subject =
        subject_override.map(|s| s.to_string()).unwrap_or_else(|| {
            c.t(
                &format!("[{}] 你提交的种子已通过审核", c.site),
                &format!("[{}] 你提交的種子已通過審核", c.site),
                &format!("[{}] Your torrent was approved", c.site),
            )
        });
    m.title = c.t(
        "你提交的种子已通过审核",
        "你提交的種子已通過審核",
        "Your torrent was approved",
    );
    m.preheader = c.t(
        "资源已经上线。",
        "資源已經上線。",
        "Your upload is now live.",
    );
    m.intro = c.t(
        &format!("你收到这封邮件，是因为你在 {} 提交的种子完成了审核。", c.site),
        &format!("你收到這封郵件，是因為你在 {} 提交的種子完成了審核。", c.site),
        &format!("You are receiving this because your upload on {} has been reviewed.", c.site),
    );
    m.result = Some(Block {
        heading: c.t("审核结论：通过", "審核結論：通過", "Result: approved"),
        body: c.t(
            "资源已公开，其他用户可以在站点搜索到它。",
            "資源已公開，其他使用者可以在站點搜尋到它。",
            "The torrent is public and searchable on the site.",
        ),
    });
    m.info = vec![
        row(
            c.t("种子 ID", "種子 ID", "Torrent ID"),
            torrent_id.to_string(),
            false,
        ),
        row(c.t("标题", "標題", "Title"), title, false),
    ];
    m.body.push(c.t(
        "接下来能不能留住下载者，取决于保种。欢迎继续挂在做种队列里。",
        "接下來能不能留住下載者，取決於保種。歡迎繼續掛在做種佇列裡。",
        "Whether downloaders stay depends on seeding. Keep it in your seed queue if you can.",
    ));
    if let Some(u) = c.url(&format!("/torrents/{}", torrent_id)) {
        m.cta = Some(Cta {
            label: c.t("查看种子页面", "查看種子頁面", "View torrent"),
            url: u,
        });
    }
    m.safety = Some(footer_security(
        c,
        "如果这不是你提交的资源，请通过站内「联系管理组」告知我们。",
        "如果這不是你提交的資源，請透過站內「聯絡管理組」告知我們。",
        "If you did not upload this torrent, please contact staff through the site.",
    ));
    super::build_content(&subject, m)
}

/// 5. 种子审核未通过（日常档 + 朱砂）
pub fn review_rejected(
    c: &Ctx,
    torrent_id: i64,
    title: &str,
    reason: &str,
    subject_override: Option<&str>,
) -> super::Mail {
    let mut m = blank(c, Tier::Daily, Tone::Reject);
    m.seal = c.t("驳", "駁", "R");
    m.category = c.t("种 子 审 核", "種 子 審 核", "TORRENT REVIEW");
    let subject =
        subject_override.map(|s| s.to_string()).unwrap_or_else(|| {
            c.t(
                &format!("[{}] 你提交的种子未通过审核", c.site),
                &format!("[{}] 你提交的種子未通過審核", c.site),
                &format!("[{}] Your torrent was not approved", c.site),
            )
        });
    m.title = c.t(
        "你提交的种子未通过审核",
        "你提交的種子未通過審核",
        "Your torrent was not approved",
    );
    m.preheader = c.t(
        "请按建议修改后重新提交。",
        "請依建議修改後重新提交。",
        "Fix and resubmit if you wish.",
    );
    m.intro = c.t(
        &format!("你收到这封邮件，是因为你在 {} 提交的种子没有通过审核。", c.site),
        &format!("你收到這封郵件，是因為你在 {} 提交的種子沒有通過審核。", c.site),
        &format!(
            "You are receiving this because your upload on {} did not pass review.",
            c.site
        ),
    );
    let why = if reason.trim().is_empty() {
        c.t(
            "审核未通过，但审核记录里没有填写具体理由。",
            "審核未通過，但審核紀錄裡沒有填寫具體理由。",
            "The upload was declined, but no specific reason was recorded.",
        )
    } else {
        reason.to_string()
    };
    m.result = Some(Block {
        heading: c.t(
            "审核结论：未通过",
            "審核結論：未通過",
            "Result: not approved",
        ),
        body: why,
    });
    m.info = vec![
        row(
            c.t("种子 ID", "種子 ID", "Torrent ID"),
            torrent_id.to_string(),
            false,
        ),
        row(c.t("标题", "標題", "Title"), title, false),
    ];
    m.body.push(c.t(
        "被驳回不代表资源没有价值，多数情况是简介、分类或文件结构与站规不符。",
        "被駁回不代表資源沒有價值，多數情況是簡介、分類或檔案結構與站規不符。",
        "A rejection does not mean the upload is worthless. Usually the description, \
category or file layout did not match the rules.",
    ));
    m.bullets = Some(Block {
        heading: c.t("修改建议", "修改建議", "Suggestions"),
        body: c.t(
            "补全简介、截图与文件清单\n核对分类与标签是否准确\n调整后重新打包并提交",
            "補全簡介、截圖與檔案清單\n核對分類與標籤是否準確\n調整後重新打包並提交",
            "Complete the description, screenshots and file list\nCheck the category and tags\nRepack and submit again",
        ),
    });
    if let Some(u) = c.url("/upload") {
        m.cta = Some(Cta {
            label: c.t(
                "修改后重新提交",
                "修改後重新提交",
                "Revise and resubmit",
            ),
            url: u,
        });
    }
    m.safety = Some(footer_security(
        c,
        "如果这与你的提交无关，请通过站内「联系管理组」核对。",
        "如果這與你的提交無關，請透過站內「聯絡管理組」核對。",
        "If this does not match your upload, check with staff through the site.",
    ));
    super::build_content(&subject, m)
}

/// 6. 入站申请通过（日常档 + 石绿）
pub fn application_approved(
    c: &Ctx,
    username: &str,
    code: &str,
    email: &str,
    hours: i64,
) -> super::Mail {
    let mut m = blank(c, Tier::Daily, Tone::Pass);
    m.seal = c.t("准", "准", "A");
    m.category = c.t("入 站 申 请", "入 站 申 請", "APPLICATION");
    let subject = c.t(
        &format!("[{}] 你的入站申请已通过", c.site),
        &format!("[{}] 你的入站申請已通過", c.site),
        &format!("[{}] Your application was approved", c.site),
    );
    m.title = c.t(
        "你的入站申请已通过",
        "你的入站申請已通過",
        "Your application was approved",
    );
    m.preheader = c.t(
        "邀请码已附上，请在有效期内注册。",
        "邀請碼已附上，請在有效期內註冊。",
        "Your invite code is inside; register before it expires.",
    );
    m.intro = c.t(
        &format!("你收到这封邮件，是因为你在 {} 提交的入站申请已审核完成。", c.site),
        &format!("你收到這封郵件，是因為你在 {} 提交的入站申請已審核完成。", c.site),
        &format!("You are receiving this because your application on {} has been reviewed.", c.site),
    );
    m.result = Some(Block {
        heading: c.t("申请结论：通过", "申請結論：通過", "Result: approved"),
        body: c.t(
            "请在有效期内用随信邀请码完成注册。",
            "請在有效期內用隨信邀請碼完成註冊。",
            "Use the invite code in this mail to finish signing up before it expires.",
        ),
    });
    m.info = vec![
        row(c.t("用户名", "使用者名稱", "Username"), username, false),
        row(c.t("注册邮箱", "註冊信箱", "Email"), email, false),
        row(c.t("邀请码", "邀請碼", "Invite code"), code, true),
        row(
            c.t("有效期", "有效期", "Valid for"),
            c.t(
                &format!("{} 小时", hours),
                &format!("{} 小時", hours),
                &format!("{} hours", hours),
            ),
            false,
        ),
    ];
    if let Some(u) = c.url(&format!("/register?code={}", code)) {
        m.cta = Some(Cta {
            label: c.t("完成注册", "完成註冊", "Finish signing up"),
            url: u.clone(),
        });
        m.cta_note = Some(copy_hint(c, &u));
    }
    m.safety = Some(footer_security(
        c,
        "邀请码只可使用一次，请不要转发给他人；它不是你的登录密码。",
        "邀請碼只可使用一次，請不要轉發給他人；它不是你的登入密碼。",
        "The code is single-use. Do not forward it; it is not your password.",
    ));
    super::build_content(&subject, m)
}

/// 7. 入站申请未通过（日常档 + 朱砂）
pub fn application_rejected(
    c: &Ctx,
    username: &str,
    note: &str,
    email: &str,
    apply_no: Option<i64>,
) -> super::Mail {
    let mut m = blank(c, Tier::Daily, Tone::Reject);
    m.seal = c.t("未", "未", "D");
    m.category = c.t("入 站 申 请", "入 站 申 請", "APPLICATION");
    let subject = c.t(
        &format!("[{}] 你的入站申请未通过", c.site),
        &format!("[{}] 你的入站申請未通過", c.site),
        &format!("[{}] Your application was not approved", c.site),
    );
    m.title = c.t(
        "你的入站申请未通过",
        "你的入站申請未通過",
        "Your application was not approved",
    );
    m.preheader = c.t(
        "下面是本次结论与可以改进的地方。",
        "下面是本次結論與可以改進的地方。",
        "Here is the result and what you can improve.",
    );
    m.intro = c.t(
        &format!("你收到这封邮件，是因为你在 {} 提交的入站申请已审核完成。", c.site),
        &format!("你收到這封郵件，是因為你在 {} 提交的入站申請已審核完成。", c.site),
        &format!("You are receiving this because your application on {} has been reviewed.", c.site),
    );
    let why = if note.trim().is_empty() {
        c.t(
            "本次申请未通过，但没有附加说明。",
            "本次申請未通過，但沒有附加說明。",
            "The application was declined without a specific note.",
        )
    } else {
        note.to_string()
    };
    m.result = Some(Block {
        heading: c.t(
            "申请结论：未通过",
            "申請結論：未通過",
            "Result: not approved",
        ),
        body: why,
    });
    let mut info = vec![
        row(c.t("用户名", "使用者名稱", "Username"), username, false),
        row(c.t("联系邮箱", "聯絡信箱", "Email"), email, false),
    ];
    if let Some(n) = apply_no {
        info.push(row(
            c.t("申请编号", "申請編號", "Application no."),
            n.to_string(),
            false,
        ));
    }
    m.info = info;
    m.body.push(c.t(
        "未通过常见于申请信息不完整、答非所问，或与站点方向不匹配。",
        "未通過常見於申請資訊不完整、答非所問，或與站點方向不匹配。",
        "Common causes are incomplete answers, off-topic replies, or a mismatch with the site.",
    ));
    m.bullets = Some(Block {
        heading: c.t("下一步", "下一步", "Next steps"),
        body: c.t(
            "阅读站规与申请说明，确认理解站点定位\n间隔一段时间后再提交，并补全信息\n有疑问可通过「联系管理组」询问",
            "閱讀站規與申請說明，確認理解站點定位\n間隔一段時間後再提交，並補全資訊\n有疑問可透過「聯絡管理組」詢問",
            "Read the rules and application guide\nWait a while, then apply again with complete details\nAsk staff if anything is unclear",
        ),
    });
    if let Some(u) = c.url("/applications") {
        m.cta = Some(Cta {
            label: c.t("查看申请说明", "查看申請說明", "Read the guide"),
            url: u,
        });
    }
    m.safety = Some(footer_security(
        c,
        "如果你没有提交过申请，请忽略这封邮件，或联系管理组核对邮箱。",
        "如果你沒有提交過申請，請忽略這封郵件，或聯絡管理組核對信箱。",
        "If you never applied, ignore this mail or ask staff to check the address.",
    ));
    super::build_content(&subject, m)
}

/// 8. 申诉结果（日常档；成立=石绿，驳回=朱砂，减轻=石绿）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Accepted,
    Rejected,
    Mitigated,
}

pub fn appeal_result(
    c: &Ctx,
    appeal_no: &str,
    verdict: Verdict,
    note: &str,
    effect: &str,
    subject_override: Option<&str>,
) -> super::Mail {
    let accepted = verdict != Verdict::Rejected;
    let mut m = blank(
        c,
        Tier::Daily,
        if accepted { Tone::Pass } else { Tone::Reject },
    );
    m.seal = c.t("复", "復", "L");
    m.category = c.t("申 诉 结 果", "申 訴 結 果", "APPEAL");
    let subject =
        subject_override.map(|s| s.to_string()).unwrap_or_else(|| {
            if accepted {
                c.t(
                    &format!("[{}] 你的申诉已通过", c.site),
                    &format!("[{}] 你的申訴已通過", c.site),
                    &format!("[{}] Your appeal was accepted", c.site),
                )
            } else {
                c.t(
                    &format!("[{}] 你的申诉未通过", c.site),
                    &format!("[{}] 你的申訴未通過", c.site),
                    &format!("[{}] Your appeal was declined", c.site),
                )
            }
        });
    m.title = match verdict {
        Verdict::Accepted => c.t(
            "你的申诉已通过",
            "你的申訴已通過",
            "Your appeal was accepted",
        ),
        Verdict::Mitigated => c.t(
            "你的申诉已部分通过",
            "你的申訴已部分通過",
            "Your appeal was partly accepted",
        ),
        Verdict::Rejected => c.t(
            "你的申诉未通过",
            "你的申訴未通過",
            "Your appeal was declined",
        ),
    };
    m.preheader = c.t(
        "下面是结论与账号状态。",
        "下面是結論與帳號狀態。",
        "Here is the result and your account status.",
    );
    m.intro = c.t(
        &format!("你收到这封邮件，是因为你在 {} 提交的申诉已经复核完成。", c.site),
        &format!("你收到這封郵件，是因為你在 {} 提交的申訴已經複核完成。", c.site),
        &format!("You are receiving this because your appeal on {} has been reviewed.", c.site),
    );
    let heading = match verdict {
        Verdict::Accepted => {
            c.t("申诉结论：成立", "申訴結論：成立", "Result: accepted")
        }
        Verdict::Mitigated => c.t(
            "申诉结论：减轻",
            "申訴結論：減輕",
            "Result: partially accepted",
        ),
        Verdict::Rejected => {
            c.t("申诉结论：驳回", "申訴結論：駁回", "Result: declined")
        }
    };
    let body = if note.trim().is_empty() {
        c.t(
            "复核已完成，结论如上。",
            "複核已完成，結論如上。",
            "The review is complete; see the result above.",
        )
    } else {
        note.to_string()
    };
    m.result = Some(Block { heading, body });
    m.info = vec![
        row(c.t("申诉编号", "申訴編號", "Appeal no."), appeal_no, false),
        row(c.t("处理结果", "處理結果", "Outcome"), effect, accepted),
    ];
    if !accepted {
        m.body.push(c.t(
            "申诉不是一次重审全部记录，只针对本次处理是否有误。",
            "申訴不是一次重審全部紀錄，只針對本次處理是否有誤。",
            "An appeal reviews whether this decision was correct, not your whole history.",
        ));
        m.bullets = Some(Block {
            heading: c.t("补充材料建议", "補充材料建議", "What to include next time"),
            body: c.t(
                "能说明当时网络环境与客户端的信息\n与本次处理相关的日志、截图或凭证\n对争议点的直接说明，而不是重复结论",
                "能說明當時網路環境與用戶端的資訊\n與本次處理相關的日誌、截圖或憑證\n對爭議點的直接說明，而不是重複結論",
                "Details about your network and client at the time\nLogs, screenshots or proof related to the decision\nA direct answer to the disputed point",
            ),
        });
        if let Some(u) = c.url("/rules") {
            m.cta = Some(Cta {
                label: c.t("阅读申诉规则", "閱讀申訴規則", "Read appeal rules"),
                url: u,
            });
        }
    } else {
        m.body.push(c.t(
            "如结论涉及限制变更，账号状态会在短时间内同步更新。",
            "如結論涉及限制變更，帳號狀態會在短時間內同步更新。",
            "If the outcome changed any restriction, your account status will update shortly.",
        ));
        if let Some(u) = c.url("/me") {
            m.cta = Some(Cta {
                label: c.t("查看账号状态", "查看帳號狀態", "View account"),
                url: u,
            });
        }
    }
    m.safety = Some(footer_security(
        c,
        "如果这不是你提交的申诉，请通过站内「联系管理组」告知我们。",
        "如果這不是你提交的申訴，請透過站內「聯絡管理組」告知我們。",
        "If you did not file this appeal, please contact staff through the site.",
    ));
    super::build_content(&subject, m)
}

/// 9. 账号状态变更（警示档；封禁/禁止登录=朱砂，禁言=琥珀，恢复=石绿）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusKind {
    Ban,
    Mute,
    Warn,
    Restore,
}

pub fn account_status(
    c: &Ctx,
    username: &str,
    kind: StatusKind,
    label: &str,
    reason: &str,
    extra: Option<(&str, &str)>,
    subject_override: Option<&str>,
) -> super::Mail {
    let (tone, seal, cat) = match kind {
        StatusKind::Ban => (
            Tone::Reject,
            c.t("封", "封", "B"),
            c.t("账 号 状 态", "帳 號 狀 態", "ACCOUNT"),
        ),
        StatusKind::Mute => (
            Tone::Warn,
            c.t("禁", "禁", "M"),
            c.t("账 号 状 态", "帳 號 狀 態", "ACCOUNT"),
        ),
        StatusKind::Warn => (
            Tone::Warn,
            c.t("警", "警", "W"),
            c.t("账 号 状 态", "帳 號 狀 態", "ACCOUNT"),
        ),
        StatusKind::Restore => (
            Tone::Pass,
            c.t("复", "復", "R"),
            c.t("账 号 状 态", "帳 號 狀 態", "ACCOUNT"),
        ),
    };
    let mut m = blank(c, Tier::Alert, tone);
    m.seal = seal;
    m.category = cat;
    let subject =
        subject_override.map(|s| s.to_string()).unwrap_or_else(|| {
            c.t(
                &format!("[{}] 你的账号状态有变更", c.site),
                &format!("[{}] 你的帳號狀態有變更", c.site),
                &format!("[{}] Your account status changed", c.site),
            )
        });
    m.title = c.t(
        &format!("账号状态：{}", label),
        &format!("帳號狀態：{}", label),
        &format!("Account status: {}", label),
    );
    m.preheader = c.t(
        "请查看处理原因与后续操作。",
        "請查看處理原因與後續操作。",
        "See the reason and what to do next.",
    );
    m.intro = c.t(
        &format!("你收到这封邮件，是因为你在 {} 的账号状态刚刚发生了变更。", c.site),
        &format!("你收到這封郵件，是因為你在 {} 的帳號狀態剛剛發生了變更。", c.site),
        &format!("You are receiving this because your account on {} just changed status.", c.site),
    );
    let body = if reason.trim().is_empty() {
        match kind {
            StatusKind::Restore => c.t(
                "此前的限制已解除。",
                "此前的限制已解除。",
                "The earlier restriction has been lifted.",
            ),
            _ => c.t(
                "处理记录里没有填写具体原因，请联系管理组核对。",
                "處理紀錄裡沒有填寫具體原因，請聯絡管理組核對。",
                "No specific reason was recorded. Please ask staff to check.",
            ),
        }
    } else {
        reason.to_string()
    };
    m.result = Some(Block {
        heading: c.t("当前状态", "目前狀態", "Current status"),
        body: format!("{}：{}", label, body),
    });
    let mut info = vec![row(
        c.t("用户名", "使用者名稱", "Username"),
        username,
        false,
    )];
    if let Some((k, v)) = extra {
        info.push(row(k, v, true));
    }
    m.info = info;
    m.body.push(match kind {
        StatusKind::Ban => c.t(
            "账号已无法登录，下载与做种同时停止。可以在站内提交申诉，说明当时的情况。",
            "帳號已無法登入，下載與做種同時停止。可以在站內提交申訴，說明當時的情況。",
            "Sign-in is blocked and transfers stopped. You may file an appeal explaining the situation.",
        ),
        StatusKind::Mute => c.t(
            "账号仍可登录，但发言与部分互动受限。禁言会到期自动解除。",
            "帳號仍可登入，但發言與部分互動受限。禁言會到期自動解除。",
            "You can still sign in, but posting and some actions are restricted until the mute expires.",
        ),
        StatusKind::Warn => c.t(
            "这是一次记录在案的提醒，暂不限制账号功能，但再次发生会加重处理。",
            "這是一次記錄在案的提醒，暫不限制帳號功能，但再次發生會加重處理。",
            "This warning is on record. Your account works normally, but a repeat will be treated more strictly.",
        ),
        StatusKind::Restore => c.t(
            "账号功能已恢复。请确认此前的限制原因已经处理完毕。",
            "帳號功能已恢復。請確認此前的限制原因已經處理完畢。",
            "Your account is restored. Please make sure the earlier issue has been resolved.",
        ),
    });
    let cta = match kind {
        StatusKind::Ban | StatusKind::Mute => c.url("/appeal").map(|u| Cta {
            label: c.t("提交申诉", "提交申訴", "File an appeal"),
            url: u,
        }),
        _ => c.url("/me").map(|u| Cta {
            label: c.t("查看账号", "查看帳號", "View account"),
            url: u,
        }),
    };
    m.cta = cta;
    m.safety = Some(footer_security(
        c,
        "如果这不是你的账号，请通过站内「联系管理组」告知我们。",
        "如果這不是你的帳號，請透過站內「聯絡管理組」告知我們。",
        "If this is not your account, please contact staff through the site.",
    ));
    super::build_content(&subject, m)
}

/// 10. H&R 预警（警示档 + 琥珀）
pub fn hr_warning(
    c: &Ctx,
    torrent_id: i64,
    title: &str,
    seeded: f64,
    required: f64,
    deadline: &str,
    subject_override: Option<&str>,
) -> super::Mail {
    let mut m = blank(c, Tier::Alert, Tone::Warn);
    m.seal = c.t("预", "預", "H");
    m.category = c.t("H & R 预 警", "H & R 預 警", "H&R WARNING");
    let subject =
        subject_override.map(|s| s.to_string()).unwrap_or_else(|| {
            c.t(
                &format!("[{}] H&R 预警：请尽快恢复做种", c.site),
                &format!("[{}] H&R 預警：請盡快恢復做種", c.site),
                &format!("[{}] H&R warning: resume seeding", c.site),
            )
        });
    m.title = c.t(
        "做种时长不足，请尽快恢复",
        "做種時長不足，請盡快恢復",
        "Seeding time is below the requirement",
    );
    m.preheader = c.t(
        "超时会记一次 H&R。",
        "逾時會記一次 H&R。",
        "Missing the deadline records an H&R.",
    );
    m.intro = c.t(
        &format!("你收到这封邮件，是因为你在 {} 下载的种子做种时长不足。", c.site),
        &format!("你收到這封郵件，是因為你在 {} 下載的種子做種時長不足。", c.site),
        &format!("You are receiving this because a torrent you downloaded on {} is under-seeded.", c.site),
    );
    m.result = Some(Block {
        heading: c.t("预警：做种时长不足", "預警：做種時長不足", "Warning: seeding time short"),
        body: c.t(
            "在截止时间前补足做种时长，本次 H&R 不会记录。",
            "在截止時間前補足做種時長，本次 H&R 不會記錄。",
            "Top up the seeding time before the deadline and no H&R will be recorded.",
        ),
    });
    m.info = vec![
        row(
            c.t("种子 ID", "種子 ID", "Torrent ID"),
            torrent_id.to_string(),
            false,
        ),
        row(c.t("标题", "標題", "Title"), title, false),
        row(
            c.t("已做种", "已做種", "Seeded"),
            format!("{:.1} h", seeded),
            false,
        ),
        row(
            c.t("要求", "要求", "Required"),
            format!("{:.1} h", required),
            false,
        ),
        row(c.t("截止时间", "截止時間", "Deadline"), deadline, true),
    ];
    m.body.push(c.t(
        "H&R 是站点维持资源存活的最低要求；补足时长后，记录会自动消除。",
        "H&R 是站點維持資源存活的最低要求；補足時長後，紀錄會自動消除。",
        "H&R keeps torrents alive. Once the time is met, the record clears automatically.",
    ));
    if let Some(u) = c.url("/me") {
        m.cta = Some(Cta {
            label: c.t("查看我的做种", "查看我的做種", "View my seeding"),
            url: u,
        });
    }
    m.safety = Some(footer_security(
        c,
        "如果种子已删除或设备长期离线，请尽快登录站点说明情况。",
        "如果種子已刪除或裝置長期離線，請盡快登入站點說明情況。",
        "If the torrent is gone or your device is offline, sign in and tell staff as soon as possible.",
    ));
    super::build_content(&subject, m)
}

/// 11. 通用站内通知（管理员群发 / 私信转录 / 赞助致谢 / SMTP 自检）。
/// 管理员自由文本不做加工，仅按空行分段；仍套同一骨架。
pub fn generic_notice(
    c: &Ctx,
    subject: &str,
    body: &str,
    category: &str,
    tone: Tone,
) -> super::Mail {
    let mut m = blank(c, Tier::Daily, tone);
    m.seal = c.t("通", "通", "N");
    m.category = category.to_string();
    m.title = subject.to_string();
    m.preheader = body.lines().next().unwrap_or("").to_string();
    m.intro = c.t(
        &format!("你收到这封邮件，是因为 {} 的管理组向你的账号发送了通知。", c.site),
        &format!("你收到這封郵件，是因為 {} 的管理組向你的帳號發送了通知。", c.site),
        &format!("You are receiving this because the {} team sent a notice to your account.", c.site),
    );
    for p in body.split("\n\n") {
        if !p.trim().is_empty() {
            m.body.push(p.trim().to_string());
        }
    }
    m.cta_note = c.url("").map(|u| {
        c.t(
            &format!("站点地址：{}", u),
            &format!("站點網址：{}", u),
            &format!("Site: {}", u),
        )
    });
    super::build_content(subject, m)
}
