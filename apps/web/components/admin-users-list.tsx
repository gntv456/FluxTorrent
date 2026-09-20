"use client";

import { fmtBytes } from "@/components/admin-users-detail";

/** 用户列表与批量操作面板（从 admin-users.tsx 按域拆出，300 门禁）：
 *  用户表格（多选/排序）/ 批量操作条 / 分页条。数据与动作留在 admin-users.tsx。 */

interface AdminUserRow {
  id: number;
  username: string;
  email: string;
  class_id: number;
  class_name: string | null;
  uploaded: number;
  downloaded: number;
  status: number;
  download_enabled: boolean;
  suspended: boolean;
  created_at: string;
  last_seen_at: string | null;
}

const STATUS_LABELS = ["正常", "禁言", "封禁"];

export function UsersBatchBar({
  sel,
  busy,
  batch,
  classes,
  batchClass,
  setBatchClass,
}: {
  sel: Set<number>;
  busy: boolean;
  batch: (action: "status" | "class", value: number) => Promise<void>;
  classes: [number, string][];
  batchClass: string;
  setBatchClass: React.Dispatch<React.SetStateAction<string>>;
}) {
  return (
    <section className="baozi-panel flex flex-wrap items-end gap-2 p-3">
      <p className="w-full text-xs font-bold text-sub">批量操作（已选 {sel.size} 个；只能操作等级低于自己的用户）</p>
      <button disabled={busy || sel.size === 0} onClick={() => batch("status", 0)} className="min-h-[36px] rounded-full bg-mint px-4 text-xs font-bold text-white disabled:opacity-50">批量恢复正常</button>
      <button disabled={busy || sel.size === 0} onClick={() => batch("status", 1)} className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold disabled:opacity-50">批量禁言</button>
      <button disabled={busy || sel.size === 0} onClick={() => batch("status", 2)} className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-danger disabled:opacity-50">批量封禁</button>
      <label className="flex flex-col gap-1 text-xs">
        批量改等级
        <select value={batchClass} onChange={(e) => setBatchClass(e.target.value)} className="min-h-[36px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
          <option value="">（选择等级）</option>
          {classes.filter(([id]) => id > 0 && id < 99).map(([id, label]) => (
            <option key={id} value={id}>{id} {label}</option>
          ))}
        </select>
      </label>
      <button disabled={busy || sel.size === 0 || !batchClass} onClick={() => batch("class", Number(batchClass))} className="min-h-[36px] rounded-full bg-sky px-4 text-xs font-bold text-white disabled:opacity-50">执行</button>
    </section>
  );
}

/** 列表页数据信封（与 admin-users.tsx 的 UsersPage 同构） */
interface UsersPageData {
  rows: AdminUserRow[];
  total: number;
  page: number;
  per_page: number;
}

export function UsersTable({
  data,
  sel,
  setSel,
  sortBtn,
}: {
  data: UsersPageData | null;
  sel: Set<number>;
  setSel: React.Dispatch<React.SetStateAction<Set<number>>>;
  sortBtn: (key: string, label: string) => React.ReactNode;
}) {
  return (
    <table className="nexus-table">
      <thead>
        <tr>
          <td className="colhead w-10">
            <input
              type="checkbox"
              checked={(data?.rows.length ?? 0) > 0 && data!.rows.every((u) => sel.has(u.id))}
              onChange={(e) => setSel(new Set(e.target.checked ? data!.rows.map((u) => u.id) : []))}
            />
          </td>
          <td className="colhead">{sortBtn("id", "Id")}</td>
          <td className="colhead">用户名</td>
          <td className="colhead">邮箱</td>
          <td className="colhead">{sortBtn("class", "等级")}</td>
          <td className="colhead">{sortBtn("uploaded", "上传量")}</td>
          <td className="colhead">{sortBtn("downloaded", "下载量")}</td>
          <td className="colhead">状态</td>
          <td className="colhead">下载权限</td>
          <td className="colhead">挂起</td>
          <td className="colhead">{sortBtn("created", "添加时间")}</td>
          <td className="colhead">操作</td>
        </tr>
      </thead>
      <tbody>
        {data?.rows.map((u) => (
          <tr key={u.id}>
            <td>
              <input
                type="checkbox"
                checked={sel.has(u.id)}
                onChange={(e) => setSel((prev) => { const n = new Set(prev); if (e.target.checked) n.add(u.id); else n.delete(u.id); return n; })}
              />
            </td>
            <td>{u.id}</td>
            <td>
              <a className="font-bold text-link" href={`/admin/users/${u.id}`}>
                {u.username}
              </a>
              {u.status > 0 && <span className="ml-1 rounded-full bg-coral/20 px-2 py-0.5 text-[10px] text-danger">{STATUS_LABELS[u.status] ?? u.status}</span>}
            </td>
            <td className="text-xs text-sub">{u.email}</td>
            <td>{u.class_name ?? `LV${u.class_id}`}</td>
            <td>{fmtBytes(u.uploaded)}</td>
            <td>{fmtBytes(u.downloaded)}</td>
            <td>{u.status >= 2 ? "封禁" : u.status === 1 ? "禁言" : "正常"}</td>
            <td>{u.download_enabled ? "yes" : "no"}</td>
            <td>{u.suspended ? "yes" : "no"}</td>
            <td className="text-xs">{new Date(u.created_at).toLocaleDateString()}</td>
            <td>
              <a className="cmgmt-act" href={`/admin/users/${u.id}`}>详情</a>
            </td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}
