-- 0176: 生态商店 M2 增量 —— 内置素材精品目录（策划案 §3.1 素材包/§7.3 自营铺货）
--
-- builtin_assets 目录源：随核心发版的素材包种子（medals/avatar_frames 行集）。
-- 目录条目动态生成（pack_catalog 拼 CatalogItem），安装时按 code 现场拼包——
-- 不物化进 medals 表（0145：导入才落库，目录只是「可装清单」）。
-- 首发一套「四季·和风」头像框 + 「节气」勋章六件（正式化时由设计稿替换内容）。

CREATE TABLE IF NOT EXISTS builtin_assets (
    code        TEXT PRIMARY KEY,           -- 如 season-wafu
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    kind        TEXT NOT NULL DEFAULT 'assets',
    payload     JSONB NOT NULL              -- {tables: {medals: [...], avatar_frames: [...]}}
);

INSERT INTO builtin_assets (code, name, description, payload) VALUES
('season-wafu', '四季·和风头像框', '春夏秋冬四款和风描边头像框',
 $${"tables": {"avatar_frames": [
   {"id": 201, "name": "和风·樱", "css": ".frame-wafu-sakura{outline:2px solid #ffb7c5}", "price": 200, "sort": 201},
   {"id": 202, "name": "和风·波", "css": ".frame-wafu-nami{outline:2px solid #4aa3df}", "price": 200, "sort": 202},
   {"id": 203, "name": "和风·枫", "css": ".frame-wafu-momiji{outline:2px solid #e0662e}", "price": 200, "sort": 203},
   {"id": 204, "name": "和风·雪", "css": ".frame-wafu-yuki{outline:2px solid #cfe8ff}", "price": 200, "sort": 204}
 ]}}$$),
('solar-terms', '节气勋章六件', '立春/谷雨/芒种/白露/霜降/冬至（纪念发放型）',
 $${"tables": {"medals": [
   {"id": 9101, "name": "节气·立春", "get_type": 2, "rarity": "rare", "description": "内置素材包·节气系列", "price": null, "bonus_addition_factor": 0, "category_id": 0, "limited": false},
   {"id": 9102, "name": "节气·谷雨", "get_type": 2, "rarity": "rare", "description": "内置素材包·节气系列", "price": null, "bonus_addition_factor": 0, "category_id": 0, "limited": false},
   {"id": 9103, "name": "节气·芒种", "get_type": 2, "rarity": "rare", "description": "内置素材包·节气系列", "price": null, "bonus_addition_factor": 0, "category_id": 0, "limited": false},
   {"id": 9104, "name": "节气·白露", "get_type": 2, "rarity": "rare", "description": "内置素材包·节气系列", "price": null, "bonus_addition_factor": 0, "category_id": 0, "limited": false},
   {"id": 9105, "name": "节气·霜降", "get_type": 2, "rarity": "rare", "description": "内置素材包·节气系列", "price": null, "bonus_addition_factor": 0, "category_id": 0, "limited": false},
   {"id": 9106, "name": "节气·冬至", "get_type": 2, "rarity": "rare", "description": "内置素材包·节气系列", "price": null, "bonus_addition_factor": 0, "category_id": 0, "limited": false}
 ]}}$$)
ON CONFLICT (code) DO NOTHING;
