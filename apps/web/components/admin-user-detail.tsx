"use client";

import { CELL_CARD_SUB } from "@/lib/ui-classes";

import { useState } from "react";
import { useParams } from "next/navigation";
import { dateLocale, fmt } from "@/i18n/config";
import { useI18n } from "@/i18n/client";
import { AdminActions } from "./admin-user-detail-actions";
import { useAdminUserActions } from "./admin-user-detail-actions-hook";
import { UserDetailDialogs, type AskAct } from "./admin-user-detail-dialogs";
import { AdminUserFieldsPanel } from "./admin-user-fields-panel";
import {
  useAdminPanelDicts,
  useDetailLoad,
  useDetailTabs,
} from "./admin-user-detail-hooks";
import { UserDetailHero } from "./admin-user-detail-hero";
import { ProfilePanels } from "./admin-user-detail-panels";
import { DetailTabsBody } from "./admin-user-detail-tabs";

/** 后台用户详情（同类站 Filament UserResource 的 user-profile 口径）：
 *  字段全景 + 管理动作 + 关联数据 tab（火花流水 / 登录记录 / 做种列表）。
 *  独立路由取代管理页内的弹层，容纳更丰富的运营操作。
 *  管理操作面板拆至 ./admin-user-detail-actions.tsx；
 *  动作提交域拆至 ./admin-user-detail-actions-hook.ts；
 *  资料全景 / 关联 tab 拆至 ./admin-user-detail-panels.tsx；
 *  类型与常量拆至 ./admin-user-detail-shared.ts；
 *  详情加载 / 操作面板字典 / 关联 tab 拆至 ./admin-user-detail-hooks.ts。 */

// tab 按钮两种形态（选中高亮 / 未选中描边）
const TAB_BTN = "min-h-[40px] rounded-full px-4 text-sm font-bold";
const TAB_BTN_OFF = CELL_CARD_SUB;
// 临时密码提示条样式
const TMP_PASS_CLS =
  "rounded-[var(--r-md)] bg-sky-soft p-3 font-mono text-sm text-ink";

export function AdminUserDetailPage() {
  const params = useParams<{ id: string }>();
  const { locale, dict, currency } = useI18n();
  const classList = (dict.admin as unknown as { classList: [number, string][] })
    .classList;
  const uid = Number(params.id);
  const [adjust, setAdjust] = useState(false);
  const [adj, setAdj] = useState({
    up: "0",
    down: "0",
    spark: "0",
    invite: "0",
    note: "",
    idem: "",
  });

  const { d, msg, load } = useDetailLoad(uid);
  const [askStatus, setAskStatus] = useState<0 | 1 | 2 | null>(null);
  const [statusReason, setStatusReason] = useState("");
  const [askAct, setAskAct] = useState<AskAct | null>(null);
  const ud = dict.userDetail;
  const dicts = useAdminPanelDicts(uid);
  const tabs = useDetailTabs(uid);

  const actions = useAdminUserActions({
    uid,
    d,
    values: {
      classId: dicts.classId,
      roleKey: dicts.roleKey,
      roleExp: dicts.roleExp,
      newRole: dicts.newRole,
      permKey: dicts.permKey,
      permGrant: dicts.permGrant,
      medalId: dicts.medalId,
      medalDays: dicts.medalDays,
      itemId: dicts.itemId,
      jixiaoTypeId: dicts.jixiaoTypeId,
      jixiaoPeriod: dicts.jixiaoPeriod,
      newName: dicts.newName,
      adj,
    },
    load,
    setPanel: dicts.setPanel,
    setAdjust,
    setRenameOpen: dicts.setRenameOpen,
    setNewName: dicts.setNewName,
    setNewRole: dicts.setNewRole,
    setRoles: dicts.setRoles,
  });

  if (!d)
    return (
      <p className="py-8 text-center text-sub">
        {msg ?? dict.userDetail.loading}
      </p>
    );

  const dt = (s: string | null) =>
    s ? new Date(s).toLocaleString(dateLocale(locale)) : "—";
  return (
    <div className="flex flex-col gap-3">
      {/* 资料 Hero（对齐样图 § 用户详情）：组件本体拆至
          ./admin-user-detail-hero.tsx（300 行门禁）。 */}
      <UserDetailHero d={d} currency={currency} dt={dt} />

      {/* 关联数据 tab */}
      <div className="flex flex-wrap gap-2" role="tablist">
        {(
          [
            ["profile", dict.userDetail.tabProfile],
            ["spark", fmt(dict.userDetail.tabSpark, { magic: currency })],
            ["logins", dict.userDetail.tabLogins],
            ["seeding", dict.userDetail.tabSeeding],
          ] as const
        ).map(([k, label]) => (
          <button
            key={k}
            role="tab"
            aria-selected={tabs.tab === k}
            onClick={() => tabs.setTab(k)}
            className={`${TAB_BTN} ${
              tabs.tab === k ? "bg-sky text-white" : TAB_BTN_OFF
            }`}
          >
            {label}
          </button>
        ))}
      </div>

      {tabs.tab === "profile" && (
        <>
          <ProfilePanels d={d} currency={currency} dt={dt} />
          {/* 自定义字段值（G4）：运营侧查看/代改；站长没建字段时组件自身不渲染 */}
          <AdminUserFieldsPanel userId={d.id} />
          {actions.tmpPass && (
            <p className={TMP_PASS_CLS}>
              {fmt(dict.userDetail.tmpPass, { pass: actions.tmpPass })}
            </p>
          )}
          <AdminActions
            d={d}
            busy={actions.busy}
            currency={currency}
            classList={classList}
            dict={{
              adminrename: dict.adminrename as unknown as Record<
                string,
                string
              >,
            }}
            panel={dicts.panel}
            setPanel={dicts.setPanel}
            adjust={adjust}
            setAdjust={setAdjust}
            adj={adj}
            setAdj={setAdj}
            classId={dicts.classId}
            setClassId={dicts.setClassId}
            roles={dicts.roles}
            roleKey={dicts.roleKey}
            setRoleKey={dicts.setRoleKey}
            roleExp={dicts.roleExp}
            setRoleExp={dicts.setRoleExp}
            newRole={dicts.newRole}
            setNewRole={dicts.setNewRole}
            perms={dicts.perms}
            permKey={dicts.permKey}
            setPermKey={dicts.setPermKey}
            permGrant={dicts.permGrant}
            setPermGrant={dicts.setPermGrant}
            permData={dicts.permData}
            medals={dicts.medals}
            medalId={dicts.medalId}
            medalDays={dicts.medalDays}
            setMedalDays={dicts.setMedalDays}
            setMedalId={dicts.setMedalId}
            items={dicts.items}
            itemId={dicts.itemId}
            setItemId={dicts.setItemId}
            jixiaoTypes={dicts.jixiaoTypes}
            jixiaoTypeId={dicts.jixiaoTypeId}
            setJixiaoTypeId={dicts.setJixiaoTypeId}
            jixiaoPeriod={dicts.jixiaoPeriod}
            setJixiaoPeriod={dicts.setJixiaoPeriod}
            renameOpen={dicts.renameOpen}
            setRenameOpen={dicts.setRenameOpen}
            newName={dicts.newName}
            setNewName={dicts.setNewName}
            onSubmitAdjust={actions.submitAdjust}
            onSubmitClass={actions.submitClass}
            onSubmitRole={actions.submitRole}
            onSubmitPerm={actions.submitPerm}
            onSubmitMedal={actions.submitMedal}
            onSubmitItem={actions.submitItem}
            onSubmitJixiao={actions.submitJixiao}
            onSubmitNewRole={actions.submitNewRole}
            onSubmitRename={() => setAskAct("rename")}
            onResetPass={() => setAskAct("resetPass")}
            onToggle={actions.toggle}
            onChangeStatus={(next) => {
              if (next > 0) {
                setStatusReason("");
                setAskStatus(next as 1 | 2);
              } else actions.changeStatus(0);
            }}
            onDeleteUser={() => setAskAct("deleteUser")}
          />
        </>
      )}

      <DetailTabsBody tabs={tabs} dt={dt} />
      <UserDetailDialogs
        askStatus={askStatus}
        setAskStatus={setAskStatus}
        statusReason={statusReason}
        setStatusReason={setStatusReason}
        askAct={askAct}
        setAskAct={setAskAct}
        username={d?.username ?? ""}
        newName={dicts.newName}
        uid={uid}
        onStatus={(next, reason) => actions.changeStatus(next, reason)}
        onAct={(act) => {
          if (act === "rename") actions.submitRename();
          else if (act === "resetPass") actions.resetPass();
          else actions.deleteUser();
        }}
      />
    </div>
  );
}
