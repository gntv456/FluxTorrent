"use client";

/**
 * 后台用户详情·管理操作面板（从 components/admin-user-detail.tsx 按域拆出）：
 * AdminActions 等级修改 / 分配角色 / 分配权限 / 授予勋章 / 授予道具 /
 * 分配考核 / 管理员改名 / 数值调整表单。状态留在 admin-user-detail.tsx，
 * 经 props 全量注入（受控展示，动作回调上抛）。
 */

import type { Detail } from "./admin-user-detail-shared";
import {
  CATEGORY_LABEL,
  ITEM_KIND_LABEL,
} from "./admin-user-detail-shared";

export interface AdminActionsProps {
  d: Detail;
  busy: boolean;
  currency: string;
  classList: [number, string][];
  dict: {
    adminrename: Record<string, string>;
  };
  panel: "" | "class" | "role" | "perm" | "medal" | "item" | "jixiao";
  setPanel: (p: "" | "class" | "role" | "perm" | "medal" | "item" | "jixiao") => void;
  adjust: boolean;
  setAdjust: (v: boolean) => void;
  adj: { up: string; down: string; spark: string; invite: string; note: string };
  setAdj: (v: { up: string; down: string; spark: string; invite: string; note: string }) => void;
  classId: string;
  setClassId: (v: string) => void;
  roles: { key: string; name: string }[];
  roleKey: string;
  setRoleKey: (v: string) => void;
  roleExp: string;
  setRoleExp: (v: string) => void;
  newRole: { key: string; name: string; descr: string };
  setNewRole: (v: { key: string; name: string; descr: string }) => void;
  perms: {
    key: string;
    name: string | null;
    category: string | null;
    descr: string | null;
  }[];
  permKey: string;
  setPermKey: (v: string) => void;
  permGrant: "1" | "0" | "";
  setPermGrant: (v: "1" | "0" | "") => void;
  permData: {
    effective: string[];
    overrides: { permission_key: string; granted: boolean }[];
  } | null;
  medals: { id: number; name: string }[];
  medalId: string;
  setMedalId: (v: string) => void;
  items: { id: number; name: string; kind: string }[];
  itemId: string;
  setItemId: (v: string) => void;
  jixiaoTypes: { id: number; name: string }[];
  jixiaoTypeId: string;
  setJixiaoTypeId: (v: string) => void;
  jixiaoPeriod: string;
  setJixiaoPeriod: (v: string) => void;
  renameOpen: boolean;
  setRenameOpen: (v: boolean) => void;
  newName: string;
  setNewName: (v: string) => void;
  onSubmitAdjust: () => void;
  onSubmitClass: () => void;
  onSubmitRole: (grant: boolean) => void;
  onSubmitPerm: () => void;
  onSubmitMedal: () => void;
  onSubmitItem: () => void;
  onSubmitJixiao: () => void;
  onSubmitNewRole: () => void;
  onSubmitRename: () => void;
  onResetPass: () => void;
  onToggle: (flag: "download_enabled" | "suspended") => void;
  onChangeStatus: (next: number) => void;
  onDeleteUser: () => void;
}

export function AdminActions(props: AdminActionsProps) {
  const { d, busy, currency, classList, dict, panel, setPanel, adjust, setAdjust } = props;
  return (
    <section className="baozi-panel flex flex-col gap-3 p-4">
      <h2 className="text-base font-bold text-ink">管理操作</h2>
      <div className="flex flex-wrap gap-2">
        <button className="baozi-button" onClick={() => { setAdjust(!adjust); setPanel(""); }}>修改上传量等</button>
        <button className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${panel === "class" ? "bg-sky text-white" : "border border-line"}`} onClick={() => { setPanel(panel === "class" ? "" : "class"); setAdjust(false); }}>等级修改</button>
        <button className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${panel === "role" ? "bg-sky text-white" : "border border-line"}`} onClick={() => { setPanel(panel === "role" ? "" : "role"); setAdjust(false); }}>分配角色</button>
        <button className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${panel === "perm" ? "bg-sky text-white" : "border border-line"}`} onClick={() => { setPanel(panel === "perm" ? "" : "perm"); setAdjust(false); }}>分配权限</button>
        <button className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${panel === "medal" ? "bg-sky text-white" : "border border-line"}`} onClick={() => { setPanel(panel === "medal" ? "" : "medal"); setAdjust(false); }}>授予勋章</button>
        <button className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${panel === "item" ? "bg-sky text-white" : "border border-line"}`} onClick={() => { setPanel(panel === "item" ? "" : "item"); setAdjust(false); }}>授予道具</button>
        <button className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${panel === "jixiao" ? "bg-sky text-white" : "border border-line"}`} onClick={() => { setPanel(panel === "jixiao" ? "" : "jixiao"); setAdjust(false); }}>分配考核</button>
        <button className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${props.renameOpen ? "bg-sky text-white" : "border border-line"}`} onClick={() => { props.setRenameOpen(!props.renameOpen); setPanel(""); setAdjust(false); }}>{dict.adminrename.btn}</button>
        <button disabled={busy} onClick={props.onResetPass} className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold">重置密码</button>
        <button disabled={busy} onClick={() => props.onToggle("download_enabled")}
          className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${d.download_enabled ? "border border-line text-danger" : "bg-mint text-white"}`}>
          {d.download_enabled ? "禁用下载权限" : "恢复下载权限"}
        </button>
        <button disabled={busy} onClick={() => props.onToggle("suspended")}
          className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${d.suspended ? "bg-mint text-white" : "border border-line text-danger"}`}>
          {d.suspended ? "解除挂起" : "挂起账号"}
        </button>
        {d.status === 1 && <button disabled={busy} onClick={() => props.onChangeStatus(0)} className="min-h-[36px] rounded-full bg-mint px-4 text-xs font-bold text-white">解除禁言</button>}
        {d.status === 2 && <button disabled={busy} onClick={() => props.onChangeStatus(0)} className="min-h-[36px] rounded-full bg-mint px-4 text-xs font-bold text-white">解除封禁</button>}
        {d.status === 0 && <button disabled={busy} onClick={() => props.onChangeStatus(1)} className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-danger">禁言</button>}
        {d.status < 2 && <button disabled={busy} onClick={() => props.onChangeStatus(2)} className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-danger">封禁</button>}
        {d.status >= 2 && <button disabled={busy} onClick={props.onDeleteUser} className="min-h-[36px] rounded-full bg-coral px-4 text-xs font-bold text-white">删除用户</button>}
      </div>

      {/* 等级修改 */}
      {panel === "class" && (
        <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
          <p className="mb-2 text-xs text-sub">当前 {d.class_name ?? `LV${d.class_id}`}；仅站长可改，且不可设为站长。</p>
          <div className="flex flex-wrap items-center gap-2">
            <select value={props.classId} onChange={(e) => props.setClassId(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
              <option value="">选择新等级</option>
              {classList.map(([id, label]) => (id < 99 ? <option key={id} value={id}>{id} {label}</option> : null))}
            </select>
            <button className="baozi-button" disabled={busy || !props.classId} onClick={props.onSubmitClass}>提交</button>
          </div>
        </div>
      )}

      {/* 分配角色 */}
      {panel === "role" && (
        <div className="cmgmt-form flex flex-col gap-3 rounded-[var(--r-md)] border border-line p-3">
          <p className="text-xs text-sub">职务可兼任；到期自动失效（可选）。</p>
          <div className="flex flex-wrap items-center gap-2">
            <select value={props.roleKey} onChange={(e) => props.setRoleKey(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
              <option value="">选择职务</option>
              {props.roles.map((r) => <option key={r.key} value={r.key}>{r.name}</option>)}
            </select>
            <input type="datetime-local" value={props.roleExp} onChange={(e) => props.setRoleExp(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" title="到期时间（可选）" />
            <button className="baozi-button" disabled={busy || !props.roleKey} onClick={() => props.onSubmitRole(true)}>分配</button>
            <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-danger" disabled={busy || !props.roleKey} onClick={() => props.onSubmitRole(false)}>收回</button>
          </div>
          {/* 新增职务（sysop）：分配前发现缺角色可直接建 */}
          <details className="rounded-[var(--r-sm)] border border-dashed border-line p-2">
            <summary className="cursor-pointer text-xs font-bold text-sub">＋ 新增职务（需要新角色时在此创建）</summary>
            <div className="mt-2 flex flex-wrap items-end gap-2">
              <label className="flex flex-col gap-1 text-xs">Key（小写/下划线）
                <input value={props.newRole.key} onChange={(e) => props.setNewRole({ ...props.newRole, key: e.target.value })} placeholder="translator" className="min-h-[40px] w-40 rounded-[var(--r-sm)] border border-line px-2" /></label>
              <label className="flex flex-col gap-1 text-xs">名称
                <input value={props.newRole.name} onChange={(e) => props.setNewRole({ ...props.newRole, name: e.target.value })} placeholder="翻译员" className="min-h-[40px] w-32 rounded-[var(--r-sm)] border border-line px-2" /></label>
              <label className="flex flex-col gap-1 text-xs">说明（可选）
                <input value={props.newRole.descr} onChange={(e) => props.setNewRole({ ...props.newRole, descr: e.target.value })} className="min-h-[40px] w-48 rounded-[var(--r-sm)] border border-line px-2" /></label>
              <button className="baozi-button" disabled={busy || !props.newRole.key.trim() || !props.newRole.name.trim()} onClick={props.onSubmitNewRole}>创建</button>
            </div>
          </details>
        </div>
      )}

      {/* 分配权限 */}
      {panel === "perm" && (
        <div className="cmgmt-form flex flex-col gap-2 rounded-[var(--r-md)] border border-line p-3">
          <p className="text-xs text-sub">覆盖角色判定：授予 = 额外允许；拒绝 = 显式禁止。</p>
          {props.permData && (
            <p className="text-xs text-sub">
              生效权限 {props.permData.effective.length} 项
              {props.permData.overrides.length > 0 && `（含 ${props.permData.overrides.length} 项个人覆盖）`}
            </p>
          )}
          <div className="flex flex-wrap items-center gap-2">
            <select value={props.permKey} onChange={(e) => props.setPermKey(e.target.value)} className="min-h-[40px] max-w-96 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
              <option value="">选择权限</option>
              {Object.entries(
                props.perms.reduce<Record<string, typeof props.perms>>((acc, p) => {
                  (acc[p.category ?? "其他"] ??= []).push(p);
                  return acc;
                }, {}),
              ).map(([cat, list]) => (
                <optgroup key={cat} label={CATEGORY_LABEL[cat] ?? cat}>
                  {list.map((p) => (
                    <option key={p.key} value={p.key}>
                      {p.name ?? p.key}{p.descr ? ` — ${p.descr}` : ""}
                    </option>
                  ))}
                </optgroup>
              ))}
            </select>
            <select value={props.permGrant} onChange={(e) => props.setPermGrant(e.target.value as "1" | "0" | "")} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
              <option value="">授予/拒绝</option>
              <option value="1">授予（额外允许）</option>
              <option value="0">拒绝（显式禁止）</option>
            </select>
            <button className="baozi-button" disabled={busy || !props.permKey || !props.permGrant} onClick={props.onSubmitPerm}>提交</button>
          </div>
        </div>
      )}

      {/* 授予勋章 */}
      {panel === "medal" && (
        <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
          <p className="mb-2 text-xs text-sub">管理发放（source=admin），已拥有 {d.medals} 枚。</p>
          <div className="flex flex-wrap items-center gap-2">
            <select value={props.medalId} onChange={(e) => props.setMedalId(e.target.value)} className="min-h-[40px] max-w-72 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
              <option value="">选择勋章</option>
              {props.medals.map((m) => <option key={m.id} value={m.id}>#{m.id} {m.name}</option>)}
            </select>
            <button className="baozi-button" disabled={busy || !props.medalId} onClick={props.onSubmitMedal}>授予</button>
          </div>
        </div>
      )}

      {/* 授予道具（含卡牌/装饰类） */}
      {panel === "item" && (
        <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
          <p className="mb-2 text-xs text-sub">免费发放：上传量/{currency}/邀请即时生效；化妆卡、改名卡等卡牌道具入背包待用户使用。</p>
          <div className="flex flex-wrap items-center gap-2">
            <select value={props.itemId} onChange={(e) => props.setItemId(e.target.value)} className="min-h-[40px] max-w-80 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
              <option value="">选择道具</option>
              {Object.entries(
                props.items.reduce<Record<string, typeof props.items>>((acc, it) => {
                  (acc[(ITEM_KIND_LABEL[it.kind] ?? it.kind).replaceAll("CURRENCY", currency)] ??= []).push(it);
                  return acc;
                }, {}),
              ).map(([kind, list]) => (
                <optgroup key={kind} label={kind}>
                  {list.map((it) => <option key={it.id} value={it.id}>#{it.id} {it.name}</option>)}
                </optgroup>
              ))}
            </select>
            <button className="baozi-button" disabled={busy || !props.itemId} onClick={props.onSubmitItem}>发放</button>
          </div>
        </div>
      )}

      {/* 管理员改名（P2-6b） */}
      {props.renameOpen && (
        <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
          <p className="mb-2 text-xs text-sub">{dict.adminrename.note}</p>
          <div className="flex flex-wrap items-center gap-2">
            <input
              value={props.newName}
              onChange={(e) => props.setNewName(e.target.value)}
              maxLength={32}
              placeholder={`${d.username} → ?`}
              aria-label={dict.adminrename.new_name}
              className="min-h-[40px] w-48 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-3 text-sm"
            />
            <button className="baozi-button" disabled={busy || !props.newName.trim()} onClick={props.onSubmitRename}>
              {dict.adminrename.submit}
            </button>
          </div>
        </div>
      )}

      {/* 分配考核 */}
      {panel === "jixiao" && (
        <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
          <p className="mb-2 text-xs text-sub">登记为考核岗位后按系统流水自动核算绩效；期间留空默认当月。</p>
          <div className="flex flex-wrap items-center gap-2">
            <select value={props.jixiaoTypeId} onChange={(e) => props.setJixiaoTypeId(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
              <option value="">选择考核岗位</option>
              {props.jixiaoTypes.map((j) => <option key={j.id} value={j.id}>#{j.id} {j.name}</option>)}
            </select>
            <input type="month" value={props.jixiaoPeriod} onChange={(e) => props.setJixiaoPeriod(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" title="考核期间（默认当月）" />
            <button className="baozi-button" disabled={busy || !props.jixiaoTypeId} onClick={props.onSubmitJixiao}>登记</button>
          </div>
        </div>
      )}
      {adjust && (
        <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
          <p className="mb-2 text-xs text-sub">正数增加、负数减少（下限 0）；邀请正数增发、负数回收。</p>
          <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
            <label className="flex flex-col gap-1 text-xs">上传量增量（字节）
              <input type="number" value={props.adj.up} onChange={(e) => props.setAdj({ ...props.adj, up: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" /></label>
            <label className="flex flex-col gap-1 text-xs">下载量增量（字节）
              <input type="number" value={props.adj.down} onChange={(e) => props.setAdj({ ...props.adj, down: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" /></label>
            <label className="flex flex-col gap-1 text-xs">{currency}增量
              <input type="number" value={props.adj.spark} onChange={(e) => props.setAdj({ ...props.adj, spark: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" /></label>
            <label className="flex flex-col gap-1 text-xs">邀请增发/回收
              <input type="number" value={props.adj.invite} onChange={(e) => props.setAdj({ ...props.adj, invite: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" /></label>
          </div>
          <label className="mt-2 flex flex-col gap-1 text-xs">备注（入审计）
            <input value={props.adj.note} onChange={(e) => props.setAdj({ ...props.adj, note: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" /></label>
          <div className="mt-2 flex gap-2">
            <button className="baozi-button" disabled={busy} onClick={props.onSubmitAdjust}>提交调整</button>
            <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold" onClick={() => setAdjust(false)}>取消</button>
          </div>
        </div>
      )}
    </section>
  );
}
