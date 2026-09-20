-- 0139 头像框·四季系列（avatar_frames 新增春/夏/秋/冬四款）
--
-- 背景：avatar_frames 现仅有 0030 播下的金色光环/蓝色流光/彩虹描边三款。
-- 本轮按需求补「春夏秋冬」四款季节限定框。
--
-- 约束（重要）：前端 FrameShop 的 frameStyle() 只放行 border-color / box-shadow
-- 两个属性（apps/web/components/plugins.tsx），CSS 里写其它声明会被静默丢弃，
-- 所以四款框全部用「border-color 主色 + 双层 box-shadow（内圈描边 + 外圈光晕）」表达：
--   春·樱语：樱粉柔光   夏·竹青：青绿冷光
--   秋·鎏金：暖金辉光   冬·霜蓝：冰蓝霜光
-- 幂等口径：0030 的种子用的是「表空才插」，本库已有数据会导致全表跳过，
-- 这里改为按 name 逐条判重（重放安全，也不会与旧三款撞名）。

INSERT INTO avatar_frames (name, css, price, sort)
SELECT * FROM (VALUES
  ('春·樱语', 'border-color:#f4a7bb; box-shadow: 0 0 0 2px rgba(244,167,187,.5), 0 0 12px rgba(244,167,187,.8);', 6000, 11),
  ('夏·竹青', 'border-color:#2fa878; box-shadow: 0 0 0 2px rgba(47,168,120,.5), 0 0 12px rgba(47,168,120,.85);', 6000, 12),
  ('秋·鎏金', 'border-color:#d99419; box-shadow: 0 0 0 2px rgba(217,148,25,.5), 0 0 12px rgba(217,148,25,.85);', 6000, 13),
  ('冬·霜蓝', 'border-color:#8fc3e8; box-shadow: 0 0 0 2px rgba(143,195,232,.6), 0 0 12px rgba(143,195,232,.9);', 6000, 14)
) AS seed(name, css, price, sort)
WHERE NOT EXISTS (SELECT 1 FROM avatar_frames WHERE avatar_frames.name = seed.name);
