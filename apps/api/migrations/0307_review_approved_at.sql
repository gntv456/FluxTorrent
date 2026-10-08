-- 0307（审种链路深测，2026-10-08，报告 _doc/审种员视角深测审计-2026-10-08.md）
-- 审核链路的 approved_at 从不写入（P1-1）+ 被拒阈值配置键漂移（P1-2）两处静默失效的数据侧修复。
-- 代码侧同步修改：decide/batch/upload/revive/edit/resubmit/restore 全部对齐 approved_at 生命周期。
--
-- P1-1 背景：worker 泄露检测（settle.rs：过审 10 分钟内完成下载）与社交队小时窗
-- 都以 `approved_at IS NOT NULL` 为前提，而审核台的 UPDATE 从不写该字段——
-- 全库过审种该列恒 NULL（实测 60/60），两类检测对所有常规过审种从不触发。
-- offers 转正（唯一写点）只覆盖极少数官种。
--
-- 回填口径：过审时间取 created_at（近似值）。mtime 语义是「最后编辑」而非
-- 「最后过审」，且编辑回退待审后仍会残留新值，用它会把「过审后 10 分钟」
-- 的窗口整体后移，泄露检测更失真。created_at 是过审时间的下界，宁可早不可晚。

UPDATE torrents
   SET approved_at = created_at
 WHERE approval_status = 1
   AND approved_at IS NULL;

-- P1-2：设置页（0039）种的键是 upload_deny_approval_deny_count，代码旧读
-- upload_deny_limit（互不相认，站长改设置恒不生效）。代码已统一改读前者；
-- 旧键从未被任何 UI 暴露，若站长手工种过它，尊重其意图迁移过来后删除，
-- 避免两个键各说各话。

INSERT INTO site_settings (name, value)
SELECT 'upload_deny_approval_deny_count', s.value
  FROM site_settings s
 WHERE s.name = 'upload_deny_limit'
   AND NOT EXISTS (SELECT 1 FROM site_settings n
                    WHERE n.name = 'upload_deny_approval_deny_count')
ON CONFLICT (name) DO NOTHING;

DELETE FROM site_settings WHERE name = 'upload_deny_limit';
