-- 0167: 生态商店 M1 —— 内容包登记表（策划案《生态商店与适配器体系》§3 / §8 M1）
--
-- 内容包（taxonomy / theme）本地导入与回滚的登记。M1 无中央仓库：包文件由站长
-- 在后台导入导出页粘贴/上传，登记表承担三件事：
--   1) 已装包清单（pack_id 唯一——同 id 再导入 = 升级，snapshot 滚动覆盖）；
--   2) 导入前快照（回滚 = 把快照当一次导入执行，A2「禁用即还原」）；
--   3) 审计锚点（content_pack:import / content_pack:rollback 写 audit_log）。
--
-- 0145 纪律重申：包只做「纯覆盖 + 动态合并」，不物化任何派生值——导航/列表/
-- 上传表单/搜索聚合仍从 categories / section_kinds / section_dict 动态派生，
-- 与 site_type 派生值同一套口径（排查先看 audit_log）。

CREATE TABLE IF NOT EXISTS content_packs (
  id           BIGSERIAL PRIMARY KEY,
  pack_id      TEXT NOT NULL,             -- 全局标识，如 taxonomy.movie.v1
  kind         TEXT NOT NULL CHECK (kind IN ('taxonomy', 'theme')),
  name         TEXT NOT NULL,
  version      TEXT NOT NULL,
  core_compat  TEXT NOT NULL DEFAULT '*', -- 商店侧兼容矩阵预留（M2 用）
  snapshot     JSONB NOT NULL,            -- 导入前的分类/维度/设置快照（回滚源）
  applied_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
  applied_by   BIGINT REFERENCES users(id),
  UNIQUE (pack_id)
);

-- 登记表自检：M1 只支持两类内容包；扩展品类（素材/汉化）时在此 CHECK 追加
