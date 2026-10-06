"use client";

/**
 * 后台用户详情·管理动作提交域（从 components/admin-user-detail.tsx 按域拆出）：
 * useAdminUserActions 聚合调整 / 等级 / 角色 / 权限 / 勋章 / 道具 / 考核 /
 * 改名 / 重置密码 / 状态切换 / 删除用户等 API 动作与 busy/flash 状态。
 * 面板表单值（classId / roleKey / …）仍由调用方持有并传入。
 * 新建职务 / 改名 / 重置密码 / 删除用户拆至
 * ./admin-user-detail-actions-hook2.ts。 */

import { useCallback, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { Detail } from "./admin-user-detail-shared";
import { useAdminUserActions2 } from "./admin-user-detail-actions-hook2";

export interface ActionFormValues {
  classId: string;
  roleKey: string;
  roleExp: string;
  newRole: { key: string; name: string; descr: string };
  permKey: string;
  permGrant: "1" | "0" | "";
  medalId: string;
  medalDays: string;
  itemId: string;
  jixiaoTypeId: string;
  jixiaoPeriod: string;
  newName: string;
  adj: {
    up: string;
    down: string;
    spark: string;
    invite: string;
    note: string;
    idem: string;
  };
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
  setPanel: (
    p: "" | "class" | "role" | "perm" | "medal" | "item" | "jixiao",
  ) => void;
  setAdjust: (v: boolean) => void;
  setRenameOpen: (v: boolean) => void;
  setNewName: (v: string) => void;
  setNewRole: (v: { key: string; name: string; descr: string }) => void;
  setRoles: (v: { key: string; name: string }[]) => void;
}) {
  const router = useRouter();
  const { dict } = useI18n();
  const t = dict.userDetail;
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
      flash(e instanceof ApiError ? e.message : t.actionFail);
    } finally {
      setBusy(false);
    }
  }

  const submitAdjust = useCallback(
    () =>
      run(async () => {
        const adj = values.adj;
        await api.post("/api/v1/admin/users/adjust", {
          user_id: uid,
          uploaded_delta: Number(adj.up) || 0,
          downloaded_delta: Number(adj.down) || 0,
          spark_delta: Number(adj.spark) || 0,
          invite_grant: Number(adj.invite) || 0,
          note: adj.note || undefined,
          idempotency_key: adj.idem?.trim() || undefined,
        });
        flash(t.adjusted);
        setAdjust(false);
        await load();
      }),
    [values.adj, uid, flash, setAdjust, load],
  );

  const toggle = useCallback(
    (flag: "download_enabled" | "suspended") =>
      run(async () => {
        if (!d) return;
        await api.put("/api/v1/admin/users/flags", {
          user_id: uid,
          [flag]: !d[flag],
        });
        flash(
          flag === "suspended"
            ? d.suspended
              ? t.unsuspended
              : t.suspended
            : d.download_enabled
              ? t.downloadDisabled
              : t.downloadEnabled,
        );
        await load();
      }),
    [d, uid, flash, load],
  );

  const changeStatus = useCallback(
    (next: number) =>
      run(async () => {
        const reason =
          next > 0
            ? (prompt(next === 2 ? t.muteReason : t.banReason) ??
              "")
            : "";
        if (next > 0 && !reason.trim()) return;
        await api.post("/api/v1/admin/users/status", {
          user_id: uid,
          status: next,
          reason,
        });
        flash(next === 0 ? t.restored : next === 2 ? t.muted : t.banned);
        await load();
      }),
    [uid, flash, load],
  );

  const submitClass = useCallback(
    () =>
      run(async () => {
        if (!values.classId) return;
        await api.post("/api/v1/admin/users/class", {
          user_id: uid,
          class_id: Number(values.classId),
        });
        flash(t.classChanged);
        setPanel("");
        await load();
      }),
    [values.classId, uid, flash, setPanel, load],
  );

  const submitRole = useCallback(
    (grant: boolean) =>
      run(async () => {
        if (!values.roleKey) return;
        if (grant) {
          await api.post("/api/v1/admin/user-roles", {
            user_id: uid,
            role_key: values.roleKey,
            expires_at: values.roleExp || undefined,
          });
          flash(t.roleAssigned);
        } else {
          await api.del(`/api/v1/admin/user-roles/${uid}/${values.roleKey}`);
          flash(t.roleRevoked);
        }
        setPanel("");
      }),
    [values.roleKey, values.roleExp, uid, flash, setPanel],
  );

  const submitPerm = useCallback(
    () =>
      run(async () => {
        if (!values.permKey || !values.permGrant) return;
        await api.put("/api/v1/admin/user-permissions", {
          user_id: uid,
          permission_key: values.permKey,
          granted: values.permGrant === "1", // true=额外授予 / false=显式拒绝
        });
        flash(values.permGrant === "1" ? t.permGranted : t.permDenied);
        setPanel("");
      }),
    [values.permKey, values.permGrant, uid, flash, setPanel],
  );

  const submitMedal = useCallback(
    () =>
      run(async () => {
        if (!values.medalId) return;
        const days = Number(values.medalDays);
        await api.post(
          `/api/v1/admin/users/${uid}/medal/${values.medalId}`,
          Number.isFinite(days) && values.medalDays.trim() !== ""
            ? { days }
            : {},
        );
        flash(t.medalGranted);
        setPanel("");
        await load();
      }),
    [values.medalId, values.medalDays, uid, flash, setPanel, load],
  );

  const submitItem = useCallback(
    () =>
      run(async () => {
        if (!values.itemId) return;
        const r = await api.post<{ name: string; kind: string }>(
          `/api/v1/admin/users/${uid}/grant-item/${values.itemId}`,
          {},
        );
        flash(fmt(t.itemGranted, { name: r.name, kind: r.kind }));
        setPanel("");
        await load();
      }),
    [values.itemId, uid, flash, setPanel, load],
  );

  const submitJixiao = useCallback(
    () =>
      run(async () => {
        if (!values.jixiaoTypeId) return;
        await api.post(`/api/v1/admin/users/${uid}/jixiao`, {
          type_id: Number(values.jixiaoTypeId),
          period: values.jixiaoPeriod || undefined,
        });
        flash(t.examPositionSet);
        setPanel("");
      }),
    [values.jixiaoTypeId, values.jixiaoPeriod, uid, flash, setPanel],
  );

  const second = useAdminUserActions2({
    uid,
    d,
    values,
    setPanel,
    setRenameOpen,
    setNewName,
    setNewRole,
    setRoles,
    flash,
    load,
    run,
    onTmpPass: setTmpPass,
  });

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
    ...second,
  };
}
