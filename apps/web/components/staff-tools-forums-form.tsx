"use client";

import { useState } from "react";
import { api } from "@/lib/api-client";

import type { ForumAdminData } from "./staff-tools-forums";

/** 论坛版块管理·新建/编辑表单（从 components/staff-tools-forums.tsx
 *  按域拆出）：三档门槛 + 受保护 + 分区选择（forummanage 口径）。 */

// 表单输入框 / 数字输入 / 按钮
const INPUT_CLS =
  "min-h-[40px] rounded-[var(--r-sm)] border border-line " +
  "bg-cloud px-3 text-sm outline-none focus:border-sky";
const NUM_INPUT = `${INPUT_CLS} w-24`;
const BTN_SKY =
  "min-h-[40px] rounded-full bg-sky px-5 text-sm font-bold " +
  "text-white disabled:opacity-50";
const BTN_CANCEL =
  "min-h-[40px] rounded-full border border-line px-4 text-sm text-sub";

export function ForumEditForm({
  forumData,
  setForumData,
  busy,
  guard,
  fEditId,
  setFEditId,
}: {
  forumData: ForumAdminData | null;
  setForumData: React.Dispatch<React.SetStateAction<ForumAdminData | null>>;
  busy: boolean;
  guard: (fn: () => Promise<void>, ok: string) => void;
  fEditId: number | null;
  setFEditId: React.Dispatch<React.SetStateAction<number | null>>;
}) {
  const [fName, setFName] = useState("");
  const [fDescr, setFDescr] = useState("");
  const [fMr, setFMr] = useState(0);
  const [fMw, setFMw] = useState(0);
  const [fMc, setFMc] = useState(0);
  const [fProt, setFProt] = useState(false);
  const [fCatId, setFCatId] = useState<number | "">("");

  return (
    <div className="mt-4 flex flex-wrap items-end gap-2">
      <h3 className="w-full text-sm font-bold">
        {fEditId === null ? "新建版块" : `编辑版块 #${fEditId}`}
      </h3>
      <label className="flex flex-col gap-1">
        <span className="text-xs text-sub">名称</span>
        <input
          value={fName}
          onChange={(e) => setFName(e.target.value)}
          className={INPUT_CLS}
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-xs text-sub">描述</span>
        <input
          value={fDescr}
          onChange={(e) => setFDescr(e.target.value)}
          className={INPUT_CLS}
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-xs text-sub">minclassread</span>
        <input
          type="number"
          min={0}
          value={fMr}
          onChange={(e) => setFMr(Number(e.target.value))}
          className={NUM_INPUT}
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-xs text-sub">minclasswrite</span>
        <input
          type="number"
          min={0}
          value={fMw}
          onChange={(e) => setFMw(Number(e.target.value))}
          className={NUM_INPUT}
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-xs text-sub">minclasscreate</span>
        <input
          type="number"
          min={0}
          value={fMc}
          onChange={(e) => setFMc(Number(e.target.value))}
          className={NUM_INPUT}
        />
      </label>
      <label className="flex flex-col gap-1">
        <span className="text-xs text-sub">分区</span>
        <select
          value={fCatId}
          onChange={(e) =>
            setFCatId(e.target.value === "" ? "" : Number(e.target.value))
          }
          className={INPUT_CLS}
        >
          <option value="">未分组</option>
          {(forumData?.categories ?? []).map((c) => (
            <option key={c.id} value={c.id}>
              {c.name}
            </option>
          ))}
        </select>
      </label>
      <label className="flex items-center gap-2 pb-2 text-sm">
        <input
          type="checkbox"
          checked={fProt}
          onChange={(e) => setFProt(e.target.checked)}
        />
        受保护版块
      </label>
      <button
        disabled={busy}
        className={BTN_SKY}
        onClick={() =>
          guard(
            async () => {
              const payload = {
                name: fName.trim(),
                descr: fDescr.trim() || null,
                minclassread: fMr,
                minclasswrite: fMw,
                minclasscreate: fMc,
                protected: fProt,
                category_id: fCatId === "" ? null : Number(fCatId),
              };
              if (fEditId === null) {
                await api.post("/api/v1/admin/forums", payload);
              } else {
                await api.put(`/api/v1/admin/forums/${fEditId}`, payload);
                setFEditId(null);
              }
              setFName("");
              setFDescr("");
              setFMr(0);
              setFMw(0);
              setFMc(0);
              setFProt(false);
              setFCatId("");
              setForumData(await api.get("/api/v1/admin/forums"));
            },
            fEditId === null ? "已创建" : "已保存",
          )
        }
      >
        {fEditId === null ? "创建" : "保存"}
      </button>
      {fEditId !== null && (
        <button
          className={BTN_CANCEL}
          onClick={() => {
            setFEditId(null);
            setFName("");
            setFDescr("");
            setFMr(0);
            setFMw(0);
            setFMc(0);
            setFProt(false);
            setFCatId("");
          }}
        >
          取消
        </button>
      )}
    </div>
  );
}
