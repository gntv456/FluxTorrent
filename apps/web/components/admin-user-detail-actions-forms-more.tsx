"use client";

import type { AdminPanelsProps } from "./admin-user-detail-actions-forms";
import {
  FIELD_CLS, PANEL_BOX_CLS, PLAIN_BTN_CLS, PLAIN_FIELD_CLS,
} from "./admin-user-detail-actions-forms";
import { ItemGroupSelect } from "./admin-user-detail-item-select";
import { categoryLabels, itemKindLabels } from "./admin-user-detail-shared";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";

/** 用户详情操作面板·下半（perm/medal/item/rename/jixiao/adjust）。
 *  从 admin-user-detail-actions-forms.tsx 按域拆出；props 同源共享。 */
export function AdminActionPanelsMore(props: AdminPanelsProps) {
  const { dict: i18nDict } = useI18n();
  const u = i18nDict.userDetail;
  const CL = categoryLabels(i18nDict.userDetail.categoryLabels),
    IKL = itemKindLabels(i18nDict.userDetail.itemKindLabels);
  const { d, busy, currency, dict, panel, adjust, setAdj } = props;
  return (
    <>
      {/* 分配权限 */}
      {panel === "perm" && (
        <div className={`${PANEL_BOX_CLS} flex flex-col gap-2`}>
          <p className="text-xs text-sub">{u.permHint}</p>
          {props.permData && (
            <p className="text-xs text-sub">
              {fmt(u.effectiveCount, { n: props.permData.effective.length })}
              {props.permData.overrides.length > 0 &&
                fmt(u.withOverrides, { n: props.permData.overrides.length })}
            </p>
          )}
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={props.permKey}
              onChange={(e) => props.setPermKey(e.target.value)}
              className={`max-w-96 ${FIELD_CLS}`}
            >
              <option value="">{u.pickPerm}</option>
              {Object.entries(
                props.perms.reduce<Record<string, typeof props.perms>>(
                  (acc, p) => {
                    (acc[p.category ?? u.otherCat] ??= []).push(p);
                    return acc;
                  },
                  {},
                ),
              ).map(([cat, list]) => (
                <optgroup key={cat} label={CL[cat] ?? cat}>
                  {list.map((p) => (
                    <option key={p.key} value={p.key}>
                      {p.name ?? p.key}
                      {p.descr ? ` — ${p.descr}` : ""}
                    </option>
                  ))}
                </optgroup>
              ))}
            </select>
            <select
              value={props.permGrant}
              onChange={(e) =>
                props.setPermGrant(e.target.value as "1" | "0" | "")
              }
              className={FIELD_CLS}
            >
              <option value="">{u.pickGrant}</option>
              <option value="1">{u.grantAllow}</option>
              <option value="0">{u.grantDeny}</option>
            </select>
            <button
              className="baozi-button"
              disabled={busy || !props.permKey || !props.permGrant}
              onClick={props.onSubmitPerm}
            >
              {u.submit}
            </button>
          </div>
        </div>
      )}

      {/* 授予勋章 */}
      {panel === "medal" && (
        <div className={PANEL_BOX_CLS}>
          <p className="mb-2 text-xs text-sub">
            {fmt(u.medalHint, { n: d.medals })}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={props.medalId}
              onChange={(e) => props.setMedalId(e.target.value)}
              className={`max-w-72 ${FIELD_CLS}`}
            >
              <option value="">{u.pickMedal}</option>
              {props.medals.map((m) => (
                <option key={m.id} value={m.id}>
                  #{m.id} {m.name}
                </option>
              ))}
            </select>
            <input
              type="number"
              min={1}
              max={3650}
              value={props.medalDays}
              onChange={(e) => props.setMedalDays(e.target.value)}
              placeholder={u.medalDaysPh}
              title={u.medalDaysPh}
              className={`w-28 ${FIELD_CLS}`}
            />
            <button
              className="baozi-button"
              disabled={busy || !props.medalId}
              onClick={props.onSubmitMedal}
            >
              {u.medalGrant}
            </button>
          </div>
        </div>
      )}

      {/* 授予道具（含卡牌/装饰类） */}
      {panel === "item" && (
        <div className={PANEL_BOX_CLS}>
          <p className="mb-2 text-xs text-sub">
            {fmt(u.itemHint, { magic: currency })}
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <ItemGroupSelect
              items={props.items}
              currency={currency}
              IKL={IKL}
              itemId={props.itemId}
              setItemId={props.setItemId}
              fieldCls={FIELD_CLS}
              emptyLabel={u.pickItem}
            />
            <button
              className="baozi-button"
              disabled={busy || !props.itemId}
              onClick={props.onSubmitItem}
            >
              {u.grantItem}
            </button>
          </div>
        </div>
      )}

      {/* 管理员改名（P2-6b） */}
      {props.renameOpen && (
        <div className={PANEL_BOX_CLS}>
          <p className="mb-2 text-xs text-sub">{dict.adminrename.note}</p>
          <div className="flex flex-wrap items-center gap-2">
            <input
              value={props.newName}
              onChange={(e) => props.setNewName(e.target.value)}
              maxLength={32}
              placeholder={`${d.username} → ?`}
              aria-label={dict.adminrename.new_name}
              className={`w-48 ${FIELD_CLS} px-3 text-sm`}
            />
            <button
              className="baozi-button"
              disabled={busy || !props.newName.trim()}
              onClick={props.onSubmitRename}
            >
              {dict.adminrename.submit}
            </button>
          </div>
        </div>
      )}

      {/* 分配考核 */}
      {panel === "jixiao" && (
        <div className={PANEL_BOX_CLS}>
          <p className="mb-2 text-xs text-sub">{u.jixiaoHint}</p>
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={props.jixiaoTypeId}
              onChange={(e) => props.setJixiaoTypeId(e.target.value)}
              className={FIELD_CLS}
            >
              <option value="">{u.pickJixiao}</option>
              {props.jixiaoTypes.map((j) => (
                <option key={j.id} value={j.id}>
                  #{j.id} {j.name}
                </option>
              ))}
            </select>
            <input
              type="month"
              value={props.jixiaoPeriod}
              onChange={(e) => props.setJixiaoPeriod(e.target.value)}
              className={PLAIN_FIELD_CLS}
              title={u.jixiaoPeriodTitle}
            />
            <button
              className="baozi-button"
              disabled={busy || !props.jixiaoTypeId}
              onClick={props.onSubmitJixiao}
            >
              {u.register}
            </button>
          </div>
        </div>
      )}
      {adjust && (
        <div className={PANEL_BOX_CLS}>
          <p className="mb-2 text-xs text-sub">{u.adjustHint}</p>
          <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
            <label className="flex flex-col gap-1 text-xs">
              {u.adjUp}
              <input
                type="number"
                value={props.adj.up}
                onChange={(e) =>
                  props.setAdj({ ...props.adj, up: e.target.value })
                }
                className={PLAIN_FIELD_CLS}
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {u.adjDown}
              <input
                type="number"
                value={props.adj.down}
                onChange={(e) =>
                  props.setAdj({ ...props.adj, down: e.target.value })
                }
                className={PLAIN_FIELD_CLS}
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {fmt(u.adjSpark, { magic: currency })}
              <input
                type="number"
                value={props.adj.spark}
                onChange={(e) =>
                  props.setAdj({ ...props.adj, spark: e.target.value })
                }
                className={PLAIN_FIELD_CLS}
              />
            </label>
            <label className="flex flex-col gap-1 text-xs">
              {u.adjInvite}
              <input
                type="number"
                value={props.adj.invite}
                onChange={(e) =>
                  props.setAdj({ ...props.adj, invite: e.target.value })
                }
                className={PLAIN_FIELD_CLS}
              />
            </label>
          </div>
          <label className="mt-2 flex flex-col gap-1 text-xs">
            {u.adjNote}
            <input
              value={props.adj.note}
              onChange={(e) =>
                props.setAdj({ ...props.adj, note: e.target.value })
              }
              className={PLAIN_FIELD_CLS}
            />
          </label>
          <label className="mt-2 flex flex-col gap-1 text-xs">
            {u.adjIdem}
            <input
              value={props.adj.idem}
              onChange={(e) =>
                props.setAdj({ ...props.adj, idem: e.target.value })
              }
              placeholder={u.adjIdemPh}
              className={PLAIN_FIELD_CLS}
            />
          </label>
          <div className="mt-2 flex gap-2">
            <button
              className="baozi-button"
              disabled={busy}
              onClick={props.onSubmitAdjust}
            >
              {u.adjustSubmit}
            </button>
            <button
              className={PLAIN_BTN_CLS}
              onClick={() => props.setAdjust(false)}
            >
              {i18nDict.common.cancel}
            </button>
          </div>
        </div>
      )}
    </>
  );
}
