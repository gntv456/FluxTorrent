"use client";

import type { AdminPanelsProps } from "./admin-user-detail-actions-forms";
import {
  FIELD_CLS,
  PANEL_BOX_CLS,
  PLAIN_BTN_CLS,
  PLAIN_FIELD_CLS,
} from "./admin-user-detail-actions-forms";
import { CATEGORY_LABEL, ITEM_KIND_LABEL } from "./admin-user-detail-shared";

/** 用户详情操作面板·下半（perm/medal/item/rename/jixiao/adjust）。
 *  从 admin-user-detail-actions-forms.tsx 按域拆出；props 同源共享。 */
export function AdminActionPanelsMore(props: AdminPanelsProps) {
  const { d, busy, currency, dict, panel, adjust, setAdj } = props;
  return (
    <>
      {/* 分配权限 */}
      {panel === "perm" && (
        <div className={`${PANEL_BOX_CLS} flex flex-col gap-2`}>
          <p className="text-xs text-sub">
            覆盖角色判定：授予 = 额外允许；拒绝 = 显式禁止。
          </p>
          {props.permData && (
            <p className="text-xs text-sub">
              生效权限 {props.permData.effective.length} 项
              {props.permData.overrides.length > 0 &&
                `（含 ${props.permData.overrides.length} 项个人覆盖）`}
            </p>
          )}
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={props.permKey}
              onChange={(e) => props.setPermKey(e.target.value)}
              className={`max-w-96 ${FIELD_CLS}`}
            >
              <option value="">选择权限</option>
              {Object.entries(
                props.perms.reduce<Record<string, typeof props.perms>>(
                  (acc, p) => {
                    (acc[p.category ?? "其他"] ??= []).push(p);
                    return acc;
                  },
                  {},
                ),
              ).map(([cat, list]) => (
                <optgroup key={cat} label={CATEGORY_LABEL[cat] ?? cat}>
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
              <option value="">授予/拒绝</option>
              <option value="1">授予（额外允许）</option>
              <option value="0">拒绝（显式禁止）</option>
            </select>
            <button
              className="baozi-button"
              disabled={busy || !props.permKey || !props.permGrant}
              onClick={props.onSubmitPerm}
            >
              提交
            </button>
          </div>
        </div>
      )}

      {/* 授予勋章 */}
      {panel === "medal" && (
        <div className={PANEL_BOX_CLS}>
          <p className="mb-2 text-xs text-sub">
            管理发放（source=admin），已拥有 {d.medals} 枚。
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={props.medalId}
              onChange={(e) => props.setMedalId(e.target.value)}
              className={`max-w-72 ${FIELD_CLS}`}
            >
              <option value="">选择勋章</option>
              {props.medals.map((m) => (
                <option key={m.id} value={m.id}>
                  #{m.id} {m.name}
                </option>
              ))}
            </select>
            <button
              className="baozi-button"
              disabled={busy || !props.medalId}
              onClick={props.onSubmitMedal}
            >
              授予
            </button>
          </div>
        </div>
      )}

      {/* 授予道具（含卡牌/装饰类） */}
      {panel === "item" && (
        <div className={PANEL_BOX_CLS}>
          <p className="mb-2 text-xs text-sub">
            免费发放：上传量/{currency}
            /邀请即时生效；化妆卡、改名卡等卡牌道具入背包待用户使用。
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={props.itemId}
              onChange={(e) => props.setItemId(e.target.value)}
              className={`max-w-80 ${FIELD_CLS}`}
            >
              <option value="">选择道具</option>
              {Object.entries(
                props.items.reduce<Record<string, typeof props.items>>(
                  (acc, it) => {
                    (acc[
                      (ITEM_KIND_LABEL[it.kind] ?? it.kind).replaceAll(
                        "CURRENCY",
                        currency,
                      )
                    ] ??= []).push(it);
                    return acc;
                  },
                  {},
                ),
              ).map(([kind, list]) => (
                <optgroup key={kind} label={kind}>
                  {list.map((it) => (
                    <option key={it.id} value={it.id}>
                      #{it.id} {it.name}
                    </option>
                  ))}
                </optgroup>
              ))}
            </select>
            <button
              className="baozi-button"
              disabled={busy || !props.itemId}
              onClick={props.onSubmitItem}
            >
              发放
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
          <p className="mb-2 text-xs text-sub">
            登记为考核岗位后按系统流水自动核算绩效；期间留空默认当月。
          </p>
          <div className="flex flex-wrap items-center gap-2">
            <select
              value={props.jixiaoTypeId}
              onChange={(e) => props.setJixiaoTypeId(e.target.value)}
              className={FIELD_CLS}
            >
              <option value="">选择考核岗位</option>
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
              title="考核期间（默认当月）"
            />
            <button
              className="baozi-button"
              disabled={busy || !props.jixiaoTypeId}
              onClick={props.onSubmitJixiao}
            >
              登记
            </button>
          </div>
        </div>
      )}
      {adjust && (
        <div className={PANEL_BOX_CLS}>
          <p className="mb-2 text-xs text-sub">
            正数增加、负数减少（下限 0）；邀请正数增发、负数回收。
          </p>
          <div className="grid grid-cols-2 gap-2 md:grid-cols-4">
            <label className="flex flex-col gap-1 text-xs">
              上传量增量（字节）
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
              下载量增量（字节）
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
              {currency}增量
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
              邀请增发/回收
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
            备注（入审计）
            <input
              value={props.adj.note}
              onChange={(e) =>
                props.setAdj({ ...props.adj, note: e.target.value })
              }
              className={PLAIN_FIELD_CLS}
            />
          </label>
          <div className="mt-2 flex gap-2">
            <button
              className="baozi-button"
              disabled={busy}
              onClick={props.onSubmitAdjust}
            >
              提交调整
            </button>
            <button
              className={PLAIN_BTN_CLS}
              onClick={() => props.setAdjust(false)}
            >
              取消
            </button>
          </div>
        </div>
      )}
    </>
  );
}
