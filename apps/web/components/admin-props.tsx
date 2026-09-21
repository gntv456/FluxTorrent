"use client";

import { useI18n } from "@/i18n/client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { FramesPanel } from "@/components/admin-props-frames";

/** 第八轮 P2-8：道具管理（好学站 prop/props + prop/user-props 口径）
 *  道具 CRUD（上下架）+ 用户背包浏览与回收；头像框库拆出
 *  admin-props-frames.tsx（300 门禁）。 */

interface ShopItemRow {
  id: number;
  name: string;
  kind: string;
  price: number;
  config: Record<string, unknown>;
  active: boolean;
}

interface UserPropRow {
  order_id: number;
  user_id: number;
  username: string;
  item_id: number;
  item_name: string;
  kind: string;
  price: number;
  created_at: string;
}

interface FrameRow {
  id: number;
  name: string;
  css: string;
  image_url: string | null;
  price: number;
  sort: number;
  worn_count: number;
}

const KIND_LABEL: Record<string, string> = {
  upload_credit: "上传量",
  invite: "邀请",
  temp_invite: "临时邀请",
  gift_spark: "CURRENCY",
  custom_title: "头衔卡",
  rename_card: "改名卡",
  makeup_card: "补签卡",
  rainbow_name: "彩虹名",
  rainbow_id: "彩虹ID",
  avatar_frame: "头像框",
  animated_avatar: "动态头像",
  vip: "VIP",
  app_vip: "APP VIP",
  ad_free: "去广告",
  charity: "公益",
};

/** 每种类型暴露的结构化 config 字段（自动拼装；装扮类的 slot/dressup 在 save 时自动补）。
 *  options: "frames" = 头像框库下拉；否则为 [值, 文案] 数组。wide = 占满一行。 */
interface KindField {
  key: string;
  label: string;
  num?: boolean;
  wide?: boolean;
  ph?: string;
  options?: [string, string][] | "frames";
}

const KIND_FIELDS: Record<string, KindField[]> = {
  upload_credit: [{ key: "gb", label: "上传量(GB)", num: true, ph: "10" }],
  gift_spark: [{ key: "spark", label: `CURRENCY 数量`, num: true, ph: "5000" }],
  charity: [
    { key: "spark", label: "捐赠 CURRENCY 数（空=按价格全额）", num: true },
  ],
  custom_title: [
    { key: "title", label: "头衔文字", wide: true, ph: "种田大户" },
  ],
  avatar_frame: [
    { key: "avatar_url", label: "关联头像框（佩戴时生效）", options: "frames" },
  ],
  animated_avatar: [
    {
      key: "avatar_url",
      label: "头像图片 URL（GIF 动图）",
      wide: true,
      ph: "https://…/cat.gif",
    },
  ],
  vip: [{ key: "days", label: "时长(天)", num: true, ph: "30" }],
  app_vip: [{ key: "days", label: "时长(天)", num: true, ph: "30" }],
  ad_free: [{ key: "days", label: "时长(天)", num: true, ph: "15" }],
  voucher_free: [{ key: "kind", label: "券种", options: [["free", "免费券"]] }],
  voucher_neutral: [
    { key: "kind", label: "券种", options: [["neutral", "中性券"]] },
  ],
  // invite / temp_invite / rename_card / makeup_card / rainbow_* 无额外字段
};

/** config JSON ↔ 结构化字段互转（容错：解析失败返回空对象，不炸表单） */
function cfgField(configJson: string): Record<string, string> {
  try {
    const o = JSON.parse(configJson || "{}");
    return Object.fromEntries(
      Object.entries(o).map(([k, v]) => [k, String(v ?? "")]),
    );
  } catch {
    return {};
  }
}

function setCfgField(configJson: string, key: string, value: string): string {
  const o = cfgField(configJson);
  if (value === "") delete o[key];
  else o[key] = value;
  return JSON.stringify(o);
}

export function AdminProps() {
  const { currency } = useI18n();
  const [items, setItems] = useState<ShopItemRow[]>([]);
  const [props, setProps] = useState<UserPropRow[]>([]);
  const [frames, setFrames] = useState<FrameRow[]>([]);
  const [uid, setUid] = useState("");
  const [edit, setEdit] = useState<{
    id: number | null;
    f: {
      name: string;
      kind: string;
      price: string;
      config: string;
      active: boolean;
    };
  }>({
    id: null,
    f: {
      name: "",
      kind: "custom_title",
      price: "0",
      config: "{}",
      active: true,
    },
  });
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
      setEdit({
        id: null,
        f: {
          name: "",
          kind: "custom_title",
          price: "0",
          config: "{}",
          active: true,
        },
      });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  const inp =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm outline-none focus:border-sky";

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
              className={`flex flex-col gap-1 text-xs ${fd.wide ? "flex-1" : ""}`}
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
                  className={`${inp} ${fd.wide ? "min-w-[200px]" : "w-36"} ${fd.num ? "" : "font-mono"}`}
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
              className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
              onClick={() =>
                setEdit({
                  id: null,
                  f: {
                    name: "",
                    kind: "custom_title",
                    price: "0",
                    config: "{}",
                    active: true,
                  },
                })
              }
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

      <section className="baozi-panel p-4">
        <div className="mb-2 flex items-end gap-2">
          <h3 className="text-sm font-bold">用户背包（购买 + 发放）</h3>
          <input
            value={uid}
            onChange={(e) => setUid(e.target.value)}
            placeholder="按用户 UID 过滤"
            className="min-h-[32px] w-40 rounded-full border border-line px-3 text-xs"
          />
        </div>
        <table className="nexus-table text-xs">
          <thead>
            <tr>
              <td className="colhead">单号</td>
              <td className="colhead">用户</td>
              <td className="colhead">道具</td>
              <td className="colhead">类型</td>
              <td className="colhead">价格</td>
              <td className="colhead">时间</td>
              <td className="colhead text-right">操作</td>
            </tr>
          </thead>
          <tbody>
            {props.map((p) => (
              <tr key={p.order_id}>
                <td className="num">{p.order_id}</td>
                <td>
                  <a
                    href={`/admin/users/${p.user_id}`}
                    className="font-bold text-link"
                  >
                    {p.username}
                  </a>
                </td>
                <td>{p.item_name}</td>
                <td>
                  {(KIND_LABEL[p.kind] ?? p.kind).replaceAll(
                    "CURRENCY",
                    currency,
                  )}
                </td>
                <td className="num">{p.price}</td>
                <td className="text-sub">
                  {new Date(p.created_at).toLocaleString()}
                </td>
                <td className="text-right">
                  {[
                    "upload_credit",
                    "gift_spark",
                    "invite",
                    "temp_invite",
                  ].includes(p.kind) ? (
                    <span className="text-sub">即时生效</span>
                  ) : (
                    <button
                      className="cmgmt-act cmgmt-act--danger"
                      disabled={busy}
                      onClick={async () => {
                        try {
                          await api.del(
                            `/api/v1/admin/user-props/${p.order_id}`,
                          );
                          flash("已回收");
                          await load();
                        } catch (e) {
                          flash(e instanceof ApiError ? e.message : "回收失败");
                        }
                      }}
                    >
                      回收
                    </button>
                  )}
                </td>
              </tr>
            ))}
            {props.length === 0 && (
              <tr>
                <td colSpan={7} className="py-4 text-center text-sub">
                  暂无持有记录
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </section>
    </div>
  );
}
