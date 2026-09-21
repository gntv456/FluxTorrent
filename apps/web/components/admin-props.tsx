"use client";

import { BTN_SM_BOLD } from "@/lib/ui-classes";

import { useI18n } from "@/i18n/client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { FramesPanel } from "@/components/admin-props-frames";
import { PropsPanel } from "./admin-props-user-bag";
import type {
  EditState,
  FrameRow,
  ShopItemRow,
  UserPropRow,
} from "./admin-props-shared";
import {
  cfgField,
  KIND_FIELDS,
  KIND_LABEL,
  setCfgField,
} from "./admin-props-shared";

/** 第八轮 P2-8：道具管理（好学站 prop/props + prop/user-props 口径）
 *  道具 CRUD（上下架）+ 用户背包浏览与回收；头像框库拆出
 *  admin-props-frames.tsx；类型/常量拆至 ./admin-props-shared.ts；
 *  用户背包拆至 ./admin-props-user-bag.tsx（300 门禁）。 */

const EMPTY_EDIT: EditState = {
  id: null,
  f: {
    name: "",
    kind: "custom_title",
    price: "0",
    config: "{}",
    active: true,
  },
};

/** 圆角描边小按钮 */
const PLAIN_BTN_CLS =
  BTN_SM_BOLD;
/** wide 字段占满一行的附加样式 */
const WIDE_CLS = "flex-1";

export function AdminProps() {
  const { currency } = useI18n();
  const [items, setItems] = useState<ShopItemRow[]>([]);
  const [props, setProps] = useState<UserPropRow[]>([]);
  const [frames, setFrames] = useState<FrameRow[]>([]);
  const [uid, setUid] = useState("");
  const [edit, setEdit] = useState<EditState>(EMPTY_EDIT);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  const load = useCallback(async () => {
    try {
      setItems(await api.get<ShopItemRow[]>("/api/v1/admin/shop-items"));
      const q = uid.trim() ? `?uid=${encodeURIComponent(uid.trim())}` : "";
      // 后端返回分页信封 {rows,total,page,per_page}（旧版是裸数组，双形态兼容）
      const r = await api.get<UserPropRow[] | { rows: UserPropRow[] }>(
        `/api/v1/admin/user-props${q}`,
      );
      setProps(Array.isArray(r) ? r : r.rows);
      setFrames(await api.get<FrameRow[]>("/api/v1/admin/avatar-frames"));
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
  }, [uid]);
  useEffect(() => {
    load();
  }, [load]);

  async function save() {
    let config: unknown;
    try {
      config = JSON.parse(edit.f.config || "{}");
    } catch {
      flash("config 需为合法 JSON");
      return;
    }
    // 装扮类自动补 slot/dressup（结构化字段只管效果键，槽位口径不该让站长手填）
    if (
      [
        "avatar_frame",
        "animated_avatar",
        "rainbow_id",
        "rainbow_name",
      ].includes(edit.f.kind)
    ) {
      const c = (config && typeof config === "object" ? config : {}) as Record<
        string,
        unknown
      >;
      c.dressup = true;
      c.slot =
        edit.f.kind === "rainbow_id" || edit.f.kind === "rainbow_name"
          ? "username"
          : "avatar";
      config = c;
    }
    setBusy(true);
    try {
      const payload = {
        name: edit.f.name,
        kind: edit.f.kind,
        price: Number(edit.f.price) || 0,
        config,
        active: edit.f.active,
      };
      if (edit.id === null) await api.post("/api/v1/admin/shop-items", payload);
      else await api.put(`/api/v1/admin/shop-items/${edit.id}`, payload);
      flash("已保存");
      setEdit(EMPTY_EDIT);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  const inp =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud " +
    "px-2 text-sm outline-none focus:border-sky";

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}

      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold">
          {edit.id === null ? "新建道具" : `编辑道具 #${edit.id}`}
        </h2>
        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1 text-xs">
            名称
            <input
              value={edit.f.name}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, name: e.target.value } })
              }
              className={`${inp} w-36`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            类型
            <select
              value={edit.f.kind}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, kind: e.target.value } })
              }
              className={inp}
            >
              {Object.entries(KIND_LABEL).map(([k, l]) => (
                <option key={k} value={k}>
                  {l.replaceAll("CURRENCY", currency)}（{k}）
                </option>
              ))}
            </select>
          </label>
          <label className="flex flex-col gap-1 text-xs">
            价格({currency})
            <input
              type="number"
              value={edit.f.price}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, price: e.target.value } })
              }
              className={`${inp} w-24`}
            />
          </label>

          {/* —— 效果配置：按类型出结构化字段，自动拼 config（参考 NP Filament 后台的字段式表单） —— */}
          {KIND_FIELDS[edit.f.kind]?.map((fd) => (
            <label
              key={fd.key}
              className={`flex flex-col gap-1 text-xs ${fd.wide ? WIDE_CLS : ""}`.trim()}
            >
              {fd.label.replace("CURRENCY", currency)}
              {fd.options ? (
                <select
                  value={cfgField(edit.f.config)[fd.key] ?? ""}
                  onChange={(e) =>
                    setEdit({
                      ...edit,
                      f: {
                        ...edit.f,
                        config: setCfgField(
                          edit.f.config,
                          fd.key,
                          e.target.value,
                        ),
                      },
                    })
                  }
                  className={`${inp} ${fd.wide ? "min-w-[140px]" : "w-40"}`}
                >
                  <option value="">（不选）</option>
                  {fd.options === "frames"
                    ? frames.map((fr) => (
                        <option key={fr.id} value={String(fr.id)}>
                          {fr.name}（#{fr.id}）
                        </option>
                      ))
                    : fd.options.map(([v, l]) => (
                        <option key={v} value={v}>
                          {l}
                        </option>
                      ))}
                </select>
              ) : (
                <input
                  type={fd.num ? "number" : "text"}
                  value={cfgField(edit.f.config)[fd.key] ?? ""}
                  placeholder={fd.ph ?? ""}
                  onChange={(e) =>
                    setEdit({
                      ...edit,
                      f: {
                        ...edit.f,
                        config: setCfgField(
                          edit.f.config,
                          fd.key,
                          e.target.value,
                        ),
                      },
                    })
                  }
                  className={[
                    inp,
                    fd.wide ? "min-w-[200px]" : "w-36",
                    fd.num ? "" : "font-mono",
                  ]
                    .join(" ")
                    .trim()}
                />
              )}
            </label>
          ))}

          <label className="flex flex-col gap-1 text-xs">
            config(JSON，专家模式可直接改)
            <input
              value={edit.f.config}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, config: e.target.value } })
              }
              className={`${inp} w-52 font-mono`}
            />
          </label>
          <label className="flex items-center gap-1 pb-2 text-xs">
            <input
              type="checkbox"
              checked={edit.f.active}
              onChange={(e) =>
                setEdit({ ...edit, f: { ...edit.f, active: e.target.checked } })
              }
            />
            上架
          </label>
          <button
            className="baozi-button"
            disabled={busy || !edit.f.name.trim()}
            onClick={save}
          >
            保存
          </button>
          {edit.id !== null && (
            <button
              className={PLAIN_BTN_CLS}
              onClick={() => setEdit(EMPTY_EDIT)}
            >
              取消
            </button>
          )}
        </div>
        <p className="mt-2 text-xs text-sub">
          即时生效类（上传量/{currency}
          /邀请）发放直接入账；卡牌/装饰类入背包待用户使用。装扮类会自动补
          slot/dressup，无需手填。
        </p>
      </section>

      <FramesPanel frames={frames} currency={currency} load={load} />

      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">名称</td>
            <td className="colhead">类型</td>
            <td className="colhead">价格</td>
            <td className="colhead">config</td>
            <td className="colhead">状态</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {items.map((it) => (
            <tr key={it.id} className={it.active ? "" : "opacity-50"}>
              <td className="num">{it.id}</td>
              <td className="font-bold">{it.name}</td>
              <td>
                {(KIND_LABEL[it.kind] ?? it.kind).replaceAll(
                  "CURRENCY",
                  currency,
                )}
              </td>
              <td className="num">{it.price}</td>
              <td className="max-w-[220px] truncate font-mono">
                {JSON.stringify(it.config)}
              </td>
              <td>{it.active ? "上架" : "下架"}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() =>
                    setEdit({
                      id: it.id,
                      f: {
                        name: it.name,
                        kind: it.kind,
                        price: String(it.price),
                        config: JSON.stringify(it.config),
                        active: it.active,
                      },
                    })
                  }
                >
                  编辑
                </button>
                <button
                  className="cmgmt-act cmgmt-act--danger"
                  disabled={busy}
                  onClick={async () => {
                    try {
                      const r = await api.del<{
                        deleted?: number;
                        disabled?: boolean;
                      }>(`/api/v1/admin/shop-items/${it.id}`);
                      flash(
                        r?.disabled ? "已有持有记录，已改为下架" : "已删除",
                      );
                      await load();
                    } catch (e) {
                      flash(e instanceof ApiError ? e.message : "删除失败");
                    }
                  }}
                >
                  删除/下架
                </button>
              </td>
            </tr>
          ))}
          {items.length === 0 && (
            <tr>
              <td colSpan={7} className="py-6 text-center text-sub">
                暂无道具
              </td>
            </tr>
          )}
        </tbody>
      </table>

      <PropsPanel
        props={props}
        currency={currency}
        uid={uid}
        setUid={setUid}
        busy={busy}
        flash={flash}
        load={load}
      />
    </div>
  );
}
