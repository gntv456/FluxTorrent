"use client";

import { BTN_SM_BOLD } from "@/lib/ui-classes";

import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

import type { RoleDef } from "./staff-tools-roles";

/** 职务管理面板（从 components/staff-tools-roles.tsx 按域拆出）：
 *  职务字典卡片区 + 新增/编辑表单（sysop：roles.manage；
 *  无权限时后端拒绝，前端如实提示）。 */

export type RoleEdit = {
  key: string;
  name: string;
  descr: string;
  mode: "new" | "edit";
};

// 卡片容器：最小宽度 + 圆角描边浮层底
const ROLE_CARD =
  "min-w-[150px] rounded-[var(--r-md)] border border-line " +
  "bg-[var(--surface-raised)] px-3 py-2";
// 卡片上的小操作按钮（编辑=天蓝 / 删除=警示红）
const ROLE_CARD_BTN_EDIT =
  "min-h-[24px] rounded-full border border-line px-2 " +
  "text-[11px] font-bold text-sky";
const ROLE_CARD_BTN_DEL =
  "min-h-[24px] rounded-full border border-line px-2 " +
  "text-[11px] font-bold text-danger";
// 表单容器 / 表单输入框底色
const ROLE_FORM_BOX = "cmgmt-form rounded-[var(--r-md)] border border-line p-3";
const ROLE_INPUT =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line " +
  "bg-cloud px-2 text-sm";
// 次级描边按钮（新增/编辑表单的取消 / 新增职务虚线按钮共用）
const BTN_OUTLINE_S =
  BTN_SM_BOLD;

export function RoleDictSection({
  roleDefs,
  roleEdit,
  setRoleEdit,
  setRoleDefs,
  busy,
  guard,
}: {
  roleDefs: RoleDef[];
  roleEdit: RoleEdit | null;
  setRoleEdit: React.Dispatch<React.SetStateAction<RoleEdit | null>>;
  setRoleDefs: React.Dispatch<React.SetStateAction<RoleDef[]>>;
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => void;
}) {
  const { dict } = useI18n();
  const t = dict.adminRoles;
  return (
    <div className="mb-4 flex flex-col gap-2">
      <div className="flex flex-wrap gap-2">
        {roleDefs.map((r) => (
          <div key={r.key} className={ROLE_CARD}>
            <p className="text-sm font-bold text-ink">{r.name}</p>
            <p className="mt-0.5 text-xs text-sub">{r.descr}</p>
            <p className="mt-1 flex items-center justify-between">
              <code className="text-[10px] text-sub">{r.key}</code>
              <span className="flex gap-1">
                <button
                  className={ROLE_CARD_BTN_EDIT}
                  title={t.editTip}
                  onClick={() => {
                    setRoleEdit({
                      key: r.key,
                      name: r.name,
                      descr: r.descr ?? "",
                      mode: "edit",
                    });
                  }}
                >
                  {t.editBtn}
                </button>
                <button
                  className={ROLE_CARD_BTN_DEL}
                  title={t.delTip}
                  onClick={() =>
                    guard(async () => {
                      if (!window.confirm(fmt(t.delConfirm, { name: r.name })))
                        return;
                      await api.del(`/api/v1/admin/roles/${r.key}`);
                      setRoleDefs(
                        await api.get<RoleDef[]>("/api/v1/admin/roles"),
                      );
                    }, t.deleted)
                  }
                >
                  {t.delBtn}
                </button>
              </span>
            </p>
          </div>
        ))}
        {roleDefs.length === 0 && (
          <p className="text-xs text-sub">{t.dictEmpty}</p>
        )}
      </div>

      {/* 新增/编辑职务（sysop：roles.manage；无权限时后端拒绝，前端如实提示） */}
      {roleEdit && (
        <div className={ROLE_FORM_BOX}>
          <h3 className="mb-2 text-sm font-bold">
            {roleEdit.mode === "new"
              ? t.newTitle
              : fmt(t.editTitle, { key: roleEdit.key })}
          </h3>
          <div className="flex flex-wrap items-end gap-2">
            {roleEdit.mode === "new" && (
              <label className="flex flex-col gap-1 text-xs">
                {t.fldKey}
                <input
                  value={roleEdit.key}
                  onChange={(e) =>
                    setRoleEdit({ ...roleEdit, key: e.target.value })
                  }
                  placeholder={t.keyPh}
                  className={`${ROLE_INPUT} w-40`}
                />
              </label>
            )}
            <label className="flex flex-col gap-1 text-xs">
              {t.fldName}
              <input
                value={roleEdit.name}
                onChange={(e) =>
                  setRoleEdit({ ...roleEdit, name: e.target.value })
                }
                placeholder={t.namePh}
                className={`${ROLE_INPUT} w-32`}
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {t.fldDescr}
              <input
                value={roleEdit.descr}
                onChange={(e) =>
                  setRoleEdit({ ...roleEdit, descr: e.target.value })
                }
                className={`${ROLE_INPUT} w-48`}
              />
            </label>
            <button
              className="baozi-button"
              disabled={
                busy ||
                !roleEdit.name.trim() ||
                (roleEdit.mode === "new" && !roleEdit.key.trim())
              }
              onClick={() =>
                guard(async () => {
                  const payload = {
                    key: roleEdit.key.trim(),
                    name: roleEdit.name.trim(),
                    descr: roleEdit.descr.trim() || undefined,
                  };
                  if (roleEdit.mode === "new") {
                    await api.post("/api/v1/admin/roles", payload);
                  } else {
                    await api.put(
                      `/api/v1/admin/roles/${encodeURIComponent(roleEdit.key)}`,
                      payload,
                    );
                  }
                  setRoleDefs(await api.get<RoleDef[]>("/api/v1/admin/roles"));
                  setRoleEdit(null);
                }, t.saved)
              }
            >
              {t.saveBtn}
            </button>
            <button className={BTN_OUTLINE_S} onClick={() => setRoleEdit(null)}>
              {t.cancelBtn}
            </button>
          </div>
          {roleEdit.mode === "new" && (
            <p className="mt-2 text-xs text-sub">{t.nextStepHint}</p>
          )}
        </div>
      )}
      {!roleEdit && (
        <button
          className={`${BTN_OUTLINE_S} self-start border-dashed text-sky`}
          onClick={() =>
            setRoleEdit({ key: "", name: "", descr: "", mode: "new" })
          }
        >
          {t.newBtn}
        </button>
      )}
    </div>
  );
}
