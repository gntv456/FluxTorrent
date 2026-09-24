-- 0170: 审核可见性与免审等级（用户反馈 2026-09-24）
--
-- 背景：种子编辑会回退待审（NP takeedit 口径，approval_status=0），而默认列表
-- 谓词只显示 approval_status=1 —— 编辑过的种子在列表「消失」；审核中/失败的
-- 种子对普通作者完全不可见，无法自查进度。本迁移给站长三个站点设定：
--   1) list_show_pending    默认列表放行「审核中」种子（列表带状态徽标）
--   2) list_show_rejected   默认列表放行「审核失败」种子（列表带状态徽标）
--   3) upload_auto_approve_class 发布免审最低等级（缺省 92 = 论坛版主）：
--      class ≥ 阈值发布即通过；该等级作者编辑自己的种子也不再回退待审。
-- 另见 upload.rs 免审链（分类 auto_approve / 连续过审 ≥5 / 权限点
-- torrent.approval.auto）与 manage_perm.rs 阈值模式（torrent_edit_class 同款）。

INSERT INTO site_settings (name, value, descr, grp) VALUES
('list_show_pending', 'no', '种子列表显示「审核中」的种子', 'torrent'),
('list_show_rejected', 'no', '种子列表显示「审核失败」的种子', 'torrent'),
('upload_auto_approve_class', '92', '发布免审最低等级（≥该等级发布即通过；92=论坛版主）', 'torrent')
ON CONFLICT (name) DO NOTHING;

INSERT INTO settings_meta (name, type, label_zh, label_en, hint, group_key, card_order) VALUES
('list_show_pending', 'yesno', '列表显示审核中种子', 'Show pending in list',
 '开启后「审核中」种子出现在种子列表（带状态徽标）；关闭时仅作者与审核人员可见',
 'torrent', 70),
('list_show_rejected', 'yesno', '列表显示审核失败种子', 'Show rejected in list',
 '开启后「审核失败」种子出现在种子列表（带状态徽标）；关闭时仅作者与审核人员可见',
 'torrent', 71),
('upload_auto_approve_class', 'classlevel', '发布免审最低等级', 'Auto-approve min class',
 '等级 ≥ 该值的用户发布种子免审直接通过；该等级作者编辑自己的种子也不再回退待审',
 'torrent', 72)
ON CONFLICT (name) DO UPDATE
  SET type = EXCLUDED.type, label_zh = EXCLUDED.label_zh,
      label_en = EXCLUDED.label_en, hint = EXCLUDED.hint,
      group_key = EXCLUDED.group_key, card_order = EXCLUDED.card_order;
