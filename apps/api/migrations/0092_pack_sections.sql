-- 0092: 站型包携带质量维度种子（NP 自定义 Section 口径的站型化延伸）
--
-- 现状缺口：切换站型包只重建分类/品牌/模块开关，质量维度（section_kinds 的
-- 标签与 section_dict 的选项）仍是教育站默认（人教/部编/一年级/课件…），
-- 导致发布页「质量」与高级搜索「多维筛选」跟站型格格不入。
--
-- 方案：site_type_packs 增加 sections JSONB 列——
--   { "kinds": [{ "kind": "media", "label": "平台", "sort": 10 }, ...],
--     "dict":  { "media": ["PC", "主机", ...], ... } }
-- apply 端点在应用包时同步重建这些维度的标签与选项（未在包内定义的维度不动，
-- 站方自建维度不受影响）。清空维度（[]）可显式清空某维度。

ALTER TABLE site_type_packs ADD COLUMN IF NOT EXISTS sections JSONB;

-- 游戏站种子：标签本地化 + 常用选项
UPDATE site_type_packs SET sections = $$
{
  "kinds": [
    { "kind": "media",     "label": "平台",     "sort": 10 },
    { "kind": "grades",    "label": "游戏类型", "sort": 20 },
    { "kind": "editions",  "label": "版本",     "sort": 30 },
    { "kind": "team",      "label": "制作组",   "sort": 90 }
  ],
  "dict": {
    "media":    ["PC", "主机", "掌机", "手机", "街机", "模拟器"],
    "grades":   ["动作", "角色扮演", "策略", "射击", "竞速", "体育", "模拟经营", "休闲", "独立游戏"],
    "editions": ["官方中文", "汉化补丁", "免安装硬盘版", "数字版", "重制版"],
    "team":     ["官方", "汉化组", "破解组", "整合组"]
  }
}
$$::jsonb
WHERE code = 'game';

-- 影视站种子
UPDATE site_type_packs SET sections = $$
{
  "kinds": [
    { "kind": "media",     "label": "媒介",     "sort": 10 },
    { "kind": "codec",     "label": "编码",     "sort": 40 },
    { "kind": "standard",  "label": "规格",     "sort": 60 },
    { "kind": "source",    "label": "来源",     "sort": 70 },
    { "kind": "team",      "label": "制作组",   "sort": 90 }
  ],
  "dict": {
    "media":    ["电影", "剧集", "动画", "纪录片", "综艺"],
    "codec":    ["H.264/x264", "H.265/x265", "AV1", "VC-1", "Xvid"],
    "standard": ["1080p", "2160p/4K", "720p", "BD50/BD25", "REMUX"],
    "source":   ["Blu-ray", "Web-DL", "HDTV", "DVD", "BDRip"],
    "team":     ["官方", "字幕组", "压制组"]
  }
}
$$::jsonb
WHERE code = 'movie';

-- 音乐站种子
UPDATE site_type_packs SET sections = $$
{
  "kinds": [
    { "kind": "media",    "label": "格式",   "sort": 10 },
    { "kind": "standard", "label": "音质",   "sort": 60 },
    { "kind": "source",   "label": "来源",   "sort": 70 }
  ],
  "dict": {
    "media":    ["无损 FLAC", "无损 APE/WAV", "DSD/SACD", "MP3/AAC", "黑盒抓轨"],
    "standard": ["24bit/192kHz", "24bit/96kHz", "16bit/44.1kHz", "320kbps"],
    "source":   ["CD 抓轨", "黑胶转录", "流媒体", "网络发行"]
  }
}
$$::jsonb
WHERE code = 'music';

-- 电子书站种子
UPDATE site_type_packs SET sections = $$
{
  "kinds": [
    { "kind": "media",    "label": "格式",   "sort": 10 },
    { "kind": "source",   "label": "来源",   "sort": 70 }
  ],
  "dict": {
    "media":    ["PDF", "EPUB", "MOBI/AZW3", "扫描版", "文字版"],
    "source":   ["出版社正版", "个人扫描", "网络整理"]
  }
}
$$::jsonb
WHERE code = 'ebook';
