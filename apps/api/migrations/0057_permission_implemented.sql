-- 标注权限项的接入状态：避免管理界面给出「配了却不生效」的假承诺
--
-- 背景：部分权限项（发布员的价格设置、查看匿名、转载等）在当前代码里
--       尚无对应业务端点为生效点。管理界面需要如实区分，否则站长配完
--       发现没效果，会误判为权限系统故障。

ALTER TABLE permissions
  ADD COLUMN IF NOT EXISTS implemented boolean NOT NULL DEFAULT true;

-- 已接线（真实生效点）
UPDATE permissions SET implemented = true WHERE key IN (
  'torrent.upload',           -- POST /torrents 发种入口
  'torrent.approval.auto',    -- POST /torrents 发布即通过
  'invites.bonus',            -- POST /invites 配额加成
  'hr.exempt'                 -- worker hr_enforce 跳过 H&R 快照
);

-- 尚未接线（当前代码无对应业务端点）
UPDATE permissions SET implemented = false WHERE key IN (
  'torrent.set_price',        -- 发种时设置促销（业务未实现）
  'torrent.view_anonymous',   -- 浏览端点判定（未实现）
  'torrent.upload_special',   -- 分类级发布限制（未实现）
  'torrent.see_banned',       -- 浏览端点判定（未实现）
  'torrent.repost',           -- 转载功能未实现
  'seed.stats.view',          -- 保种统计端点未实现
  'announce.publish'          -- 公告发布端点未实现
);

COMMENT ON COLUMN permissions.implemented IS
  '是否为真实生效点：false 表示当前代码尚无对应业务端点，界面应标注「未接入」';
