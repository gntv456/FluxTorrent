-- 0176: 给仍未声明 sections 的站型包补上维度预置
--
-- 0092 只给 game/movie/music 铺了 sections，0175 补了 general，其余包仍是 NULL。
-- NULL 的语义是「本包不管质量维度」——新实例一旦选择这些包，后台就一个维度都没有，
-- 发布页「质量」与高级搜索「多维筛选」全是空的。这里按各站型实际用语补齐预置。
-- 只填 sections IS NULL 的包：站长改过的包、以及已预置过的包都不动。
-- 维度与选项只是初始值，后台可自由增删改（0176 之后内置维度已解除冻结）。

-- 动漫站
UPDATE site_type_packs SET sections = $$
{
  "kinds": [
    { "kind": "media",  "label": "媒介",   "sort": 10 },
    { "kind": "codec",  "label": "编码",   "sort": 40 },
    { "kind": "source", "label": "来源",   "sort": 70 },
    { "kind": "team",   "label": "制作组", "sort": 90 }
  ],
  "dict": {
    "media":  ["番剧", "OVA/OAD", "剧场版", "漫画", "画册", "音乐"],
    "codec":  ["x264", "x265", "AV1", "HEVC"],
    "source": ["BDRIP", "Web-DL", "HDTV", "DVDRIP", "自制"],
    "team":   []
  }
}
$$::jsonb
WHERE code = 'anime' AND sections IS NULL;

-- 纪录片站
UPDATE site_type_packs SET sections = $$
{
  "kinds": [
    { "kind": "media",  "label": "媒介",   "sort": 10 },
    { "kind": "codec",  "label": "编码",   "sort": 40 },
    { "kind": "source", "label": "来源",   "sort": 70 },
    { "kind": "team",   "label": "制作组", "sort": 90 }
  ],
  "dict": {
    "media":  ["纪录片", "短片", "影像资料", "图文"],
    "codec":  ["H.264", "H.265", "AV1", "ProRes"],
    "source": ["电视台", "流媒体", "院线", "自制", "出版"],
    "team":   []
  }
}
$$::jsonb
WHERE code = 'documentary' AND sections IS NULL;

-- 教育站：学段与版本在这一站型下才是通用维度，名称与 grades/editions 实体表逐字对齐
-- （详情页对历史种子仍按实体表 id 翻译，两套名字必须一致，否则同一行会显示成两种叫法）
UPDATE site_type_packs SET sections = $$
{
  "kinds": [
    { "kind": "media",    "label": "媒介", "sort": 10 },
    { "kind": "grades",   "label": "学段", "sort": 20 },
    { "kind": "editions", "label": "版本", "sort": 30 },
    { "kind": "source",   "label": "来源", "sort": 70 }
  ],
  "dict": {
    "media":    ["视频", "音频", "文档", "图片", "软件", "书籍"],
    "grades":   ["幼儿园", "一年级", "二年级", "三年级", "四年级", "五年级", "六年级",
                 "初一", "初二", "初三", "高一", "高二", "高三"],
    "editions": ["人教", "部编", "统编", "苏教", "北师大", "外研", "沪教"],
    "source":   ["自制", "转载", "采集"]
  }
}
$$::jsonb
WHERE code = 'education' AND sections IS NULL;

-- 无损音乐站
UPDATE site_type_packs SET sections = $$
{
  "kinds": [
    { "kind": "media",       "label": "媒介",     "sort": 10 },
    { "kind": "codec",       "label": "编码",     "sort": 40 },
    { "kind": "audio_codec", "label": "音频编码", "sort": 50 },
    { "kind": "source",      "label": "来源",     "sort": 70 },
    { "kind": "team",        "label": "制作组",   "sort": 90 }
  ],
  "dict": {
    "media":       ["专辑", "单曲/EP", "现场", "影像"],
    "codec":       ["Lossless", "24bit Lossless", "MP3", "AAC"],
    "audio_codec": ["FLAC", "APE", "WAV", "ALAC", "OGG"],
    "source":      ["CD抓轨", "SACD", "DVD-A", "黑胶转录", "数字下载"],
    "team":        []
  }
}
$$::jsonb
WHERE code = 'lossless' AND sections IS NULL;

-- 软件站
UPDATE site_type_packs SET sections = $$
{
  "kinds": [
    { "kind": "media",  "label": "媒介",   "sort": 10 },
    { "kind": "source", "label": "来源",   "sort": 70 },
    { "kind": "team",   "label": "制作组", "sort": 90 }
  ],
  "dict": {
    "media":  ["应用", "游戏", "插件", "驱动", "系统", "教程"],
    "source": ["官方", "绿色便携", "汉化", "修改版", "自制"],
    "team":   []
  }
}
$$::jsonb
WHERE code = 'software' AND sections IS NULL;

-- 体育站
UPDATE site_type_packs SET sections = $$
{
  "kinds": [
    { "kind": "media",  "label": "媒介",   "sort": 10 },
    { "kind": "codec",  "label": "编码",   "sort": 40 },
    { "kind": "source", "label": "来源",   "sort": 70 },
    { "kind": "team",   "label": "制作组", "sort": 90 }
  ],
  "dict": {
    "media":  ["全场", "集锦", "纪录片", "图文"],
    "codec":  ["H.264", "H.265", "AV1"],
    "source": ["电视台", "流媒体", "院线", "自制"],
    "team":   []
  }
}
$$::jsonb
WHERE code = 'sports' AND sections IS NULL;
