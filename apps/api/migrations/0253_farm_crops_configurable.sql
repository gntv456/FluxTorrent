-- 农场作物表变成「可配」（0253）。
--
-- 这是娱乐屋最后一个改不了参数的玩法表：作物名 / 种子价 / 基准产量 / 成熟时长
-- 全冻在 0011 的播种里，而 `games/farm.rs` 的文件注释一直写着「站长可在后台改名」。
-- 更要紧的是：0252 之后彩蛋池的定标单位就是 `min(farm_crops.seed_price)`，
-- 「产量 = 种子价 × 0.75」这条标定也就是 `FARM_BASE_EV = 0.90` 成立的前提 ——
-- 一张没人能改、也就能被随手改库的表，撑着两条经济口径。
--
-- 这一刀把口径落到结构上：
--   ① id 走序列（此前没有默认值，加一行得自己数 max(id)+1）；
--   ② CHECK 钉住正数、成熟时长区间、名字唯一，以及**标定本身**
--      （base_yield × 4 <= seed_price × 3，等价于产量 <= 75% 种子价、
--       含 20% 双倍后 EV <= 0.90）—— 越过它就不是「更慷慨的作物」，而是农场
--       开始增发，写侧闸与运行时都以此为前提；
--   ③ active 一列做「下架」：收获过一次的作物有 farm_harvests 外键挡着删不掉
--      （那是审计留痕，不该被绕过），站长要的其实是「别再让人买这一款」；
--   ④ farm_plots.crop_id 补外键 RESTRICT：没有它，删作物会把已种的地变成
--      INNER JOIN 里查不到的幽灵地块（既看不见也收不走）。当前孤儿地块为 0。

BEGIN;

CREATE SEQUENCE IF NOT EXISTS farm_crops_id_seq;
ALTER TABLE farm_crops
    ALTER COLUMN id SET DEFAULT nextval('farm_crops_id_seq');
SELECT setval('farm_crops_id_seq', (SELECT max(id) FROM farm_crops));

ALTER TABLE farm_crops DROP CONSTRAINT IF EXISTS farm_crops_seed_price_pos;
ALTER TABLE farm_crops ADD CONSTRAINT farm_crops_seed_price_pos
    CHECK (seed_price > 0);
ALTER TABLE farm_crops DROP CONSTRAINT IF EXISTS farm_crops_base_yield_pos;
ALTER TABLE farm_crops ADD CONSTRAINT farm_crops_base_yield_pos
    CHECK (base_yield > 0);
ALTER TABLE farm_crops DROP CONSTRAINT IF EXISTS farm_crops_grow_hours_range;
ALTER TABLE farm_crops ADD CONSTRAINT farm_crops_grow_hours_range
    CHECK (grow_hours BETWEEN 1 AND 720);
ALTER TABLE farm_crops DROP CONSTRAINT IF EXISTS farm_crops_calibration;
ALTER TABLE farm_crops ADD CONSTRAINT farm_crops_calibration
    CHECK (base_yield * 4 <= seed_price * 3);

ALTER TABLE farm_crops ADD COLUMN IF NOT EXISTS active boolean NOT NULL DEFAULT true;
CREATE UNIQUE INDEX IF NOT EXISTS farm_crops_name_uniq ON farm_crops (name);

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM farm_plots p
                WHERE NOT EXISTS (SELECT 1 FROM farm_crops c
                                   WHERE c.id = p.crop_id)) THEN
        RAISE EXCEPTION 'farm_plots 有指向不存在作物的地块，先手工核对再补外键';
    END IF;
END $$;

ALTER TABLE farm_plots DROP CONSTRAINT IF EXISTS farm_plots_crop_id_fkey;
ALTER TABLE farm_plots ADD CONSTRAINT farm_plots_crop_id_fkey
    FOREIGN KEY (crop_id) REFERENCES farm_crops (id) ON DELETE RESTRICT;

COMMIT;
