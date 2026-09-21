"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";

import {
  PER_PAGE,
  type Detail,
  type DetailTab,
  type LoginRow,
  type SeedRow,
  type SparkRow,
} from "./admin-user-detail-shared";

/** 后台用户详情（好学站 Filament UserResource 的 user-profile 口径）：
 *  从 components/admin-user-detail.tsx 按域拆出：
 *  1) useDetailLoad —— 用户详情本体加载；
 *  2) useAdminPanelDicts —— NP 级管理操作面板（class/role/perm/medal/
 *     item/jixiao）的开关、表单值与字典/现状按需拉取（等级字典来自
 *     i18n classList，无需请求）；
 *  3) useDetailTabs —— 关联数据 tab（火花流水 / 登录记录 / 做种列表）
 *     的翻页与数据拉取。 */

/** 加载用户详情本体。 */
export function useDetailLoad(uid: number) {
  const [d, setD] = useState<Detail | null>(null);
  const [msg, setMsg] = useState<string | null>(null);
  const load = useCallback(async () => {
    try {
      setD(await api.get<Detail>(`/api/v1/admin/users/${uid}`));
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : "加载失败");
    }
  }, [uid]);
  useEffect(() => {
    load();
  }, [load]);
  return { d, msg, load };
}

export type AdminPanel =
  "" | "class" | "role" | "perm" | "medal" | "item" | "jixiao";

type RoleItem = { key: string; name: string };
type RoleSetter = React.Dispatch<React.SetStateAction<RoleItem[]>>;
type NewRole = { key: string; name: string; descr: string };
type NewRoleSetter = React.Dispatch<React.SetStateAction<NewRole>>;

/** NP 级管理操作面板的开关、表单值与按需字典。 */
export function useAdminPanelDicts(uid: number) {
  // NP 级管理操作面板
  const [panel, setPanel] = useState<AdminPanel>("");
  const [classId, setClassId] = useState("");
  const [roles, setRoles] = useState<RoleItem[]>([]);
  const [roleKey, setRoleKey] = useState("");
  const [roleExp, setRoleExp] = useState("");
  const [newRole, setNewRole] = useState<NewRole>({
    key: "",
    name: "",
    descr: "",
  });
  const [perms, setPerms] = useState<
    {
      key: string;
      name: string | null;
      category: string | null;
      descr: string | null;
    }[]
  >([]);
  const [permData, setPermData] = useState<{
    effective: string[];
    overrides: { permission_key: string; granted: boolean }[];
  } | null>(null);
  const [permKey, setPermKey] = useState("");
  const [permGrant, setPermGrant] = useState<"1" | "0" | "">("");
  const [medals, setMedals] = useState<{ id: number; name: string }[]>([]);
  const [medalId, setMedalId] = useState("");
  const [items, setItems] = useState<
    { id: number; name: string; kind: string }[]
  >([]);
  const [itemId, setItemId] = useState("");
  const [jixiaoTypes, setJixiaoTypes] = useState<
    { id: number; name: string }[]
  >([]);
  const [jixiaoTypeId, setJixiaoTypeId] = useState("");
  const [jixiaoPeriod, setJixiaoPeriod] = useState("");
  // 管理员改名（P2-6b）：POST /admin/users/{id}/rename {new_name}
  const [renameOpen, setRenameOpen] = useState(false);
  const [newName, setNewName] = useState("");

  // 打开各操作面板时按需拉取字典/现状（等级字典来自 i18n classList，无需请求）
  useEffect(() => {
    if (panel === "role" && roles.length === 0) {
      api
        .get<RoleItem[]>("/api/v1/admin/roles")
        .then(setRoles)
        .catch(() => {});
    }
    if (panel === "perm") {
      api
        .get<{
          permissions: {
            key: string;
            name: string | null;
            category: string | null;
            descr: string | null;
          }[];
        }>("/api/v1/admin/permission-matrix")
        .then((m) => setPerms(m.permissions ?? []))
        .catch(() => {});
      api
        .get<{
          effective: string[];
          overrides: { permission_key: string; granted: boolean }[];
        }>(`/api/v1/admin/user-permissions?user_id=${uid}`)
        .then(setPermData)
        .catch(() => setPermData(null));
    }
    if (panel === "medal" && medals.length === 0) {
      api
        .get<{ id: number; name: string }[]>("/api/v1/medals")
        .then(setMedals)
        .catch(() => {});
    }
    if (panel === "item" && items.length === 0) {
      api
        .get<{ id: number; name: string; kind: string }[]>("/api/v1/shop/items")
        .then(setItems)
        .catch(() => {});
    }
    if (panel === "jixiao" && jixiaoTypes.length === 0) {
      api
        .get<{ id: number; name: string }[]>("/api/v1/jixiao/types")
        .then(setJixiaoTypes)
        .catch(() => {});
    }
  }, [
    panel,
    uid,
    roles.length,
    medals.length,
    items.length,
    jixiaoTypes.length,
  ]);

  return {
    panel,
    setPanel,
    classId,
    setClassId,
    roles,
    setRoles: setRoles as RoleSetter,
    roleKey,
    setRoleKey,
    roleExp,
    setRoleExp,
    newRole,
    setNewRole: setNewRole as NewRoleSetter,
    perms,
    permData,
    permKey,
    setPermKey,
    permGrant,
    setPermGrant,
    medals,
    medalId,
    setMedalId,
    items,
    itemId,
    setItemId,
    jixiaoTypes,
    jixiaoTypeId,
    setJixiaoTypeId,
    jixiaoPeriod,
    setJixiaoPeriod,
    renameOpen,
    setRenameOpen,
    newName,
    setNewName,
  };
}

/** 关联数据 tab（火花流水 / 登录记录 / 做种列表）的翻页与数据拉取。 */
export function useDetailTabs(uid: number) {
  const [tab, setTab] = useState<DetailTab>("profile");
  const [spark, setSpark] = useState<{
    rows: SparkRow[];
    total: number;
  } | null>(null);
  const [sparkPage, setSparkPage] = useState(1);
  const [logins, setLogins] = useState<{
    rows: LoginRow[];
    total: number;
  } | null>(null);
  const [loginsPage, setLoginsPage] = useState(1);
  const [seeds, setSeeds] = useState<SeedRow[] | null>(null);

  useEffect(() => {
    if (tab === "spark") {
      api
        .get<{ rows: SparkRow[]; total: number }>(
          `/api/v1/admin/spark-logs` +
            `?user_id=${uid}&page=${sparkPage}&per_page=${PER_PAGE}`,
        )
        .then(setSpark)
        .catch(() => setSpark(null));
    } else if (tab === "logins") {
      api
        .get<{ rows: LoginRow[]; total: number }>(
          `/api/v1/admin/login-logs` +
            `?user_id=${uid}&page=${loginsPage}&per_page=${PER_PAGE}`,
        )
        .then(setLogins)
        .catch(() => setLogins(null));
    } else if (tab === "seeding" && seeds === null) {
      api
        .get<SeedRow[]>(`/api/v1/admin/users/${uid}/snatches`)
        .then(setSeeds)
        .catch(() => setSeeds([]));
    }
  }, [tab, uid, sparkPage, loginsPage, seeds]);

  return {
    tab,
    setTab,
    spark,
    sparkPage,
    setSparkPage,
    logins,
    loginsPage,
    setLoginsPage,
    seeds,
  };
}
