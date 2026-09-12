-- 修正 implemented 标记：upload_special / repost 尚无代码生效点（0 处 perm:: 调用），
-- 之前被误标为 true。管理界面应如实标注「未接入」，避免「配了不生效」的假承诺。
-- todo：upload_special 需先定义「特殊分类」概念；repost（转载）功能整体未实现。

UPDATE permissions SET implemented = false WHERE key IN (
  'torrent.upload_special',
  'torrent.repost'
);
