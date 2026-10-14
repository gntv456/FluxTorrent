"use client";

/**
 * 内容域·分类管理子面板（从 components/staff-tools-content.tsx
 * 按域拆出）：类型包切换（diff 预览 + replace/merge）+ 分类 CRUD。
 * 数据与动作回调留在父组件注入。
 */

import { useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { StaffCatEditDialog } from "./staff-tools-cat-edit";
import { StaffPackAppliesPanel } from "./staff-tools-pack-applies";
import type {
  CatItem,
  PackApplyItem,
  TypePack,
} from "./staff-tools-content-shared";

interface CatsPanelProps {
  packs: TypePack[];
  cats: CatItem[];
  applies: PackApplyItem[];
  curSiteType: string;
  packMode: "replace" | "merge";
  setPackMode: (v: "replace" | "merge") => void;
  catName: string;
  setCatName: (v: string) => void;
  catParent: number | "";
  setCatParent: (v: number | "") => void;
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => Promise<void>;
}

/** 类型包切换按钮（激活=橙底白字） */
const PACK_BTN_CLS =
  "min-h-[40px] rounded-full border px-4 text-xs font-bold disabled:opacity-50";
const PACK_ON_CLS =
  "border-[var(--brand-orange)] bg-[var(--brand-orange)] text-white";
/** 另存为自定义站型的小按钮 */
const PACK_SAVE_CLS =
  "rounded-full border border-line px-3 text-xs text-sub disabled:opacity-50";
const PACK_OFF_CLS =
  "border-[var(--brand-orange)] text-[var(--brand-orange-dark)]";

export function StaffCatsPanel({
  packs,
  cats,
  applies,
  curSiteType,
  packMode,
  setPackMode,
  catName,
  setCatName,
  catParent,
  setCatParent,
  busy,
  guard,
}: CatsPanelProps) {
  const { dict } = useI18n();
  const t = dict.stafftools;
  // P2 清偿：切包确认 / 另存为自定义包共用一个 ConfirmDialog 状态机
  //（旧 window.confirm/prompt 退役；diff 预览文本放 children 白名单换行）
  const [ask, setAsk] = useState<
    | { kind: "apply"; code: string; name: string; msg: string }
    | { kind: "save" }
    | null
  >(null);
  const [saveName, setSaveName] = useState("");
  // P2 清偿：分类编辑（旧 6 连 prompt + alert）收敛到
  // StaffCatEditDialog 单对话框六字段草稿（discriminated: editing=行引用）
  const [editing, setEditing] = useState<CatItem | null>(null);
  return (
    <>
      <section className="brand-panel p-4">
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
                    // U2 §8.2 切换向导：先 diff 预览（旧值→新值），确认后才 apply。
                    // 0317 H2：预览含分类段（改名带在用种子数/新增/失活），
                    // 站长在点应用前就能看到分类语义将被如何动。
                    void (async () => {
                      let lines: string[] = [];
                      try {
                        const d = await api.post<{
                          changes: {
                            key: string;
                            old?: string;
                            new?: string;
                            torrents?: number;
                          }[];
                        }>("/api/v1/admin/site-type-packs/diff", {
                          code: pk.code,
                        });
                        lines = d.changes.map((c) => {
                          if (c.key === "category_rename") {
                            return `${c.old} → ${c.new}（在用种子 ${c.torrents ?? 0} 枚，分类将改名）`;
                          }
                          if (c.key === "category_new") {
                            return `新增分类：${c.new}`;
                          }
                          if (c.key === "category_inactive") {
                            return `${c.old}（在用种子 ${c.torrents ?? 0} 枚，本包不含——将保留但不再由包维护）`;
                          }
                          return `${c.key}: ${c.old ?? "(未设置)"} → ${c.new}`;
                        });
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
                      setAsk({
                        kind: "apply",
                        code: pk.code,
                        name: pk.name,
                        msg,
                      });
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
                    setSaveName("");
                    setAsk({ kind: "save" });
                  }}
                >
                  💾
                </button>
              </span>
            );
          })}
        </div>
      </section>
      <ConfirmDialog
        open={ask !== null}
        onClose={() => setAsk(null)}
        onConfirm={() => {
          const cur = ask;
          setAsk(null);
          if (cur?.kind === "apply") {
            void guard(
              async () => {
                await api.post("/api/v1/admin/site-type-packs/apply", {
                  code: cur.code,
                  mode: packMode,
                });
              },
              t.packApplied.replace("{name}", cur.name),
            );
          } else if (cur?.kind === "save" && saveName.trim()) {
            void guard(async () => {
              await api.post("/api/v1/admin/site-type-packs/save", {
                code: `custom_${saveName
                  .trim()
                  .toLowerCase()
                  .replace(/[^a-z0-9_]+/g, "_")
                  .slice(0, 32)}`,
                name: saveName.trim(),
              });
            }, t.packSaved);
          }
        }}
        tone={ask?.kind === "apply" ? "danger" : "neutral"}
        title={ask?.kind === "save" ? t.packSavePrompt : t.packTitle}
        busy={busy}
      >
        {ask?.kind === "apply" ? (
          <p className="whitespace-pre-line text-sm text-muted">{ask.msg}</p>
        ) : ask?.kind === "save" ? (
          <label className="flex flex-col gap-1 text-sm">
            <span>{t.packSavePrompt}</span>
            <input
              value={saveName}
              onChange={(e) => setSaveName(e.target.value)}
              maxLength={32}
              className="min-h-[40px] rounded-[var(--r-sm)] border
                border-line bg-[var(--surface-card)] px-3 text-sm"
              data-autofocus
            />
          </label>
        ) : null}
      </ConfirmDialog>
      <section className="brand-panel p-4">
        <h2 className="mb-3 text-base font-bold text-ink">{t.tabCats}</h2>
        <div className="cmgmt-form">
          <label>
            {t.fldCatName}
            <input
              value={catName}
              onChange={(e) => setCatName(e.target.value)}
            />
          </label>
          <label>
            {dict.cmgmt.parentLabel}
            <select
              value={catParent}
              onChange={(e) =>
                setCatParent(
                  e.target.value === "" ? "" : Number(e.target.value),
                )
              }
            >
              <option value="">{dict.cmgmt.parentTop}</option>
              {cats.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>
          </label>
          <button
            className="brand-button self-start"
            disabled={busy || !catName.trim()}
            onClick={() =>
              guard(async () => {
                await api.post("/api/v1/admin/categories", {
                  name: catName,
                  parent_id: catParent === "" ? null : catParent,
                });
                setCatName("");
                setCatParent("");
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
              <td className="colhead">{t.fldCatSort}</td>
              <td className="colhead">{t.colNewznab}</td>
              <td className="colhead">{t.catTorrents}</td>
              <td className="colhead text-right">{dict.cmgmt.colActions}</td>
            </tr>
            {cats.map((c) => (
              <tr key={c.id}>
                <td className="num">{c.id}</td>
                <td>
                  {c.parent_id
                    ? `${cats.find((x) => x.id === c.parent_id)?.name ?? "#" + c.parent_id} › ${c.name}`
                    : c.name}
                </td>
                <td className="num">{c.sort}</td>
                <td className="num">{c.newznab_id ?? "—"}</td>
                <td className="num">{c.torrents}</td>
                <td className="text-right">
                  <button
                    className="cmgmt-act"
                    onClick={() => setEditing(c)}
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
      <StaffPackAppliesPanel rows={applies} busy={busy} guard={guard} />
      {editing && (
        <StaffCatEditDialog
          cat={editing}
          onClose={() => setEditing(null)}
          busy={busy}
          guard={guard}
        />
      )}
    </>
  );
}
