"use client";

import { useCallback, useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { dateLocale } from "@/i18n/config";
import { useI18n } from "@/i18n/client";
import { AdminActions } from "./admin-user-detail-actions";
import { useAdminUserActions } from "./admin-user-detail-actions-hook";
import {
  LoginsPanel,
  ProfilePanels,
  SeedingPanel,
  SparkPanel,
} from "./admin-user-detail-panels";
import {
  PER_PAGE,
  STATUS_LABELS,
  type Detail,
  type DetailTab,
  type LoginRow,
  type SeedRow,
  type SparkRow,
} from "./admin-user-detail-shared";

/** 后台用户详情（好学站 Filament UserResource 的 user-profile 口径）：
 *  字段全景 + 管理动作 + 关联数据 tab（火花流水 / 登录记录 / 做种列表）。
 *  独立路由取代管理页内的弹层，容纳更丰富的运营操作。
 *  管理操作面板拆至 ./admin-user-detail-actions.tsx；
 *  动作提交域拆至 ./admin-user-detail-actions-hook.ts；
 *  资料全景 / 关联 tab 拆至 ./admin-user-detail-panels.tsx；
 *  类型与常量拆至 ./admin-user-detail-shared.ts。 */
export function AdminUserDetailPage() {
  const params = useParams<{ id: string }>();
  const router = useRouter();
  const { locale, dict, currency } = useI18n();
  const classList = (dict.admin as unknown as { classList: [number, string][] }).classList;
  const uid = Number(params.id);
  const [d, setD] = useState<Detail | null>(null);
  const [tab, setTab] = useState<DetailTab>("profile");
  const [spark, setSpark] = useState<{ rows: SparkRow[]; total: number } | null>(null);
  const [sparkPage, setSparkPage] = useState(1);
  const [logins, setLogins] = useState<{ rows: LoginRow[]; total: number } | null>(null);
  const [loginsPage, setLoginsPage] = useState(1);
  const [seeds, setSeeds] = useState<SeedRow[] | null>(null);
  const [adjust, setAdjust] = useState(false);
  const [adj, setAdj] = useState({ up: "0", down: "0", spark: "0", invite: "0", note: "" });
  // NP 级管理操作面板
  const [panel, setPanel] = useState<"" | "class" | "role" | "perm" | "medal" | "item" | "jixiao">("");
  const [classId, setClassId] = useState("");
  const [roles, setRoles] = useState<{ key: string; name: string }[]>([]);
  const [roleKey, setRoleKey] = useState("");
  const [roleExp, setRoleExp] = useState("");
  const [newRole, setNewRole] = useState({ key: "", name: "", descr: "" });
  const [perms, setPerms] = useState<{ key: string; name: string | null; category: string | null; descr: string | null }[]>([]);
  const [permData, setPermData] = useState<{ effective: string[]; overrides: { permission_key: string; granted: boolean }[] } | null>(null);
  const [permKey, setPermKey] = useState("");
  const [permGrant, setPermGrant] = useState<"1" | "0" | "">("");
  const [medals, setMedals] = useState<{ id: number; name: string }[]>([]);
  const [medalId, setMedalId] = useState("");
  const [items, setItems] = useState<{ id: number; name: string; kind: string }[]>([]);
  const [itemId, setItemId] = useState("");
  const [jixiaoTypes, setJixiaoTypes] = useState<{ id: number; name: string }[]>([]);
  const [jixiaoTypeId, setJixiaoTypeId] = useState("");
  const [jixiaoPeriod, setJixiaoPeriod] = useState("");
  // 管理员改名（P2-6b）：POST /admin/users/{id}/rename {new_name}
  const [renameOpen, setRenameOpen] = useState(false);
  const [newName, setNewName] = useState("");

  const [msg, setMsg] = useState<string | null>(null);
  const load = useCallback(async () => {
    try { setD(await api.get<Detail>(`/api/v1/admin/users/${uid}`)); }
    catch (e) { setMsg(e instanceof ApiError ? e.message : "加载失败"); }
  }, [uid]);
  useEffect(() => { load(); }, [load]);

  const actions = useAdminUserActions({
    uid,
    d,
    values: {
      classId, roleKey, roleExp, newRole,
      permKey, permGrant, medalId, itemId,
      jixiaoTypeId, jixiaoPeriod, newName, adj,
    },
    load,
    setPanel,
    setAdjust,
    setRenameOpen,
    setNewName,
    setNewRole,
    setRoles,
  });

  // 打开各操作面板时按需拉取字典/现状（等级字典来自 i18n classList，无需请求）
  useEffect(() => {
    if (panel === "role" && roles.length === 0) {
      api.get<{ key: string; name: string }[]>("/api/v1/admin/roles").then(setRoles).catch(() => {});
    }
    if (panel === "perm") {
      api.get<{ permissions: { key: string; name: string | null; category: string | null; descr: string | null }[] }>("/api/v1/admin/permission-matrix")
        .then((m) => setPerms(m.permissions ?? []))
        .catch(() => {});
      api.get<{ effective: string[]; overrides: { permission_key: string; granted: boolean }[] }>(`/api/v1/admin/user-permissions?user_id=${uid}`)
        .then(setPermData)
        .catch(() => setPermData(null));
    }
    if (panel === "medal" && medals.length === 0) {
      api.get<{ id: number; name: string }[]>("/api/v1/medals").then(setMedals).catch(() => {});
    }
    if (panel === "item" && items.length === 0) {
      api.get<{ id: number; name: string; kind: string }[]>("/api/v1/shop/items").then(setItems).catch(() => {});
    }
    if (panel === "jixiao" && jixiaoTypes.length === 0) {
      api.get<{ id: number; name: string }[]>("/api/v1/jixiao/types").then(setJixiaoTypes).catch(() => {});
    }
  }, [panel, uid, roles.length, medals.length, items.length, jixiaoTypes.length]);

  useEffect(() => {
    if (tab === "spark") {
      api.get<{ rows: SparkRow[]; total: number }>(`/api/v1/admin/spark-logs?user_id=${uid}&page=${sparkPage}&per_page=${PER_PAGE}`)
        .then(setSpark).catch(() => setSpark(null));
    } else if (tab === "logins") {
      api.get<{ rows: LoginRow[]; total: number }>(`/api/v1/admin/login-logs?user_id=${uid}&page=${loginsPage}&per_page=${PER_PAGE}`)
        .then(setLogins).catch(() => setLogins(null));
    } else if (tab === "seeding" && seeds === null) {
      api.get<SeedRow[]>(`/api/v1/admin/users/${uid}/snatches`)
        .then(setSeeds).catch(() => setSeeds([]));
    }
  }, [tab, uid, sparkPage, loginsPage, seeds]);

  if (!d) return <p className="py-8 text-center text-sub">{msg ?? "加载中…"}</p>;

  const dt = (s: string | null) => (s ? new Date(s).toLocaleString(dateLocale(locale)) : "—");
  // 动作提示条优先展示 hook 的 flash 文案，加载兜底用本地 msg
  const toast = actions.msg ?? msg;

  return (
    <div className="flex flex-col gap-3">
      {toast && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{toast}</p>}

      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap items-baseline gap-2">
          <h1 className="font-display text-2xl">{d.username}</h1>
          <span className="text-sm text-sub">UID {d.id} · {d.class_name ?? `LV${d.class_id}`}</span>
          {d.title && <span className="rounded-full bg-sun/30 px-2 py-0.5 text-xs">{d.title}</span>}
          {d.status > 0 && <span className="rounded-full bg-coral/20 px-2 py-0.5 text-xs text-danger">{STATUS_LABELS[d.status] ?? d.status}</span>}
          {d.suspended && <span className="rounded-full bg-coral/20 px-2 py-0.5 text-xs text-danger">挂起</span>}
          {!d.download_enabled && <span className="rounded-full bg-coral/20 px-2 py-0.5 text-xs text-danger">禁下载</span>}
          {d.parked && <span className="rounded-full bg-sky-soft px-2 py-0.5 text-xs">泊车</span>}
          {d.donor && <span className="rounded-full bg-mint/30 px-2 py-0.5 text-xs">捐赠者</span>}
        </div>
        <button onClick={() => router.push("/admin?tool=users")} className="min-h-[36px] rounded-full border border-line px-4 text-xs">
          返回用户列表
        </button>
      </div>

      {/* 关联数据 tab */}
      <div className="flex flex-wrap gap-2" role="tablist">
        {([["profile", "资料全景"], ["spark", `${currency}流水`], ["logins", "登录记录"], ["seeding", "做种/下载"]] as const).map(([k, label]) => (
          <button key={k} role="tab" aria-selected={tab === k} onClick={() => setTab(k)}
            className={`min-h-[40px] rounded-full px-4 text-sm font-bold ${tab === k ? "bg-sky text-white" : "border border-line bg-[var(--surface-card)] text-sub"}`}>
            {label}
          </button>
        ))}
      </div>

      {tab === "profile" && (
        <>
          <ProfilePanels d={d} currency={currency} dt={dt} />
          {actions.tmpPass && (
            <p className="rounded-[var(--r-md)] bg-sky-soft p-3 font-mono text-sm text-ink">
              临时密码（仅显示一次）：{actions.tmpPass}，用户首登需改密。
            </p>
          )}
          <AdminActions
            d={d}
            busy={actions.busy}
            currency={currency}
            classList={classList}
            dict={{ adminrename: dict.adminrename as unknown as Record<string, string> }}
            panel={panel}
            setPanel={setPanel}
            adjust={adjust}
            setAdjust={setAdjust}
            adj={adj}
            setAdj={setAdj}
            classId={classId}
            setClassId={setClassId}
            roles={roles}
            roleKey={roleKey}
            setRoleKey={setRoleKey}
            roleExp={roleExp}
            setRoleExp={setRoleExp}
            newRole={newRole}
            setNewRole={setNewRole}
            perms={perms}
            permKey={permKey}
            setPermKey={setPermKey}
            permGrant={permGrant}
            setPermGrant={setPermGrant}
            permData={permData}
            medals={medals}
            medalId={medalId}
            setMedalId={setMedalId}
            items={items}
            itemId={itemId}
            setItemId={setItemId}
            jixiaoTypes={jixiaoTypes}
            jixiaoTypeId={jixiaoTypeId}
            setJixiaoTypeId={setJixiaoTypeId}
            jixiaoPeriod={jixiaoPeriod}
            setJixiaoPeriod={setJixiaoPeriod}
            renameOpen={renameOpen}
            setRenameOpen={setRenameOpen}
            newName={newName}
            setNewName={setNewName}
            onSubmitAdjust={actions.submitAdjust}
            onSubmitClass={actions.submitClass}
            onSubmitRole={actions.submitRole}
            onSubmitPerm={actions.submitPerm}
            onSubmitMedal={actions.submitMedal}
            onSubmitItem={actions.submitItem}
            onSubmitJixiao={actions.submitJixiao}
            onSubmitNewRole={actions.submitNewRole}
            onSubmitRename={actions.submitRename}
            onResetPass={actions.resetPass}
            onToggle={actions.toggle}
            onChangeStatus={actions.changeStatus}
            onDeleteUser={actions.deleteUser}
          />
        </>
      )}

      {tab === "spark" && (
        <SparkPanel spark={spark} page={sparkPage} setPage={setSparkPage} dt={dt} />
      )}

      {tab === "logins" && (
        <LoginsPanel logins={logins} page={loginsPage} setPage={setLoginsPage} dt={dt} />
      )}

      {tab === "seeding" && <SeedingPanel seeds={seeds} />}
    </div>
  );
}
