"use client";

import { BTN_SM_BOLD } from "@/lib/ui-classes";
import { PropsForm } from "./admin-props-form";

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
  setCfgField,
} from "./admin-props-shared";

/** 第八轮 P2-8：道具管理（好学站 prop/props + prop/user-props 口径）
 *  道具 CRUD（上下架）+ 用户背包浏览与回收；头像框库拆出
 *  admin-props-frames.tsx；类型/常量拆至 ./admin-props-shared.ts；
 *  用户背包拆至 ./admin-props-user-bag.tsx（300 门禁）。 */

export const EMPTY_EDIT: EditState = {
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
const PLAIN_BTN_CLS = BTN_SM_BOLD;
/** wide 字段占满一行的附加样式 */
const WIDE_CLS = "flex-1";

import { PropsTable, type PropsEditForm } from "./admin-props-table";

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
    <>
      <PropsForm
        edit={edit}
        setEdit={setEdit}
        save={save}
        busy={busy}
        frames={frames}
      />
      <PropsTable
        items={items}
        currency={currency}
        busy={busy}
        onEdit={(id, f) => setEdit({ id, f })}
        onDel={async (id) => {
          try {
            const r = await api.del<{
              deleted?: number;
              disabled?: boolean;
            }>(`/api/v1/admin/shop-items/${id}`);
            flash(r?.disabled ? "已有持有记录，已改为下架" : "已删除");
            await load();
          } catch (e) {
            flash(e instanceof ApiError ? e.message : "删除失败");
          }
        }}
      />

      <div className="flex flex-col gap-3">
        {msg && (
          <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
            {msg}
          </p>
        )}

        <FramesPanel frames={frames} currency={currency} load={load} />

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
    </>
  );
}
