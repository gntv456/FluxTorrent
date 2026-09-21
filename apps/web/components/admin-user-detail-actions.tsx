"use client";

/**
 * 后台用户详情·管理操作面板（从 components/admin-user-detail.tsx 按域拆出）：
 * AdminActions 管理操作按钮墙 + 展开式管理表单。状态留在
 * admin-user-detail.tsx，经 props 全量注入（受控展示，动作回调上抛）。
 * 展开表单域拆至 ./admin-user-detail-actions-forms.tsx。
 */

import type { Detail } from "./admin-user-detail-shared";
import {
  AdminActionPanels,
  TAB_BTN_CLS,
} from "./admin-user-detail-actions-forms";

/** 面板切换按钮（激活/默认）样式 */
const ON_CLS = "bg-sky text-white";
const OFF_CLS = "border border-line";

/** 圆角描边小按钮（普通/danger 文案用） */
const PLAIN_BTN_CLS =
  "min-h-[36px] rounded-full border border-line px-4 text-xs font-bold";
const DANGER_BTN_CLS = `${PLAIN_BTN_CLS} text-danger`;

/** 下载/挂起切换按钮的两种态样式 */
const DANGER_CLS = "border border-line text-danger";
const MINT_CLS = "bg-mint text-white";
/** 下载开关按钮整串（避免行内模板超宽） */
const DL_BTN_CLS = `${TAB_BTN_CLS}${DANGER_CLS}`.trim();

/** 圆角实底小按钮（薄荷绿/珊瑚红） */
const SOLID_MINT_CLS =
  "min-h-[36px] rounded-full bg-mint px-4 text-xs font-bold text-white";
const SOLID_CORAL_CLS =
  "min-h-[36px] rounded-full bg-coral px-4 text-xs font-bold text-white";

export interface AdminActionsProps {
  d: Detail;
  busy: boolean;
  currency: string;
  classList: [number, string][];
  dict: {
    adminrename: Record<string, string>;
  };
  panel: "" | "class" | "role" | "perm" | "medal" | "item" | "jixiao";
  setPanel: (
    p: "" | "class" | "role" | "perm" | "medal" | "item" | "jixiao",
  ) => void;
  adjust: boolean;
  setAdjust: (v: boolean) => void;
  adj: {
    up: string;
    down: string;
    spark: string;
    invite: string;
    note: string;
  };
  setAdj: (v: {
    up: string;
    down: string;
    spark: string;
    invite: string;
    note: string;
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
  const { d, busy, dict, panel, setPanel, adjust, setAdjust } = props;
  return (
    <section className="baozi-panel flex flex-col gap-3 p-4">
      <h2 className="text-base font-bold text-ink">管理操作</h2>
      <div className="flex flex-wrap gap-2">
        <button
          className="baozi-button"
          onClick={() => {
            setAdjust(!adjust);
            setPanel("");
          }}
        >
          修改上传量等
        </button>
        <button
          className={`${TAB_BTN_CLS}${panel === "class" ? ON_CLS : OFF_CLS}`}
          onClick={() => {
            setPanel(panel === "class" ? "" : "class");
            setAdjust(false);
          }}
        >
          等级修改
        </button>
        <button
          className={`${TAB_BTN_CLS}${panel === "role" ? ON_CLS : OFF_CLS}`}
          onClick={() => {
            setPanel(panel === "role" ? "" : "role");
            setAdjust(false);
          }}
        >
          分配角色
        </button>
        <button
          className={`${TAB_BTN_CLS}${panel === "perm" ? ON_CLS : OFF_CLS}`}
          onClick={() => {
            setPanel(panel === "perm" ? "" : "perm");
            setAdjust(false);
          }}
        >
          分配权限
        </button>
        <button
          className={`${TAB_BTN_CLS}${panel === "medal" ? ON_CLS : OFF_CLS}`}
          onClick={() => {
            setPanel(panel === "medal" ? "" : "medal");
            setAdjust(false);
          }}
        >
          授予勋章
        </button>
        <button
          className={`${TAB_BTN_CLS}${panel === "item" ? ON_CLS : OFF_CLS}`}
          onClick={() => {
            setPanel(panel === "item" ? "" : "item");
            setAdjust(false);
          }}
        >
          授予道具
        </button>
        <button
          className={`${TAB_BTN_CLS}${panel === "jixiao" ? ON_CLS : OFF_CLS}`}
          onClick={() => {
            setPanel(panel === "jixiao" ? "" : "jixiao");
            setAdjust(false);
          }}
        >
          分配考核
        </button>
        <button
          className={`${TAB_BTN_CLS}${props.renameOpen ? ON_CLS : OFF_CLS}`}
          onClick={() => {
            props.setRenameOpen(!props.renameOpen);
            setPanel("");
            setAdjust(false);
          }}
        >
          {dict.adminrename.btn}
        </button>
        <button
          disabled={busy}
          onClick={props.onResetPass}
          className={PLAIN_BTN_CLS}
        >
          重置密码
        </button>
        <button
          disabled={busy}
          onClick={() => props.onToggle("download_enabled")}
          className={
            d.download_enabled ? DL_BTN_CLS : `${TAB_BTN_CLS}${MINT_CLS}`
          }
        >
          {d.download_enabled ? "禁用下载权限" : "恢复下载权限"}
        </button>
        <button
          disabled={busy}
          onClick={() => props.onToggle("suspended")}
          className={`${TAB_BTN_CLS}${d.suspended ? MINT_CLS : DANGER_CLS}`}
        >
          {d.suspended ? "解除挂起" : "挂起账号"}
        </button>
        {d.status === 1 && (
          <button
            disabled={busy}
            onClick={() => props.onChangeStatus(0)}
            className={SOLID_MINT_CLS}
          >
            解除禁言
          </button>
        )}
        {d.status === 2 && (
          <button
            disabled={busy}
            onClick={() => props.onChangeStatus(0)}
            className={SOLID_MINT_CLS}
          >
            解除封禁
          </button>
        )}
        {d.status === 0 && (
          <button
            disabled={busy}
            onClick={() => props.onChangeStatus(1)}
            className={DANGER_BTN_CLS}
          >
            禁言
          </button>
        )}
        {d.status < 2 && (
          <button
            disabled={busy}
            onClick={() => props.onChangeStatus(2)}
            className={DANGER_BTN_CLS}
          >
            封禁
          </button>
        )}
        {d.status >= 2 && (
          <button
            disabled={busy}
            onClick={props.onDeleteUser}
            className={SOLID_CORAL_CLS}
          >
            删除用户
          </button>
        )}
      </div>

      <AdminActionPanels {...props} />
    </section>
  );
}
