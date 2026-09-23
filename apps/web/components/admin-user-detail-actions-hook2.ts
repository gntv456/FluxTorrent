"use client";

/**
 * 后台用户详情·管理动作提交域（从 components/admin-user-detail-actions-hook.ts
 * 按域拆出）：新建职务 / 改名 / 重置密码 / 删除用户等 API 动作。
 */

import { useCallback } from "react";
import { useRouter } from "next/navigation";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { ActionFormValues } from "./admin-user-detail-actions-hook";
import type { Detail } from "./admin-user-detail-shared";

export function useAdminUserActions2({
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
  onTmpPass,
}: {
  uid: number;
  d: Detail | null;
  values: ActionFormValues;
  setPanel: (
    p: "" | "class" | "role" | "perm" | "medal" | "item" | "jixiao",
  ) => void;
  setRenameOpen: (v: boolean) => void;
  setNewName: (v: string) => void;
  setNewRole: (v: { key: string; name: string; descr: string }) => void;
  setRoles: (v: { key: string; name: string }[]) => void;
  flash: (m: string) => void;
  load: () => Promise<void>;
  run: (task: () => Promise<void>) => Promise<void>;
  onTmpPass: (p: string) => void;
}) {
  const router = useRouter();
  const { dict } = useI18n();
  const t = dict.userDetail;

  const submitNewRole = useCallback(
    () =>
      run(async () => {
        if (!values.newRole.key.trim() || !values.newRole.name.trim()) return;
        await api.post("/api/v1/admin/roles", {
          key: values.newRole.key,
          name: values.newRole.name,
          descr: values.newRole.descr || undefined,
        });
        flash(fmt(t.roleCreated, { name: values.newRole.name }));
        setNewRole({ key: "", name: "", descr: "" });
        const list = await api.get<{ key: string; name: string }[]>(
          "/api/v1/admin/roles",
        );
        setRoles(list);
      }),
    [values.newRole, flash, setNewRole, setRoles],
  );

  const submitRename = useCallback(
    () =>
      run(async () => {
        const rn = dict.adminrename;
        const name = values.newName.trim();
        if (!name || !d) return;
        if (
          !window.confirm(
            rn.confirm.replace("{old}", d.username).replace("{new}", name),
          )
        )
          return;
        await api.post(`/api/v1/admin/users/${uid}/rename`, { new_name: name });
        flash(rn.ok.replace("{name}", name));
        setRenameOpen(false);
        setNewName("");
        await load();
      }),
    [
      dict.adminrename,
      values.newName,
      d,
      uid,
      flash,
      setRenameOpen,
      setNewName,
      load,
    ],
  );

  const resetPass = useCallback(
    () =>
      run(async () => {
        if (
          !window.confirm(
            fmt(t.resetPwdConfirm, { name: d?.username ?? "" }),
          )
        )
          return;
        const r = await api.post<{ temp_password: string }>(
          "/api/v1/admin/resetpass",
          { user_id: uid },
        );
        onTmpPass(r.temp_password);
      }),
    [d, uid, onTmpPass],
  );

  const deleteUser = useCallback(
    () =>
      run(async () => {
        if (
          !window.confirm(
            fmt(t.delUserConfirm, { name: d?.username ?? "", id: uid }),
          )
        )
          return;
        await api.del(`/api/v1/admin/users/${uid}`);
        flash(t.userDeleted);
        setTimeout(() => router.push("/admin?tool=users"), 800);
      }),
    [d, uid, flash, router],
  );

  return {
    submitNewRole,
    submitRename,
    resetPass,
    deleteUser,
  };
}
