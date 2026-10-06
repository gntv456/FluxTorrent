"use client";

import { BTN_SM_BOLD, INPUT_MD } from "@/lib/ui-classes";

/**
 * 后台用户详情·管理操作面板（从 components/admin-user-detail-actions.tsx
 * 按域拆出）：等级修改 / 分配角色 / 分配权限 / 授予勋章 / 授予道具 /
 * 分配考核 / 管理员改名 / 数值调整等展开表单。状态仍留在父级，
 * 经 props 全量注入（受控展示，动作回调上抛）。
 */

import type { Detail } from "./admin-user-detail-shared";
import {
  categoryLabels,
  itemKindLabels,
} from "./admin-user-detail-shared";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

/** 展开面板卡片容器 */
export const PANEL_BOX_CLS =
  "cmgmt-form rounded-[var(--r-md)] border border-line p-3";
/** 纯描边输入框（datetime/month/number/note） */
export const PLAIN_FIELD_CLS = INPUT_MD;
/** 圆角描边小按钮（普通/danger） */
export const PLAIN_BTN_CLS = BTN_SM_BOLD;
const DANGER_BTN_CLS = `${PLAIN_BTN_CLS} text-danger`;
/** 新增职务折叠框 */
const DETAILS_BOX_CLS =
  "rounded-[var(--r-sm)] border border-dashed border-line p-2";

/** 展开面板共用的圆角切换按钮样式（激活=天蓝、默认=描边） */
export const TAB_BTN_CLS = "min-h-[36px] rounded-full px-4 text-xs font-bold ";
/** 下拉/输入框统一底样式（cmgmt 表单内） */
export const FIELD_CLS =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line " +
  "bg-[var(--surface-card)] px-2";

export interface AdminPanelsProps {
  d: Detail;
  busy: boolean;
  currency: string;
  classList: [number, string][];
  dict: { adminrename: Record<string, string> };
  panel: "" | "class" | "role" | "perm" | "medal" | "item" | "jixiao";
  adjust: boolean;
  setAdjust: (v: boolean) => void;
  adj: {
    up: string;
    down: string;
    spark: string;
    invite: string;
    note: string;
    idem: string;
  };
  setAdj: (v: {
    up: string;
    down: string;
    spark: string;
    invite: string;
    note: string;
    idem: string;
  }) => void;
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
  medalDays: string;
  setMedalDays: (v: string) => void;
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
}

export function AdminActionPanels(props: AdminPanelsProps) {
  const { dict: i18nDict } = useI18n();
  const u = i18nDict.userDetail;
  const CATEGORY_LABEL = categoryLabels(i18nDict.userDetail.categoryLabels);
  const ITEM_KIND_LABEL = itemKindLabels(i18nDict.userDetail.itemKindLabels);
  const { d, busy, currency, classList, dict, panel, adjust, setAdjust } =
    props;
  return (
    <>
      {/* 等级修改 */}
      {panel === "class" && (
        <div className={PANEL_BOX_CLS}>
          <p className="mb-2 text-xs text-sub">
            {fmt(u.classHint, {
              cur: d.class_name ?? `LV${d.class_id}`,
            })}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={props.classId}
              onChange={(e) => props.setClassId(e.target.value)}
              className={FIELD_CLS}
            >
              <option value="">{u.pickClass}</option>
              {classList.map(([id, label]) =>
                id < 99 ? (
                  <option key={id} value={id}>
                    {id} {label}
                  </option>
                ) : null,
              )}
            </select>
            <button
              className="baozi-button"
              disabled={busy || !props.classId}
              onClick={props.onSubmitClass}
            >
              {u.submit}
            </button>
          </div>
        </div>
      )}

      {/* 分配角色 */}
      {panel === "role" && (
        <div className={`${PANEL_BOX_CLS} flex flex-col gap-3`}>
          <p className="text-xs text-sub">{u.roleHint}</p>
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={props.roleKey}
              onChange={(e) => props.setRoleKey(e.target.value)}
              className={FIELD_CLS}
            >
              <option value="">{u.pickRole}</option>
              {props.roles.map((r) => (
                <option key={r.key} value={r.key}>
                  {r.name}
                </option>
              ))}
            </select>
            <input
              type="datetime-local"
              value={props.roleExp}
              onChange={(e) => props.setRoleExp(e.target.value)}
              className={PLAIN_FIELD_CLS}
              title={u.roleExpTitle}
            />
            <button
              className="baozi-button"
              disabled={busy || !props.roleKey}
              onClick={() => props.onSubmitRole(true)}
            >
              {u.grant}
            </button>
            <button
              className={DANGER_BTN_CLS}
              disabled={busy || !props.roleKey}
              onClick={() => props.onSubmitRole(false)}
            >
              {u.revokeRole}
            </button>
          </div>
          {/* 新增职务（sysop）：分配前发现缺角色可直接建 */}
          <details className={DETAILS_BOX_CLS}>
            <summary className="cursor-pointer text-xs font-bold text-sub">
              {u.newRoleSummary}
            </summary>
            <div className="mt-2 flex flex-wrap items-end gap-2">
              <label className="flex flex-col gap-1 text-xs">
                {u.fRoleKey}
                <input
                  value={props.newRole.key}
                  onChange={(e) =>
                    props.setNewRole({ ...props.newRole, key: e.target.value })
                  }
                  placeholder="translator"
                  className={`w-40 ${PLAIN_FIELD_CLS}`}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs">
                {u.fRoleName}
                <input
                  value={props.newRole.name}
                  onChange={(e) =>
                    props.setNewRole({ ...props.newRole, name: e.target.value })
                  }
                  placeholder={u.phRoleName}
                  className={`w-32 ${PLAIN_FIELD_CLS}`}
                />
              </label>
              <label className="flex flex-col gap-1 text-xs">
                {u.fRoleDescr}
                <input
                  value={props.newRole.descr}
                  onChange={(e) =>
                    props.setNewRole({
                      ...props.newRole,
                      descr: e.target.value,
                    })
                  }
                  className={`w-48 ${PLAIN_FIELD_CLS}`}
                />
              </label>
              <button
                className="baozi-button"
                disabled={
                  busy ||
                  !props.newRole.key.trim() ||
                  !props.newRole.name.trim()
                }
                onClick={props.onSubmitNewRole}
              >
                {u.create}
              </button>
            </div>
          </details>
        </div>
      )}
    </>
  );
}
