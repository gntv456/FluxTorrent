"use client";

/**
 * 后台用户详情·管理动作提交域（从 components/admin-user-detail.tsx 按域拆出）：
 * useAdminUserActions 聚合调整 / 等级 / 角色 / 权限 / 勋章 / 道具 / 考核 /
 * 改名 / 重置密码 / 状态切换 / 删除用户等 API 动作与 busy/flash 状态。
 * 面板表单值（classId / roleKey / …）仍由调用方持有并传入。
 */

import { useCallback, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import type { Detail } from "./admin-user-detail-shared";

export interface ActionFormValues {
  classId: string;
  roleKey: string;
  roleExp: string;
  newRole: { key: string; name: string; descr: string };
  permKey: string;
  permGrant: "1" | "0" | "";
  medalId: string;
  itemId: string;
  jixiaoTypeId: string;
  jixiaoPeriod: string;
  newName: string;
  adj: { up: string; down: string; spark: string; invite: string; note: string };
}

export function useAdminUserActions({
  uid,
  d,
  values,
  load,
  setPanel,
  setAdjust,
  setRenameOpen,
  setNewName,
  setNewRole,
  setRoles,
}: {
  uid: number;
  d: Detail | null;
  values: ActionFormValues;
  load: () => Promise<void>;
  setPanel: (p: "" | "class" | "role" | "perm" | "medal" | "item" | "jixiao") => void;
  setAdjust: (v: boolean) => void;
  setRenameOpen: (v: boolean) => void;
  setNewName: (v: string) => void;
  setNewRole: (v: { key: string; name: string; descr: string }) => void;
  setRoles: (v: { key: string; name: string }[]) => void;
}) {
  const router = useRouter();
  const { dict } = useI18n();
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [tmpPass, setTmpPass] = useState<string | null>(null);

  const flash = useCallback((m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  }, []);

  async function run(task: () => Promise<void>) {
    setBusy(true);
    try {
      await task();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  const submitAdjust = useCallback(() => run(async () => {
    const adj = values.adj;
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
  }), [values.adj, uid, flash, setAdjust, load]);

  const toggle = useCallback((flag: "download_enabled" | "suspended") => run(async () => {
    if (!d) return;
    await api.put("/api/v1/admin/users/flags", { user_id: uid, [flag]: !d[flag] });
    flash(flag === "suspended" ? (d.suspended ? "已解除挂起" : "已挂起") : d.download_enabled ? "已禁用下载权限" : "已恢复下载权限");
    await load();
  }), [d, uid, flash, load]);

  const changeStatus = useCallback((next: number) => run(async () => {
    const reason = next > 0 ? (prompt(next === 2 ? "禁言理由（必填）" : "封禁理由（必填）") ?? "") : "";
    if (next > 0 && !reason.trim()) return;
    await api.post("/api/v1/admin/users/status", { user_id: uid, status: next, reason });
    flash(next === 0 ? "已恢复正常" : next === 2 ? "已禁言" : "已封禁");
    await load();
  }), [uid, flash, load]);

  const submitClass = useCallback(() => run(async () => {
    if (!values.classId) return;
    await api.post("/api/v1/admin/users/class", { user_id: uid, class_id: Number(values.classId) });
    flash("等级已修改");
    setPanel("");
    await load();
  }), [values.classId, uid, flash, setPanel, load]);

  const submitRole = useCallback((grant: boolean) => run(async () => {
    if (!values.roleKey) return;
    if (grant) {
      await api.post("/api/v1/admin/user-roles", {
        user_id: uid, role_key: values.roleKey,
        expires_at: values.roleExp || undefined,
      });
      flash("角色已分配");
    } else {
      await api.del(`/api/v1/admin/user-roles/${uid}/${values.roleKey}`);
      flash("角色已收回");
    }
    setPanel("");
  }), [values.roleKey, values.roleExp, uid, flash, setPanel]);

  const submitPerm = useCallback(() => run(async () => {
    if (!values.permKey || !values.permGrant) return;
    await api.put("/api/v1/admin/user-permissions", {
      user_id: uid, permission_key: values.permKey,
      granted: values.permGrant === "1", // true=额外授予 / false=显式拒绝
    });
    flash(values.permGrant === "1" ? "权限已授予" : "权限已拒绝");
    setPanel("");
  }), [values.permKey, values.permGrant, uid, flash, setPanel]);

  const submitMedal = useCallback(() => run(async () => {
    if (!values.medalId) return;
    await api.post(`/api/v1/admin/users/${uid}/medal/${values.medalId}`, {});
    flash("勋章已授予");
    setPanel("");
    await load();
  }), [values.medalId, uid, flash, setPanel, load]);

  const submitItem = useCallback(() => run(async () => {
    if (!values.itemId) return;
    const r = await api.post<{ name: string; kind: string }>(`/api/v1/admin/users/${uid}/grant-item/${values.itemId}`, {});
    flash(`已发放「${r.name}」（${r.kind}）`);
    setPanel("");
    await load();
  }), [values.itemId, uid, flash, setPanel, load]);

  const submitJixiao = useCallback(() => run(async () => {
    if (!values.jixiaoTypeId) return;
    await api.post(`/api/v1/admin/users/${uid}/jixiao`, {
      type_id: Number(values.jixiaoTypeId),
      period: values.jixiaoPeriod || undefined,
    });
    flash("考核岗位已登记");
    setPanel("");
  }), [values.jixiaoTypeId, values.jixiaoPeriod, uid, flash, setPanel]);

  const submitNewRole = useCallback(() => run(async () => {
    if (!values.newRole.key.trim() || !values.newRole.name.trim()) return;
    await api.post("/api/v1/admin/roles", {
      key: values.newRole.key, name: values.newRole.name, descr: values.newRole.descr || undefined,
    });
    flash(`职务「${values.newRole.name}」已创建`);
    setNewRole({ key: "", name: "", descr: "" });
    const list = await api.get<{ key: string; name: string }[]>("/api/v1/admin/roles");
    setRoles(list);
  }), [values.newRole, flash, setNewRole, setRoles]);

  const submitRename = useCallback(() => run(async () => {
    const rn = dict.adminrename;
    const name = values.newName.trim();
    if (!name || !d) return;
    if (!window.confirm(rn.confirm.replace("{old}", d.username).replace("{new}", name))) return;
    await api.post(`/api/v1/admin/users/${uid}/rename`, { new_name: name });
    flash(rn.ok.replace("{name}", name));
    setRenameOpen(false);
    setNewName("");
    await load();
  }), [dict.adminrename, values.newName, d, uid, flash, setRenameOpen, setNewName, load]);

  const resetPass = useCallback(() => run(async () => {
    if (!window.confirm(`确认重置 ${d?.username} 的密码？将生成一次性临时密码。`)) return;
    const r = await api.post<{ temp_password: string }>("/api/v1/admin/resetpass", { user_id: uid });
    setTmpPass(r.temp_password);
  }), [d, uid]);

  const deleteUser = useCallback(() => run(async () => {
    if (!window.confirm(`确认删除用户 ${d?.username}（#${uid}）？仅封禁状态可删，数据不可恢复！`)) return;
    await api.del(`/api/v1/admin/users/${uid}`);
    flash("用户已删除，返回列表…");
    setTimeout(() => router.push("/admin?tool=users"), 800);
  }), [d, uid, flash, router]);

  return {
    msg,
    setMsg,
    flash,
    busy,
    tmpPass,
    submitAdjust,
    toggle,
    changeStatus,
    submitClass,
    submitRole,
    submitPerm,
    submitMedal,
    submitItem,
    submitJixiao,
    submitNewRole,
    submitRename,
    resetPass,
    deleteUser,
  };
}
