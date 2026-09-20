-- 0145 登录页标语联动修复：物化默认值改为纯覆盖语义
--
-- 0144 的两处缺口（设置卡切换站型标语不联动）：
--   1) 0144 把站型包默认标语物化进 site_settings.site_tagline（「读书 · 学习 · 分享」
--      留存），而「站点类型」枚举在设置卡可直改 site_type（settings_groups_put，
--      audit 2647：education→general 正是此路径）——该路径不动物化值，登录页标语
--      永远停在旧站型。
--   2) 站型包切换向导（apply 端点）虽写 site_tagline，但站长此后在设置卡改标语
--      会被下一次 apply 无声覆盖。
--
-- 语义收敛（与 0090 site_desc 同款）：site_settings.site_tagline = 站长覆盖值，
-- 空 = 动态跟随当前站型包默认（site-profile 读 site_settings.site_type JOIN
-- site_type_packs.tagline，切换站型即刻生效）。
--
-- 本迁移只做一件事：清除物化的默认值（值等于任一站型包默认 → 清空），
-- 站长自定义值不动。

UPDATE site_settings s
SET value = '', updated_at = now()
WHERE s.name = 'site_tagline'
  AND EXISTS (SELECT 1 FROM site_type_packs p WHERE p.tagline = s.value);

-- 设置卡提示同步覆盖语义（0144 注册时说的是「留空使用当前站型的默认标语」，
-- 行为已一致，仅措辞收紧）
UPDATE settings_meta
SET hint = '登录页品牌区标语；留空自动跟随当前站型默认（切换站型即刻更新），填写则固定覆盖'
WHERE name = 'site_tagline';
