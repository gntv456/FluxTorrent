"use client";

import { useState } from "react";
import Link from "next/link";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { fmt } from "@/i18n/config";
import {
  BTN_LINE, INPUT_CLS,
  type ForumAdminForum, type ModTuple,
} from "./forum-structure-types";

const MOD_TAG =
  "mr-1 inline-flex items-center gap-1 rounded-full " +
  "border border-line px-2 py-0.5";

/** 右栏：选中分区下的版块表。
 *  版主任免的输入框按版块隔离（旧版共用一个 state，会把用户任命到错版块）。 */
export function ForumBoardTable({
  forums,
  mods,
  catName,
  busy,
  run,
  onEdit,
  onNew,
}: {
  forums: ForumAdminForum[];
  mods: ModTuple[];
  catName: string;
  busy: boolean;
  run: (fn: () => Promise<void>, ok: string) => void;
  onEdit: (f: ForumAdminForum) => void;
  onNew: () => void;
}) {
  // 每个版块一个草稿值，互不影响
  const [modDraft, setModDraft] = useState<Record<number, string>>({});
  const { dict } = useI18n();
  // 字典取名 t：下面的 map 变量 f 是「版块」，别撞名
  const t = dict.adminForums;

  function addMod(x: ForumAdminForum) {
    const username = (modDraft[x.id] ?? "").trim();
    if (!username) return;
    run(async () => {
      await api.post(`/api/v1/admin/forums/${x.id}/mods`, { username });
      setModDraft((d) => ({ ...d, [x.id]: "" }));
    }, fmt(t.modAdded, { name: username }));
  }

  function removeMod(fid: number, uid: number, uname: string) {
    run(async () => {
      await api.del(`/api/v1/admin/forums/${fid}/mods/${uid}`);
    }, fmt(t.modRemoved, { name: uname }));
  }

  function removeForum(x: ForumAdminForum) {
    // 后端对非空版块要求显式 force=true（防误删整版内容）
    const msg =
      x.topics > 0
        ? fmt(t.boardDelNonEmpty, { name: x.name, n: x.topics })
        : fmt(t.boardDelEmpty, { name: x.name });
    if (!window.confirm(msg)) return;
    run(async () => {
      const q = x.topics > 0 ? "?force=true" : "";
      await api.del(`/api/v1/admin/forums/${x.id}${q}`);
    }, t.boardDeleted);
  }

  return (
    <div className="min-w-0 flex-1 pl-4">
      <div className="mb-2 flex items-baseline gap-2">
        <b className="text-sm">{catName}</b>
        <span className="text-xs text-sub">
          {fmt(t.boardCount, { n: forums.length })}
        </span>
        <span className="flex-1" />
        <button
          type="button"
          className={`${BTN_LINE} min-h-[30px] text-xs`}
          onClick={onNew}
        >
          {t.newBoardBtn}
        </button>
      </div>

      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">{t.colBoard}</td>
            <td className="colhead w-28">{t.colGates}</td>
            <td className="colhead w-14">{t.colProtect}</td>
            <td className="colhead w-14">{t.colTopics}</td>
            <td className="colhead w-56">{t.colMods}</td>
            <td className="colhead w-28" />
          </tr>
        </thead>
        <tbody>
          {forums.map((f) => {
            const mine = mods.filter((m) => m[0] === f.id);
            return (
              <tr key={f.id}>
                <td>
                  <Link
                    href={`/forums/${f.id}`}
                    className="font-bold text-sky"
                  >
                    {f.name}
                  </Link>
                  {f.descr && <p className="text-sub">{f.descr}</p>}
                </td>
                <td className="num">
                  {f.minclassread} / {f.minclasswrite} / {f.minclasscreate}
                </td>
                <td className="text-center">{f.protected ? "🛡" : "—"}</td>
                <td className="num">{f.topics}</td>
                <td>
                  {mine.map((m) => (
                    <span
                      key={m[1]}
                      className={MOD_TAG}
                    >
                      {m[2]}
                      <button
                        type="button"
                        disabled={busy}
                        className="font-bold text-danger"
                        title={t.modRemoveTip}
                        onClick={() => removeMod(f.id, m[1], m[2])}
                      >
                        ×
                      </button>
                    </span>
                  ))}
                  <span className="inline-flex items-center gap-1">
                    <input
                      value={modDraft[f.id] ?? ""}
                      onChange={(e) =>
                        setModDraft((d) => ({
                          ...d,
                          [f.id]: e.target.value,
                        }))
                      }
                      onKeyDown={(e) => {
                        if (e.key === "Enter") addMod(f);
                      }}
                      placeholder={t.usernamePh}
                      className={`${INPUT_CLS} min-h-[26px] w-24 text-xs`}
                    />
                    <button
                      type="button"
                      disabled={busy || !(modDraft[f.id] ?? "").trim()}
                      className="text-xs font-bold text-sky disabled:opacity-40"
                      onClick={() => addMod(f)}
                    >
                      ＋
                    </button>
                  </span>
                </td>
                <td>
                  <button
                    type="button"
                    disabled={busy}
                    className="font-bold text-sky"
                    onClick={() => onEdit(f)}
                  >
                    {t.editBtn}
                  </button>
                  <button
                    type="button"
                    disabled={busy}
                    className="ml-2 font-bold text-danger"
                    onClick={() => removeForum(f)}
                  >
                    {t.delBtn}
                  </button>
                </td>
              </tr>
            );
          })}
          {forums.length === 0 && (
            <tr>
              <td colSpan={6} className="py-6 text-center text-sub">
                {t.boardEmpty}
              </td>
            </tr>
          )}
        </tbody>
      </table>

      <p className="mt-3 text-xs text-sub">{t.gateNote}</p>
    </div>
  );
}
