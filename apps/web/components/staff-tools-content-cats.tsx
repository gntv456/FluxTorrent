"use client";

/**
 * 内容域·分类管理子面板（从 components/staff-tools-content.tsx
 * 按域拆出）：类型包切换（diff 预览 + replace/merge）+ 分类 CRUD。
 * 数据与动作回调留在父组件注入。
 */

import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import type { CatItem, TypePack } from "./staff-tools-content-shared";

interface CatsPanelProps {
  packs: TypePack[];
  cats: CatItem[];
  curSiteType: string;
  packMode: "replace" | "merge";
  setPackMode: (v: "replace" | "merge") => void;
  catName: string;
  setCatName: (v: string) => void;
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => Promise<void>;
}

/** 类型包切换按钮（激活=橙底白字） */
const PACK_BTN_CLS =
  "min-h-[40px] rounded-full border px-4 text-xs font-bold disabled:opacity-50";
const PACK_ON_CLS =
  "border-[var(--baozi-orange)] bg-[var(--baozi-orange)] text-white";
/** 另存为自定义站型的小按钮 */
const PACK_SAVE_CLS =
  "rounded-full border border-line px-3 text-xs text-sub disabled:opacity-50";
const PACK_OFF_CLS =
  "border-[var(--baozi-orange)] text-[var(--baozi-orange-dark)]";

export function StaffCatsPanel({
  packs,
  cats,
  curSiteType,
  packMode,
  setPackMode,
  catName,
  setCatName,
  busy,
  guard,
}: CatsPanelProps) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  return (
    <>
      <section className="baozi-panel p-4">
        <h2 className="mb-3 text-base font-bold text-ink">{t.packTitle}</h2>
        <p className="mb-3 text-xs text-sub">{t.packNote}</p>
        <div className="mb-2 flex items-center gap-3">
          <span className="text-xs font-bold text-sub">{t.packMode}</span>
          <label className="flex items-center gap-1 text-xs">
            <input
              type="radio"
              checked={packMode === "replace"}
              onChange={() => setPackMode("replace")}
            />
            {t.packModeReplace}
          </label>
          <label className="flex items-center gap-1 text-xs">
            <input
              type="radio"
              checked={packMode === "merge"}
              onChange={() => setPackMode("merge")}
            />
            {t.packModeMerge}
          </label>
        </div>
        <div className="flex flex-wrap gap-2">
          {packs.map((pk) => {
            const active = pk.code === curSiteType;
            return (
              <span key={pk.code} className="inline-flex items-center gap-1">
                <button
                  className={`${PACK_BTN_CLS} ${
                    active ? PACK_ON_CLS : PACK_OFF_CLS
                  }`}
                  disabled={busy}
                  title={pk.description ?? ""}
                  onClick={() => {
                    // U2 §8.2 切换向导：先 diff 预览（旧值→新值），确认后才 apply
                    void (async () => {
                      let lines: string[] = [];
                      try {
                        const d = await api.post<{
                          changes: {
                            key: string;
                            old: string;
                            new: string;
                          }[];
                        }>("/api/v1/admin/site-type-packs/diff", {
                          code: pk.code,
                        });
                        lines = d.changes.map(
                          (c) => `${c.key}: ${c.old} → ${c.new}`,
                        );
                      } catch {
                        /* diff 失败不阻塞——回落旧确认文案 */
                      }
                      const head = fmt(t.packDiffHead, { n: lines.length });
                      const more = lines.length > 15 ? "\n…" : "";
                      const detail = lines.length
                        ? head + lines.slice(0, 15).join("\n") + more
                        : t.packNoDiff;
                      const msg = `${t.packConfirm.replace(
                        "{name}",
                        pk.name,
                      )}\n\n${detail}`;
                      if (!window.confirm(msg)) return;
                      await guard(
                        async () => {
                          await api.post(
                            "/api/v1/admin/site-type-packs/apply",
                            { code: pk.code, mode: packMode },
                          );
                        },
                        t.packApplied.replace("{name}", pk.name),
                      );
                    })();
                  }}
                >
                  {pk.name}
                  {active ? `（${t.packCurrent}）` : ""}
                </button>
                {/* 自定义站型入口（U5 分发）：另存当前配置为新包 */}
                <button
                  className={PACK_SAVE_CLS}
                  disabled={busy}
                  title={t.packSaveTip}
                  onClick={() => {
                    const name = window.prompt(t.packSavePrompt);
                    if (!name?.trim()) return;
                    void guard(async () => {
                      await api.post("/api/v1/admin/site-type-packs/save", {
                        code: `custom_${name
                          .trim()
                          .toLowerCase()
                          .replace(/[^a-z0-9_]+/g, "_")
                          .slice(0, 32)}`,
                        name: name.trim(),
                      });
                    }, t.packSaved);
                  }}
                >
                  💾
                </button>
              </span>
            );
          })}
        </div>
      </section>
      <section className="baozi-panel p-4">
        <h2 className="mb-3 text-base font-bold text-ink">{t.tabCats}</h2>
        <div className="cmgmt-form">
          <label>
            {t.fldCatName}
            <input
              value={catName}
              onChange={(e) => setCatName(e.target.value)}
            />
          </label>
          <button
            className="baozi-button self-start"
            disabled={busy || !catName.trim()}
            onClick={() =>
              guard(async () => {
                await api.post("/api/v1/admin/categories", {
                  name: catName,
                });
                setCatName("");
              }, t.saved)
            }
          >
            {t.btnAddCat}
          </button>
        </div>
        <table className="nexus-table mt-3">
          <tbody>
            <tr>
              <td className="colhead">#</td>
              <td className="colhead">{t.fldCatName}</td>
              <td className="colhead">{t.catTorrents}</td>
              <td className="colhead text-right">{dict.cmgmt.colActions}</td>
            </tr>
            {cats.map((c) => (
              <tr key={c.id}>
                <td className="num">{c.id}</td>
                <td>{c.name}</td>
                <td className="num">{c.torrents}</td>
                <td className="text-right">
                  <button
                    className="cmgmt-act"
                    onClick={() => {
                      const nn = prompt(t.renamePrompt, c.name);
                      if (nn === null || nn === c.name) return;
                      // 图标键（0166）：默认套 film/tv/music/anime/game/app/
                      // book/sport/doc/edu；留空回落分类名首字色块
                      const ik = prompt(t.iconKeyPrompt, c.icon_key ?? "");
                      if (ik === null) return;
                      const bg = prompt(t.bgColorPrompt, c.bg_color ?? "");
                      if (bg === null) return;
                      void guard(async () => {
                        await api.put(`/api/v1/admin/categories/${c.id}`, {
                          name: nn,
                          icon_key: ik.trim(),
                          bg_color: bg.trim(),
                        });
                      }, t.saved);
                    }}
                  >
                    {dict.cmgmt.btnEdit}
                  </button>
                  <button
                    className="cmgmt-act cmgmt-act--danger"
                    onClick={() =>
                      guard(async () => {
                        await api.del(`/api/v1/admin/categories/${c.id}`);
                      }, t.deleted)
                    }
                  >
                    {dict.cmgmt.btnDelete}
                  </button>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>
    </>
  );
}
