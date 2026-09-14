"use client";

import { useCallback, useEffect, useState } from "react";
import { useParams, useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { dateLocale } from "@/i18n/config";
import { useI18n } from "@/i18n/client";

/** 后台用户详情（好学站 Filament UserResource 的 user-profile 口径）：
 *  字段全景 + 管理动作 + 关联数据 tab（火花流水 / 登录记录 / 做种列表）。
 *  独立路由取代管理页内的弹层，容纳更丰富的运营操作。 */

interface Detail {
  id: number;
  username: string;
  email: string;
  passkey: string;
  class_id: number;
  class_name: string | null;
  title: string | null;
  uploaded: number;
  downloaded: number;
  spark_balance: number;
  status: number;
  download_enabled: boolean;
  suspended: boolean;
  parked: boolean;
  donor: boolean;
  totp_enabled: boolean;
  invited_by: number | null;
  inviter_name: string | null;
  created_at: string;
  last_seen_at: string | null;
  seeding: number;
  leeching: number;
  uploads: number;
  invites_unused: number;
  comments: number;
  downloaded_count: number;
  medals: number;
  warned_until: string | null;
  warned_reason: string | null;
  last_ip: string | null;
  seed_seconds: number;
  attendance_days: number;
}

interface SparkRow { id: number; username: string; amount: number; kind: string; balance_after: number; created_at: string }
interface LoginRow { id: number; username: string; ip: string; ok: boolean; created_at: string }
interface SeedRow { torrent_id: number; name: string; size: number; seeded_seconds: number; seeding: boolean; hr_flag: boolean }

const STATUS_LABELS = ["正常", "禁言", "封禁"];
const PER_PAGE = 15;

function fmtBytes(n: number): string {
  if (n >= 1099511627776) return `${(n / 1099511627776).toFixed(2)} TB`;
  if (n >= 1073741824) return `${(n / 1073741824).toFixed(2)} GB`;
  if (n >= 1048576) return `${(n / 1048576).toFixed(2)} MB`;
  return `${(n / 1024).toFixed(2)} KB`;
}
const fmtHours = (sec: number) => `${Math.floor(sec / 3600)} 小时`;

/** 权限分类 → 中文标签（与后端 permissions.category 对应） */
const CATEGORY_LABEL: Record<string, string> = {
  content: "内容管理",
  liaison: "外联",
  repost: "转载",
  seed: "做种与 H&R",
  user: "用户管理",
  system: "系统",
  site: "站点管理",
  upload: "发布管理",
};

/** 道具 kind → 中文标签（下拉分组用） */
const ITEM_KIND_LABEL: Record<string, string> = {
  upload_credit: "上传量",
  invite: "邀请类",
  temp_invite: "邀请类",
  gift_spark: "CURRENCY",
  custom_title: "头衔卡",
  rename_card: "卡牌",
  makeup_card: "卡牌",
  rainbow_name: "卡牌",
  rainbow_id: "卡牌",
  avatar_frame: "装饰",
  animated_avatar: "装饰",
  vip: "VIP",
  app_vip: "VIP",
  ad_free: "特权",
  charity: "公益",
};

export function AdminUserDetailPage() {
  const params = useParams<{ id: string }>();
  const router = useRouter();
  const { locale, dict, currency } = useI18n();
  const classList = (dict.admin as unknown as { classList: [number, string][] }).classList;
  const uid = Number(params.id);
  const [d, setD] = useState<Detail | null>(null);
  const [tab, setTab] = useState<"profile" | "spark" | "logins" | "seeding">("profile");
  const [spark, setSpark] = useState<{ rows: SparkRow[]; total: number } | null>(null);
  const [sparkPage, setSparkPage] = useState(1);
  const [logins, setLogins] = useState<{ rows: LoginRow[]; total: number } | null>(null);
  const [loginsPage, setLoginsPage] = useState(1);
  const [seeds, setSeeds] = useState<SeedRow[] | null>(null);
  const [adjust, setAdjust] = useState(false);
  const [adj, setAdj] = useState({ up: "0", down: "0", spark: "0", invite: "0", note: "" });
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
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
  const [tmpPass, setTmpPass] = useState<string | null>(null);

  const flash = (m: string) => { setMsg(m); setTimeout(() => setMsg(null), 3000); };
  const load = useCallback(async () => {
    try { setD(await api.get<Detail>(`/api/v1/admin/users/${uid}`)); }
    catch (e) { flash(e instanceof ApiError ? e.message : "加载失败"); }
  }, [uid]);
  useEffect(() => { load(); }, [load]);

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

  async function submitAdjust() {
    setBusy(true);
    try {
      await api.post("/api/v1/admin/users/adjust", {
        user_id: uid,
        uploaded_delta: Number(adj.up) || 0,
        downloaded_delta: Number(adj.down) || 0,
        spark_delta: Number(adj.spark) || 0,
        invite_grant: Number(adj.invite) || 0,
        note: adj.note || undefined,
      });
      flash("已调整");
      setAdjust(false);
      await load();
    } catch (e) { flash(e instanceof ApiError ? e.message : "操作失败"); }
    finally { setBusy(false); }
  }

  async function toggle(flag: "download_enabled" | "suspended") {
    if (!d) return;
    setBusy(true);
    try {
      await api.put("/api/v1/admin/users/flags", { user_id: uid, [flag]: !d[flag] });
      flash(flag === "suspended" ? (d.suspended ? "已解除挂起" : "已挂起") : d.download_enabled ? "已禁用下载权限" : "已恢复下载权限");
      await load();
    } catch (e) { flash(e instanceof ApiError ? e.message : "操作失败"); }
    finally { setBusy(false); }
  }

  async function changeStatus(next: number) {
    const reason = next > 0 ? (prompt(next === 2 ? "禁言理由（必填）" : "封禁理由（必填）") ?? "") : "";
    if (next > 0 && !reason.trim()) return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/users/status", { user_id: uid, status: next, reason });
      flash(next === 0 ? "已恢复正常" : next === 2 ? "已禁言" : "已封禁");
      await load();
    } catch (e) { flash(e instanceof ApiError ? e.message : "操作失败"); }
    finally { setBusy(false); }
  }

  async function submitClass() {
    if (!classId) return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/users/class", { user_id: uid, class_id: Number(classId) });
      flash("等级已修改");
      setPanel("");
      await load();
    } catch (e) { flash(e instanceof ApiError ? e.message : "操作失败"); }
    finally { setBusy(false); }
  }

  async function submitRole(grant: boolean) {
    if (!roleKey) return;
    setBusy(true);
    try {
      if (grant) {
        await api.post("/api/v1/admin/user-roles", {
          user_id: uid, role_key: roleKey,
          expires_at: roleExp || undefined,
        });
        flash("角色已分配");
      } else {
        await api.del(`/api/v1/admin/user-roles/${uid}/${roleKey}`);
        flash("角色已收回");
      }
      setPanel("");
    } catch (e) { flash(e instanceof ApiError ? e.message : "操作失败"); }
    finally { setBusy(false); }
  }

  async function submitPerm() {
    if (!permKey || !permGrant) return;
    setBusy(true);
    try {
      await api.put("/api/v1/admin/user-permissions", {
        user_id: uid, permission_key: permKey,
        granted: permGrant === "1", // true=额外授予 / false=显式拒绝
      });
      flash(permGrant === "1" ? "权限已授予" : "权限已拒绝");
      setPanel("");
    } catch (e) { flash(e instanceof ApiError ? e.message : "操作失败"); }
    finally { setBusy(false); }
  }

  async function submitMedal() {
    if (!medalId) return;
    setBusy(true);
    try {
      await api.post(`/api/v1/admin/users/${uid}/medal/${medalId}`, {});
      flash("勋章已授予");
      setPanel("");
      await load();
    } catch (e) { flash(e instanceof ApiError ? e.message : "操作失败"); }
    finally { setBusy(false); }
  }

  async function submitItem() {
    if (!itemId) return;
    setBusy(true);
    try {
      const r = await api.post<{ name: string; kind: string }>(`/api/v1/admin/users/${uid}/grant-item/${itemId}`, {});
      flash(`已发放「${r.name}」（${r.kind}）`);
      setPanel("");
      await load();
    } catch (e) { flash(e instanceof ApiError ? e.message : "操作失败"); }
    finally { setBusy(false); }
  }

  async function submitJixiao() {
    if (!jixiaoTypeId) return;
    setBusy(true);
    try {
      await api.post(`/api/v1/admin/users/${uid}/jixiao`, {
        type_id: Number(jixiaoTypeId),
        period: jixiaoPeriod || undefined,
      });
      flash("考核岗位已登记");
      setPanel("");
    } catch (e) { flash(e instanceof ApiError ? e.message : "操作失败"); }
    finally { setBusy(false); }
  }

  async function submitNewRole() {
    if (!newRole.key.trim() || !newRole.name.trim()) return;
    setBusy(true);
    try {
      await api.post("/api/v1/admin/roles", {
        key: newRole.key, name: newRole.name, descr: newRole.descr || undefined,
      });
      flash(`职务「${newRole.name}」已创建`);
      setNewRole({ key: "", name: "", descr: "" });
      const list = await api.get<{ key: string; name: string }[]>("/api/v1/admin/roles");
      setRoles(list);
    } catch (e) { flash(e instanceof ApiError ? e.message : "操作失败"); }
    finally { setBusy(false); }
  }

  async function resetPass() {
    if (!window.confirm(`确认重置 ${d?.username} 的密码？将生成一次性临时密码。`)) return;
    setBusy(true);
    try {
      const r = await api.post<{ temp_password: string }>("/api/v1/admin/resetpass", { user_id: uid });
      setTmpPass(r.temp_password);
    } catch (e) { flash(e instanceof ApiError ? e.message : "操作失败"); }
    finally { setBusy(false); }
  }

  async function deleteUser() {
    if (!window.confirm(`确认删除用户 ${d?.username}（#${uid}）？仅封禁状态可删，数据不可恢复！`)) return;
    setBusy(true);
    try {
      await api.del(`/api/v1/admin/users/${uid}`);
      flash("用户已删除，返回列表…");
      setTimeout(() => router.push("/admin?tool=users"), 800);
    } catch (e) { flash(e instanceof ApiError ? e.message : "操作失败"); }
    finally { setBusy(false); }
  }

  const paged = (n: number, total: number, set: (v: number) => void, cur: number) => (
    <div className="flex items-center justify-between text-xs text-sub">
      <span>共 {total} 条</span>
      <div className="flex items-center gap-2">
        <button disabled={cur <= 1} onClick={() => set(cur - 1)} className="min-h-[32px] rounded-full border border-line px-3 disabled:opacity-40">上一页</button>
        <span>{cur} / {Math.max(1, Math.ceil(total / PER_PAGE))}</span>
        <button disabled={cur >= Math.ceil(total / PER_PAGE)} onClick={() => set(cur + 1)} className="min-h-[32px] rounded-full border border-line px-3 disabled:opacity-40">下一页</button>
      </div>
    </div>
  );

  if (!d) return <p className="py-8 text-center text-sub">{msg ?? "加载中…"}</p>;

  const dt = (s: string | null) => (s ? new Date(s).toLocaleString(dateLocale(locale)) : "—");

  return (
    <div className="flex flex-col gap-3">
      {msg && <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">{msg}</p>}

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
          <section className="baozi-panel p-4">
            <h2 className="mb-2 text-base font-bold text-ink">账号资料</h2>
            <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm md:grid-cols-3">
              <div><dt className="text-sub">邮箱</dt><dd>{d.email}</dd></div>
              <div><dt className="text-sub">Passkey</dt><dd className="font-mono text-xs">{d.passkey.slice(0, 8)}…{d.passkey.slice(-4)}</dd></div>
              <div><dt className="text-sub">两步验证</dt><dd>{d.totp_enabled ? "已开启" : "未开启"}</dd></div>
              <div><dt className="text-sub">邀请人</dt><dd>{d.inviter_name ? `${d.inviter_name} (#${d.invited_by})` : "—"}</dd></div>
              <div><dt className="text-sub">添加时间</dt><dd>{dt(d.created_at)}</dd></div>
              <div><dt className="text-sub">最后访问</dt><dd>{dt(d.last_seen_at)}</dd></div>
              <div><dt className="text-sub">最近登录 IP</dt><dd>{d.last_ip ?? "—"}</dd></div>
              <div><dt className="text-sub">未用邀请</dt><dd>{d.invites_unused}</dd></div>
              <div><dt className="text-sub">签到天数</dt><dd>{d.attendance_days}</dd></div>
              {d.warned_until && (
                <div className="md:col-span-3"><dt className="text-sub">警告</dt><dd className="text-danger">至 {dt(d.warned_until)} · {d.warned_reason ?? "无理由记录"}</dd></div>
              )}
            </dl>
          </section>

          <section className="baozi-panel p-4">
            <h2 className="mb-2 text-base font-bold text-ink">数据统计</h2>
            <dl className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm md:grid-cols-4">
              <div><dt className="text-sub">上传量</dt><dd>{fmtBytes(d.uploaded)}</dd></div>
              <div><dt className="text-sub">下载量</dt><dd>{fmtBytes(d.downloaded)}</dd></div>
              <div><dt className="text-sub">分享率</dt><dd>{d.downloaded > 0 ? (d.uploaded / d.downloaded).toFixed(3) : "∞"}</dd></div>
              <div><dt className="text-sub">{currency}余额</dt><dd>{d.spark_balance.toLocaleString()}</dd></div>
              <div><dt className="text-sub">发布种子</dt><dd>{d.uploads}</dd></div>
              <div><dt className="text-sub">完成下载</dt><dd>{d.downloaded_count}</dd></div>
              <div><dt className="text-sub">评论</dt><dd>{d.comments}</dd></div>
              <div><dt className="text-sub">勋章</dt><dd>{d.medals}</dd></div>
              <div><dt className="text-sub">做种中</dt><dd>{d.seeding}</dd></div>
              <div><dt className="text-sub">下载中</dt><dd>{d.leeching}</dd></div>
              <div><dt className="text-sub">做种总时长</dt><dd>{fmtHours(d.seed_seconds)}</dd></div>
            </dl>
          </section>

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
              <button disabled={busy} onClick={resetPass} className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold">重置密码</button>
              <button disabled={busy} onClick={() => toggle("download_enabled")}
                className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${d.download_enabled ? "border border-line text-danger" : "bg-mint text-white"}`}>
                {d.download_enabled ? "禁用下载权限" : "恢复下载权限"}
              </button>
              <button disabled={busy} onClick={() => toggle("suspended")}
                className={`min-h-[36px] rounded-full px-4 text-xs font-bold ${d.suspended ? "bg-mint text-white" : "border border-line text-danger"}`}>
                {d.suspended ? "解除挂起" : "挂起账号"}
              </button>
              {d.status === 1 && <button disabled={busy} onClick={() => changeStatus(0)} className="min-h-[36px] rounded-full bg-mint px-4 text-xs font-bold text-white">解除禁言</button>}
              {d.status === 2 && <button disabled={busy} onClick={() => changeStatus(0)} className="min-h-[36px] rounded-full bg-mint px-4 text-xs font-bold text-white">解除封禁</button>}
              {d.status === 0 && <button disabled={busy} onClick={() => changeStatus(1)} className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-danger">禁言</button>}
              {d.status < 2 && <button disabled={busy} onClick={() => changeStatus(2)} className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-danger">封禁</button>}
              {d.status >= 2 && <button disabled={busy} onClick={deleteUser} className="min-h-[36px] rounded-full bg-coral px-4 text-xs font-bold text-white">删除用户</button>}
            </div>

            {tmpPass && (
              <p className="rounded-[var(--r-md)] bg-sky-soft p-3 font-mono text-sm text-ink">
                临时密码（仅显示一次）：{tmpPass}，用户首登需改密。
              </p>
            )}

            {/* 等级修改 */}
            {panel === "class" && (
              <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
                <p className="mb-2 text-xs text-sub">当前 {d.class_name ?? `LV${d.class_id}`}；仅站长可改，且不可设为站长。</p>
                <div className="flex flex-wrap items-center gap-2">
                  <select value={classId} onChange={(e) => setClassId(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
                    <option value="">选择新等级</option>
                    {classList.map(([id, label]) => (id < 99 ? <option key={id} value={id}>{id} {label}</option> : null))}
                  </select>
                  <button className="baozi-button" disabled={busy || !classId} onClick={submitClass}>提交</button>
                </div>
              </div>
            )}

            {/* 分配角色 */}
            {panel === "role" && (
              <div className="cmgmt-form flex flex-col gap-3 rounded-[var(--r-md)] border border-line p-3">
                <p className="text-xs text-sub">职务可兼任；到期自动失效（可选）。</p>
                <div className="flex flex-wrap items-center gap-2">
                  <select value={roleKey} onChange={(e) => setRoleKey(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
                    <option value="">选择职务</option>
                    {roles.map((r) => <option key={r.key} value={r.key}>{r.name}</option>)}
                  </select>
                  <input type="datetime-local" value={roleExp} onChange={(e) => setRoleExp(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" title="到期时间（可选）" />
                  <button className="baozi-button" disabled={busy || !roleKey} onClick={() => submitRole(true)}>分配</button>
                  <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold text-danger" disabled={busy || !roleKey} onClick={() => submitRole(false)}>收回</button>
                </div>
                {/* 新增职务（sysop）：分配前发现缺角色可直接建 */}
                <details className="rounded-[var(--r-sm)] border border-dashed border-line p-2">
                  <summary className="cursor-pointer text-xs font-bold text-sub">＋ 新增职务（需要新角色时在此创建）</summary>
                  <div className="mt-2 flex flex-wrap items-end gap-2">
                    <label className="flex flex-col gap-1 text-xs">Key（小写/下划线）
                      <input value={newRole.key} onChange={(e) => setNewRole({ ...newRole, key: e.target.value })} placeholder="translator" className="min-h-[40px] w-40 rounded-[var(--r-sm)] border border-line px-2" /></label>
                    <label className="flex flex-col gap-1 text-xs">名称
                      <input value={newRole.name} onChange={(e) => setNewRole({ ...newRole, name: e.target.value })} placeholder="翻译员" className="min-h-[40px] w-32 rounded-[var(--r-sm)] border border-line px-2" /></label>
                    <label className="flex flex-col gap-1 text-xs">说明（可选）
                      <input value={newRole.descr} onChange={(e) => setNewRole({ ...newRole, descr: e.target.value })} className="min-h-[40px] w-48 rounded-[var(--r-sm)] border border-line px-2" /></label>
                    <button className="baozi-button" disabled={busy || !newRole.key.trim() || !newRole.name.trim()} onClick={submitNewRole}>创建</button>
                  </div>
                </details>
              </div>
            )}

            {/* 分配权限 */}
            {panel === "perm" && (
              <div className="cmgmt-form flex flex-col gap-2 rounded-[var(--r-md)] border border-line p-3">
                <p className="text-xs text-sub">覆盖角色判定：授予 = 额外允许；拒绝 = 显式禁止。</p>
                {permData && (
                  <p className="text-xs text-sub">
                    生效权限 {permData.effective.length} 项
                    {permData.overrides.length > 0 && `（含 ${permData.overrides.length} 项个人覆盖）`}
                  </p>
                )}
                <div className="flex flex-wrap items-center gap-2">
                  <select value={permKey} onChange={(e) => setPermKey(e.target.value)} className="min-h-[40px] max-w-96 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
                    <option value="">选择权限</option>
                    {Object.entries(
                      perms.reduce<Record<string, typeof perms>>((acc, p) => {
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
                  <select value={permGrant} onChange={(e) => setPermGrant(e.target.value as "1" | "0" | "")} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
                    <option value="">授予/拒绝</option>
                    <option value="1">授予（额外允许）</option>
                    <option value="0">拒绝（显式禁止）</option>
                  </select>
                  <button className="baozi-button" disabled={busy || !permKey || !permGrant} onClick={submitPerm}>提交</button>
                </div>
              </div>
            )}

            {/* 授予勋章 */}
            {panel === "medal" && (
              <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
                <p className="mb-2 text-xs text-sub">管理发放（source=admin），已拥有 {d.medals} 枚。</p>
                <div className="flex flex-wrap items-center gap-2">
                  <select value={medalId} onChange={(e) => setMedalId(e.target.value)} className="min-h-[40px] max-w-72 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
                    <option value="">选择勋章</option>
                    {medals.map((m) => <option key={m.id} value={m.id}>#{m.id} {m.name}</option>)}
                  </select>
                  <button className="baozi-button" disabled={busy || !medalId} onClick={submitMedal}>授予</button>
                </div>
              </div>
            )}

            {/* 授予道具（含卡牌/装饰类） */}
            {panel === "item" && (
              <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
                <p className="mb-2 text-xs text-sub">免费发放：上传量/{currency}/邀请即时生效；化妆卡、改名卡等卡牌道具入背包待用户使用。</p>
                <div className="flex flex-wrap items-center gap-2">
                  <select value={itemId} onChange={(e) => setItemId(e.target.value)} className="min-h-[40px] max-w-80 rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
                    <option value="">选择道具</option>
                    {Object.entries(
                      items.reduce<Record<string, typeof items>>((acc, it) => {
                        (acc[(ITEM_KIND_LABEL[it.kind] ?? it.kind).replaceAll("CURRENCY", currency)] ??= []).push(it);
                        return acc;
                      }, {}),
                    ).map(([kind, list]) => (
                      <optgroup key={kind} label={kind}>
                        {list.map((it) => <option key={it.id} value={it.id}>#{it.id} {it.name}</option>)}
                      </optgroup>
                    ))}
                  </select>
                  <button className="baozi-button" disabled={busy || !itemId} onClick={submitItem}>发放</button>
                </div>
              </div>
            )}

            {/* 分配考核 */}
            {panel === "jixiao" && (
              <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
                <p className="mb-2 text-xs text-sub">登记为考核岗位后按系统流水自动核算绩效；期间留空默认当月。</p>
                <div className="flex flex-wrap items-center gap-2">
                  <select value={jixiaoTypeId} onChange={(e) => setJixiaoTypeId(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line bg-[var(--surface-card)] px-2">
                    <option value="">选择考核岗位</option>
                    {jixiaoTypes.map((j) => <option key={j.id} value={j.id}>#{j.id} {j.name}</option>)}
                  </select>
                  <input type="month" value={jixiaoPeriod} onChange={(e) => setJixiaoPeriod(e.target.value)} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" title="考核期间（默认当月）" />
                  <button className="baozi-button" disabled={busy || !jixiaoTypeId} onClick={submitJixiao}>登记</button>
                </div>
              </div>
            )}
            {adjust && (
              <div className="cmgmt-form rounded-[var(--r-md)] border border-line p-3">
                <p className="mb-2 text-xs text-sub">正数增加、负数减少（下限 0）；邀请正数增发、负数回收。</p>
                <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
                  <label className="flex flex-col gap-1 text-xs">上传量增量（字节）
                    <input type="number" value={adj.up} onChange={(e) => setAdj({ ...adj, up: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" /></label>
                  <label className="flex flex-col gap-1 text-xs">下载量增量（字节）
                    <input type="number" value={adj.down} onChange={(e) => setAdj({ ...adj, down: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" /></label>
                  <label className="flex flex-col gap-1 text-xs">{currency}增量
                    <input type="number" value={adj.spark} onChange={(e) => setAdj({ ...adj, spark: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" /></label>
                  <label className="flex flex-col gap-1 text-xs">邀请增发/回收
                    <input type="number" value={adj.invite} onChange={(e) => setAdj({ ...adj, invite: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" /></label>
                </div>
                <label className="mt-2 flex flex-col gap-1 text-xs">备注（入审计）
                  <input value={adj.note} onChange={(e) => setAdj({ ...adj, note: e.target.value })} className="min-h-[40px] rounded-[var(--r-sm)] border border-line px-2" /></label>
                <div className="mt-2 flex gap-2">
                  <button className="baozi-button" disabled={busy} onClick={submitAdjust}>提交调整</button>
                  <button className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold" onClick={() => setAdjust(false)}>取消</button>
                </div>
              </div>
            )}
          </section>
        </>
      )}

      {tab === "spark" && (
        <section className="baozi-panel flex flex-col gap-2 p-4">
          {paged(PER_PAGE, spark?.total ?? 0, setSparkPage, sparkPage)}
          <div className="baozi-wide-table-scroll">
            <table className="nexus-table text-xs">
              <thead><tr>
                <td className="colhead">变动</td><td className="colhead">类型</td>
                <td className="colhead">变动后余额</td><td className="colhead">时间</td>
              </tr></thead>
              <tbody>
                {spark?.rows.map((r) => (
                  <tr key={r.id}>
                    <td className={`num font-bold ${r.amount >= 0 ? "text-mint" : "text-danger"}`}>{r.amount >= 0 ? "+" : ""}{r.amount.toLocaleString()}</td>
                    <td>{r.kind}</td>
                    <td className="num">{r.balance_after.toLocaleString()}</td>
                    <td className="text-sub">{dt(r.created_at)}</td>
                  </tr>
                ))}
                {spark?.rows.length === 0 && <tr><td colSpan={4} className="py-4 text-center text-sub">暂无流水</td></tr>}
              </tbody>
            </table>
          </div>
        </section>
      )}

      {tab === "logins" && (
        <section className="baozi-panel flex flex-col gap-2 p-4">
          {paged(PER_PAGE, logins?.total ?? 0, setLoginsPage, loginsPage)}
          <div className="baozi-wide-table-scroll">
            <table className="nexus-table text-xs">
              <thead><tr>
                <td className="colhead">IP</td><td className="colhead">结果</td><td className="colhead">时间</td>
              </tr></thead>
              <tbody>
                {logins?.rows.map((r) => (
                  <tr key={r.id}>
                    <td className="font-mono">{r.ip}</td>
                    <td className={r.ok ? "text-mint" : "text-danger"}>{r.ok ? "成功" : "失败"}</td>
                    <td className="text-sub">{dt(r.created_at)}</td>
                  </tr>
                ))}
                {logins?.rows.length === 0 && <tr><td colSpan={3} className="py-4 text-center text-sub">暂无登录记录</td></tr>}
              </tbody>
            </table>
          </div>
        </section>
      )}

      {tab === "seeding" && (
        <section className="baozi-panel p-4">
          <div className="baozi-wide-table-scroll">
            <table className="nexus-table text-xs">
              <thead><tr>
                <td className="colhead">种子</td><td className="colhead w-24">大小</td>
                <td className="colhead w-24">状态</td><td className="colhead w-24">做种时长</td>
                <td className="colhead w-16">H&R</td>
              </tr></thead>
              <tbody>
                {seeds?.map((s) => (
                  <tr key={s.torrent_id}>
                    <td><a className="text-link" href={`/torrent/${s.torrent_id}`}>{s.name}</a></td>
                    <td className="num">{fmtBytes(s.size)}</td>
                    <td>{s.seeding ? <span className="text-mint">做种中</span> : <span className="text-sub">已停</span>}</td>
                    <td className="num">{fmtHours(s.seeded_seconds)}</td>
                    <td>{s.hr_flag ? <span className="text-danger">命中</span> : "—"}</td>
                  </tr>
                ))}
                {seeds?.length === 0 && <tr><td colSpan={5} className="py-4 text-center text-sub">暂无做种/下载记录</td></tr>}
              </tbody>
            </table>
          </div>
        </section>
      )}
    </div>
  );
}
