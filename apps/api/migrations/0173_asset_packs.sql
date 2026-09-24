-- 0173: 生态商店 M1 增量 —— 素材包品类（策划案 §3.1 素材包）
--
-- kind=assets：medals / avatar_frames 两表的纯覆盖素材包。
-- 口径与 taxonomy/theme 一致（A1-A6）：
--   - 键白名单：payload.tables 只允许 medals / avatar_frames（越权表拒）；
--   - 引用保护：medals 被 user_medals 持有、frames 被用户佩戴的行不可删改关键键；
--   - 回滚 = 导入前快照重放（同一条纯覆盖路径）。

ALTER TABLE content_packs DROP CONSTRAINT IF EXISTS content_packs_kind_check;
ALTER TABLE content_packs ADD CONSTRAINT content_packs_kind_check
  CHECK (kind IN ('taxonomy', 'theme', 'rules', 'assets'));
