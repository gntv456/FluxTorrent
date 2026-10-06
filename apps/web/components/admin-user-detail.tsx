"use client";

import { CELL_CARD_SUB } from "@/lib/ui-classes";

import { useState } from "react";
import { useParams, useRouter } from "next/navigation";
import { dateLocale, fmt } from "@/i18n/config";
import { useI18n } from "@/i18n/client";
import { AdminActions } from "./admin-user-detail-actions";
import { useAdminUserActions } from "./admin-user-detail-actions-hook";
import { AdminUserFieldsPanel } from "./admin-user-fields-panel";
import {
  useAdminPanelDicts,
  useDetailLoad,
  useDetailTabs,
} from "./admin-user-detail-hooks";
import {
  LoginsPanel,
  ProfilePanels,
  SeedingPanel,
  SparkPanel,
} from "./admin-user-detail-panels";
import { statusLabels } from "./admin-user-detail-shared";

/** 后台用户详情（好学站 Filament UserResource 的 user-profile 口径）：
 *  字段全景 + 管理动作 + 关联数据 tab（火花流水 / 登录记录 / 做种列表）。
 *  独立路由取代管理页内的弹层，容纳更丰富的运营操作。
 *  管理操作面板拆至 ./admin-user-detail-actions.tsx；
 *  动作提交域拆至 ./admin-user-detail-actions-hook.ts；
 *  资料全景 / 关联 tab 拆至 ./admin-user-detail-panels.tsx；
 *  类型与常量拆至 ./admin-user-detail-shared.ts；
 *  详情加载 / 操作面板字典 / 关联 tab 拆至 ./admin-user-detail-hooks.ts。 */

// 头部状态徽标的公共底色（含警示红与普通两种）
const BADGE_DANGER = "rounded-full bg-coral/20 px-2 py-0.5 text-xs";
const BADGE_PLAIN = "rounded-full px-2 py-0.5 text-xs";
const BADGE_DANGER_TXT = `${BADGE_DANGER} text-danger`;
// tab 按钮两种形态（选中高亮 / 未选中描边）
const TAB_BTN = "min-h-[40px] rounded-full px-4 text-sm font-bold";
const TAB_BTN_OFF = CELL_CARD_SUB;
// 临时密码提示条样式
const TMP_PASS_CLS =
  "rounded-[var(--r-md)] bg-sky-soft p-3 font-mono text-sm text-ink";

export function AdminUserDetailPage() {
  const params = useParams<{ id: string }>();
  const router = useRouter();
  const { locale, dict, currency } = useI18n();
  const STATUS_LABELS = statusLabels(dict.userDetail.statusLabels);
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
  // 动作提示条优先展示 hook 的 flash 文案，加载兜底用本地 msg
  const toast = actions.msg ?? msg;

  return (
    <div className="flex flex-col gap-3">
      {toast && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {toast}
        </p>
      )}

      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap items-baseline gap-2">
          <h1 className="font-display text-2xl">{d.username}</h1>
          <span className="text-sm text-sub">
            UID {d.id} · {d.class_name ?? `LV${d.class_id}`}
          </span>
          {d.title && (
            <span className={`${BADGE_PLAIN} bg-sun/30`}>{d.title}</span>
          )}
          {d.status > 0 && (
            <span className={BADGE_DANGER_TXT}>
              {STATUS_LABELS[d.status] ?? d.status}
            </span>
          )}
          {d.suspended && (
            <span className={BADGE_DANGER_TXT}>
              {dict.userDetail.badgeSuspended}
            </span>
          )}
          {!d.download_enabled && (
            <span className={BADGE_DANGER_TXT}>
              {dict.userDetail.badgeNoDl}
            </span>
          )}
          {d.parked && (
            <span className={`${BADGE_PLAIN} bg-sky-soft`}>
              {dict.userDetail.badgeParked}
            </span>
          )}
          {d.donor && (
            <span className={`${BADGE_PLAIN} bg-mint/30`}>
              {dict.userDetail.badgeDonor}
            </span>
          )}
        </div>
        <button
          onClick={() => router.push("/admin?tool=users")}
          className="min-h-[36px] rounded-full border border-line px-4 text-xs"
        >
          {dict.userDetail.backToUsers}
        </button>
      </div>

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
            onSubmitRename={actions.submitRename}
            onResetPass={actions.resetPass}
            onToggle={actions.toggle}
            onChangeStatus={actions.changeStatus}
            onDeleteUser={actions.deleteUser}
          />
        </>
      )}

      {tabs.tab === "spark" && (
        <SparkPanel
          spark={tabs.spark}
          page={tabs.sparkPage}
          setPage={tabs.setSparkPage}
          dt={dt}
        />
      )}

      {tabs.tab === "logins" && (
        <LoginsPanel
          logins={tabs.logins}
          page={tabs.loginsPage}
          setPage={tabs.setLoginsPage}
          dt={dt}
        />
      )}

      {tabs.tab === "seeding" && <SeedingPanel seeds={tabs.seeds} />}
    </div>
  );
}
