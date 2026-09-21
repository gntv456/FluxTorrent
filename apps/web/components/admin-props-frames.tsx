"use client";

import { useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { avatarFrameStyle, FrameImageOverlay } from "@/lib/format";

/** 头像框库面板（从 admin-props.tsx 按域拆出，300 门禁）：
 *  头像框 CRUD（CSS 描边 / 框图链接双形态）表单 + 列表。 */

interface FrameRow {
  id: number;
  name: string;
  css: string;
  image_url: string | null;
  price: number;
  sort: number;
  worn_count: number;
}

export function FramesPanel({
  frames,
  currency,
  load,
}: {
  frames: FrameRow[];
  currency: string;
  load: () => Promise<void>;
}) {
  const emptyFrame = {
    name: "",
    css: "border-color:#2fa878; box-shadow: 0 0 0 2px rgba(47,168,120,.5), 0 0 12px rgba(47,168,120,.85);",
    image: "",
    price: "5000",
    sort: "20",
  };
  const [frameEdit, setFrameEdit] = useState<{
    id: number | null;
    f: typeof emptyFrame;
  }>({ id: null, f: { ...emptyFrame } });
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  const inp =
    "min-h-[40px] rounded-[var(--r-sm)] border border-line bg-cloud px-2 text-sm outline-none focus:border-sky";

  async function saveFrame() {
    setBusy(true);
    try {
      const payload = {
        name: frameEdit.f.name,
        css: frameEdit.f.css,
        image_url: frameEdit.f.image.trim() || null,
        price: Number(frameEdit.f.price) || 0,
        sort: Number(frameEdit.f.sort) || 0,
      };
      if (frameEdit.id === null)
        await api.post("/api/v1/admin/avatar-frames", payload);
      else
        await api.put(`/api/v1/admin/avatar-frames/${frameEdit.id}`, payload);
      flash("头像框已保存");
      setFrameEdit({ id: null, f: { ...emptyFrame } });
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}

      {/* 头像框库 CRUD：双形态——CSS 描边（白名单净化）/ 框图链接（叠层渲染，优先） */}
      <section className="baozi-panel cmgmt-form p-4">
        <h2 className="mb-2 text-base font-bold">
          {frameEdit.id === null ? "新建头像框" : `编辑头像框 #${frameEdit.id}`}
        </h2>
        <div className="flex flex-wrap items-end gap-2">
          <label className="flex flex-col gap-1 text-xs">
            名称
            <input
              value={frameEdit.f.name}
              onChange={(e) =>
                setFrameEdit({
                  ...frameEdit,
                  f: { ...frameEdit.f, name: e.target.value },
                })
              }
              className={`${inp} w-28`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            价格({currency})
            <input
              type="number"
              value={frameEdit.f.price}
              onChange={(e) =>
                setFrameEdit({
                  ...frameEdit,
                  f: { ...frameEdit.f, price: e.target.value },
                })
              }
              className={`${inp} w-24`}
            />
          </label>
          <label className="flex flex-col gap-1 text-xs">
            排序
            <input
              type="number"
              value={frameEdit.f.sort}
              onChange={(e) =>
                setFrameEdit({
                  ...frameEdit,
                  f: { ...frameEdit.f, sort: e.target.value },
                })
              }
              className={`${inp} w-16`}
            />
          </label>
          <label className="flex flex-1 flex-col gap-1 text-xs">
            css（仅 border-color / box-shadow）
            <input
              value={frameEdit.f.css}
              onChange={(e) =>
                setFrameEdit({
                  ...frameEdit,
                  f: { ...frameEdit.f, css: e.target.value },
                })
              }
              className={`${inp} min-w-[220px] font-mono`}
            />
          </label>
          <label className="flex flex-1 flex-col gap-1 text-xs">
            框图链接（可选，PNG/GIF，配置后优先于 css）
            <input
              value={frameEdit.f.image}
              onChange={(e) =>
                setFrameEdit({
                  ...frameEdit,
                  f: { ...frameEdit.f, image: e.target.value },
                })
              }
              placeholder="https://…/frame.png（清空则移除）"
              className={`${inp} min-w-[220px] font-mono`}
            />
          </label>
          {/* 实时预览：与前台同口径——框图叠层优先，否则 CSS 描边 */}
          <span
            className="avatar-frame-demo relative"
            style={
              frameEdit.f.image.trim()
                ? undefined
                : avatarFrameStyle(frameEdit.f.css)
            }
            aria-hidden
          >
            🙂
            <FrameImageOverlay url={frameEdit.f.image.trim() || null} />
          </span>
          <button
            className="baozi-button"
            disabled={busy || !frameEdit.f.name.trim()}
            onClick={saveFrame}
          >
            保存
          </button>
          {frameEdit.id !== null && (
            <button
              className="min-h-[36px] rounded-full border border-line px-4 text-xs font-bold"
              onClick={() => setFrameEdit({ id: null, f: { ...emptyFrame } })}
            >
              取消
            </button>
          )}
        </div>
        <p className="mt-2 text-xs text-sub">
          CSS 款示例色：春 #f4a7bb / 夏 #2fa878 / 秋 #d99419 / 冬
          #8fc3e8；框图需透明底圆形素材。删除框会先自动摘下所有佩戴者。
        </p>
      </section>

      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">预览</td>
            <td className="colhead">名称</td>
            <td className="colhead">价格</td>
            <td className="colhead">排序</td>
            <td className="colhead">佩戴人数</td>
            <td className="colhead">样式</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {frames.map((f) => (
            <tr key={f.id}>
              <td className="num">{f.id}</td>
              <td>
                <span
                  className="avatar-frame-demo relative"
                  style={f.image_url ? undefined : avatarFrameStyle(f.css)}
                  aria-hidden
                >
                  🙂
                  <FrameImageOverlay url={f.image_url} />
                </span>
              </td>
              <td className="font-bold">{f.name}</td>
              <td className="num">{f.price}</td>
              <td className="num">{f.sort}</td>
              <td className="num">{f.worn_count}</td>
              <td className="max-w-[220px] truncate font-mono">
                {f.image_url ? `图: ${f.image_url}` : f.css}
              </td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  onClick={() =>
                    setFrameEdit({
                      id: f.id,
                      f: {
                        name: f.name,
                        css: f.css,
                        image: f.image_url ?? "",
                        price: String(f.price),
                        sort: String(f.sort),
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
                      await api.del(`/api/v1/admin/avatar-frames/${f.id}`);
                      flash("已删除（佩戴者已自动摘下）");
                      await load();
                    } catch (e) {
                      flash(e instanceof ApiError ? e.message : "删除失败");
                    }
                  }}
                >
                  删除
                </button>
              </td>
            </tr>
          ))}
          {frames.length === 0 && (
            <tr>
              <td colSpan={8} className="py-6 text-center text-sub">
                暂无头像框
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </>
  );
}
