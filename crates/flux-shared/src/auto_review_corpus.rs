//! 金标准回归集（§18.3，≥60 例）——从 auto_review.rs 的 tests 模块外置
//! 数据文件加载不符合本仓单文件习惯，故以独立测试模块数据表落在
//! flux-shared 内。见 auto_review::tests::golden_corpus_sixty_plus。

use super::auto_review::{
    assess, Action, EngineConfig, PublisherProfile, TorrentSnapshot,
};

const LONG: &str =
    "这是一份足够长的资源描述，包含来源说明、压制参数、字幕信息与观看建议，\
        全文超过一百字符以满足 H2 的下限与软规则 S2 的长度档要求。\
        补充第二段介绍音轨与章节信息，以及制作花絮和字幕组的说明，\
        再补一段给收藏党看的版本对比：本版相对旧版修正了部分译名，\
        并附完整中文特效字幕，适合首次收藏。";

/// 每例：(标题, 描述, 画像档, 期望命中规则)。ADV=对抗样本。
pub(super) fn corpus(
) -> Vec<(&'static str, String, &'static str, Vec<&'static str>)> {
    let good = "good";
    let newbie = "new";
    let with = |extra: &str| format!("{LONG}{extra}");
    vec![
        // ---- H1 乱码/夹码（8）----
        ("资源\u{FFFD}\u{FFFD}\u{FFFD}", LONG.into(), good, vec!["arule_title_garbled"]),
        ("资源\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}\u{FFFD}", LONG.into(), good, vec!["arule_title_garbled"]),
        ("aBcDeFgHiJkLmNoPqRsTv 标题", LONG.into(), good, vec!["arule_title_garbled"]),
        ("012345678901234567890123 标题", LONG.into(), good, vec!["arule_title_garbled"]),
        ("\u{E000}\u{E001}\u{E002} 私有区", LONG.into(), good, vec!["arule_title_garbled"]), // ADV 换区绕替换符
        ("正常标题.2023.WEB-DL.1080p-x", LONG.into(), good, vec![]),
        ("正常标题.2023.WEB-DL.1080p-GROUP", LONG.into(), good, vec![]),
        ("进击的巨人 第01-12话", LONG.into(), good, vec![]),
        // ---- H2 描述（6）----
        ("正常.2023.WEB", String::new(), good, vec!["arule_descr_missing"]),
        ("正常.2023.WEB", "<img src=x><br>".into(), good, vec!["arule_descr_missing"]),
        ("正常.2023.WEB", "短".into(), good, vec!["arule_descr_missing"]),
        ("正常.2023.WEB", LONG.into(), good, vec![]),
        ("正常.2023.WEB", "<b>加粗</b>正文".into(), good, vec!["arule_descr_missing"]),
        ("正常.2023.WEB", "a&nbsp;b&nbsp;c&nbsp;d&nbsp;e&nbsp;f&nbsp;g&nbsp;h&nbsp;i&nbsp;j&nbsp;k".into(), good, vec!["arule_descr_missing"]), // ADV nbsp 充字数
        // ---- H3 广告（8）----
        ("正常.2023.WEB", with(" 加微信群"), good, vec!["arule_ads_keywords"]),
        ("正常.2023.WEB", with(" T.ME/abc"), good, vec!["arule_ads_keywords"]), // ADV 大小写混淆
        ("正常.2023.WEB", with(" 扫码进群"), good, vec![]),
        ("正常.2023.WEB", with(" 夸克网盘链接"), good, vec![]),
        ("正常.2023.WEB", with(" 代购优惠"), good, vec![]),
        ("正常.2023.WEB", with(" 兼职刷单"), good, vec![]),
        ("微信群里看片 正常.2023.WEB", LONG.into(), good, vec!["arule_ads_keywords"]), // 标题命中
        ("正常.2023.WEB", LONG.into(), good, vec![]),
        // ---- H4 泄露（4）----
        ("正常.2023.WEB", with(&format!(" {}", "a".repeat(40))), good, vec!["arule_leak_marker"]),
        ("正常.2023.WEB", with(" passkey=1234"), good, vec!["arule_leak_marker"]),
        ("正常.2023.WEB", with(" /announce?passkey"), good, vec!["arule_leak_marker"]),
        ("正常.2023.WEB", with(&format!(" {}", "f".repeat(64))), good, vec!["arule_leak_marker"]), // ADV 64hex 含 40hex 连跑
        // ---- H5 干净多样性（H5 窗口在 assess 外注入，此处空窗=不触发）（4）----
        ("流浪地球2.2023.WEB-DL.2160p.H265-GROUP", LONG.into(), good, vec![]),
        ("完全不同的纪录片.2019.DVDRip", LONG.into(), good, vec![]),
        ("Music Album 2023 FLAC", LONG.into(), good, vec![]),
        ("综艺合集.2024.第10期", LONG.into(), good, vec![]),
        // ---- H6 命名（6）----
        ("流浪地球2.2023.WEB-DL.2160p.H265-GROUP", LONG.into(), good, vec![]),
        ("流浪地球2", LONG.into(), good, vec!["arule_title_format"]),
        ("流浪地球2 WEB-DL", LONG.into(), good, vec!["arule_title_format"]),
        ("超长超长超长超长超长超长超长超长超长超长超长超长超长超长标题.2024", LONG.into(), good, vec!["arule_title_format"]),
        ("电影名字够长.2023.BluRay.1080p.x264-GRP", LONG.into(), good, vec![]),
        ("剧名完整.2024.S01.1080p", LONG.into(), good, vec!["arule_title_format"]),
        // ---- 画像分档（6）----
        ("流浪地球2.2023.WEB-DL.2160p.H265-GROUP", LONG.into(), newbie, vec![]),
        ("正常.2023.WEB", LONG.into(), newbie, vec![]),
        ("正常.2023.WEB", "短".into(), newbie, vec!["arule_descr_missing"]),
        ("正常.2023.WEB", with(" 加微信群"), newbie, vec!["arule_ads_keywords"]),
        ("正常.2023.WEB", String::new(), newbie, vec!["arule_descr_missing"]),
        ("正常.2023.WEB", with(" 扫码"), newbie, vec![]),
        // ---- D 文件层（文件注入由专项测试覆盖，此处主链不误触）（6）----
        ("剧.2024.1080p", LONG.into(), good, vec![]),
        ("音乐专辑.2023.FLC", LONG.into(), good, vec![]),
        ("书籍.2022.PDF", LONG.into(), good, vec![]),
        ("动画.2024.第01话", LONG.into(), good, vec![]),
        ("比赛.2024.HDTV", LONG.into(), good, vec![]),
        ("课程.2023.1080p", LONG.into(), good, vec![]),
        // ---- 混合命中（4）----
        ("资源\u{FFFD}", with(" 加微信群"), good, vec!["arule_title_garbled", "arule_ads_keywords"]),
        ("资源\u{FFFD}", "短".into(), good, vec!["arule_title_garbled", "arule_descr_missing"]),
        ("正常.2023.WEB", format!("短 {}", "a".repeat(40)), good, vec!["arule_descr_missing", "arule_leak_marker"]),
        ("资源", with(" 夸克网盘 T.ME/x"), good, vec!["arule_ads_keywords", "arule_title_format"]),
        // ---- 干净多样本（8）----
        ("音乐人 - 专辑名 (2020) [FLAC]", LONG.into(), good, vec![]),
        ("联赛 2024 主队 vs 客队 2024-05-01 HDTV", LONG.into(), good, vec![]),
        ("动画名 第01-24话 [字幕组]", LONG.into(), good, vec![]),
        ("书名 (作者) [PDF]", LONG.into(), good, vec![]),
        ("纪录片名.2021.1080p.WEB-DL.H264-GRP", LONG.into(), good, vec![]),
        ("电影.2022.2160p.BluRay.x265-TEAM", LONG.into(), good, vec![]),
        ("电视剧.S01E01-E12.2023.1080p.WEB-DL.H264", LONG.into(), good, vec![]),
        ("课程名.第1讲.1080p", LONG.into(), good, vec![]),
    ]
}

/// 跑全表：期望每条必命中；期望为空的样本不得有 deny 档命中。
pub(super) fn run_corpus(cfg: &EngineConfig) -> usize {
    let all = corpus();
    assert!(all.len() >= 60, "回归集须 ≥60 例（现 {}）", all.len());
    for (i, (title, descr, profile_key, expect)) in all.iter().enumerate() {
        let p = if *profile_key == "good" {
            PublisherProfile {
                publish_credits: 5,
                approve_streak: 6,
                registered_days: 400,
                approved_90d: 20,
                denied_90d: 0,
            }
        } else {
            PublisherProfile::default()
        };
        let snap = TorrentSnapshot {
            torrent_id: 1,
            owner_id: 10,
            title: title.to_string(),
            small_descr: "副标题".into(),
            descr: descr.clone(),
            category_id: 1,
            site_type: "general".into(),
            file_names: vec![],
            recent_titles: vec![],
        };
        let a = assess(&snap, &p, cfg);
        let hit_rules: Vec<&str> = a.hits.iter().map(|h| h.rule).collect();
        for e in expect {
            assert!(
                hit_rules.contains(e),
                "例 #{i}「{title}」应命中 {e}，实得 {hit_rules:?}"
            );
        }
        if expect.is_empty() {
            assert!(
                a.hits.iter().all(|h| h.action != Action::Deny),
                "例 #{i}「{title}」干净样本不得有 deny 命中：{:?}",
                a.hits.iter().map(|h| h.rule).collect::<Vec<_>>()
            );
        }
    }
    all.len()
}
