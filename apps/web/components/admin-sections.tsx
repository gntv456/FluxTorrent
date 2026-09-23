"use client";

import { useCallback, useEffect, useState } from "react";
import { useI18n } from "@/i18n/client";
import { SectionKindsPanel } from "./admin-sections-kinds";
import { api, ApiError } from "@/lib/api-client";
import { SectionCatsPanel, SectionDictPanel } from "./admin-sections-dict";
import type {
  CategoryRow,
  DictRow,
  ModeRow,
  SectionKindMeta,
} from "./admin-sections-shared";
import {
  fallbackKinds,
  FIELD_INPUT_CLS,
  FLAGS,
} from "./admin-sections-shared";

/** 第八轮 P3-12：维度管理（好学站 Section 组口径）
 *  分类模式（维度开关 + 归属分类 + 自动过审）+ 七维字典 CRUD。
 *  字典/分类归属拆至 ./admin-sections-dict.tsx；
 *  类型与常量拆至 ./admin-sections-shared.ts。 */

export function AdminSections() {
  const { dict } = useI18n();
  const at = dict.adminSections;
  const [modes, setModes] = useState<ModeRow[]>([]);
  const [kind, setKind] = useState("codec");
  const [kinds, setKinds] = useState<SectionKindMeta[]>([]);
  const [dicts, setDicts] = useState<DictRow[]>([]);
  const [kindCounts, setKindCounts] = useState<Record<string, number>>({});
  const [cats, setCats] = useState<CategoryRow[]>([]);
  const [newMode, setNewMode] = useState("");
  const [newKind, setNewKind] = useState("");
  const [newKindLabel, setNewKindLabel] = useState("");
  const [dName, setDName] = useState("");
  const [dSort, setDSort] = useState(0);
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const flash = (m: string) => {
    setMsg(m);
    setTimeout(() => setMsg(null), 3000);
  };

  const load = useCallback(async () => {
    try {
      setModes(await api.get<ModeRow[]>("/api/v1/admin/section-modes"));
      setDicts(
        await api.get<DictRow[]>(`/api/v1/admin/section-dict?kind=${kind}`),
      );
      setCats(await api.get<CategoryRow[]>("/api/v1/admin/categories"));
      const pub = await api.get<Record<string, unknown>>(
        "/api/v1/section-dict",
      );
      setKinds(
        (pub.kinds as SectionKindMeta[] | undefined) ??
          fallbackKinds(dict.adminSections.kinds),
      );
      const counts: Record<string, number> = {};
      for (const [k, v] of Object.entries(pub)) {
        if (k !== "kinds" && k !== "modes" && Array.isArray(v))
          counts[k] = v.length;
      }
      setKindCounts(counts);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.loadFail);
    }
  }, [kind, dict.adminSections.kinds]);
  useEffect(() => {
    load();
  }, [load]);

  async function act(fn: () => Promise<unknown>, okMsg: string) {
    setBusy(true);
    try {
      await fn();
      flash(okMsg);
      await load();
    } catch (e) {
      flash(e instanceof ApiError ? e.message : at.opFail);
    } finally {
      setBusy(false);
    }
  }

  return (
    <>
      <SectionKindsPanel
        kinds={kinds}
        kindCounts={kindCounts}
        newKind={newKind}
        setNewKind={setNewKind}
        newKindLabel={newKindLabel}
        setNewKindLabel={setNewKindLabel}
        busy={busy}
        act={act}
      />
      <div className="flex flex-col gap-3">
        {msg && (
          <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
            {msg}
          </p>
        )}

        {/* 分类模式 */}
        <section className="baozi-panel p-4">
          <h2 className="mb-2 text-base font-bold">{at.modesTitle}</h2>
          <p className="mb-3 text-xs text-sub">{at.modesHint}</p>
          <div className="flex flex-wrap items-end gap-2">
            <input
              value={newMode}
              onChange={(e) => setNewMode(e.target.value)}
              placeholder={at.phMode}
              className={`w-40 ${FIELD_INPUT_CLS}`}
            />
            <button
              disabled={busy || !newMode.trim()}
              className="baozi-button"
              onClick={() =>
                act(async () => {
                  await api.post("/api/v1/admin/section-modes", {
                    name: newMode,
                  });
                  setNewMode("");
                }, at.modeCreated)
              }
            >
              {at.createMode}
            </button>
          </div>
          <table className="nexus-table mt-3 text-xs">
            <thead>
              <tr>
                <td className="colhead">ID</td>
                <td className="colhead">名称</td>
                {FLAGS.map(([k, l]) => (
                  <td key={k} className="colhead">
                    {l}
                  </td>
                ))}
                <td className="colhead">{at.thCategories}</td>
                <td className="colhead text-right">操作</td>
              </tr>
            </thead>
            <tbody>
              {modes.map((m) => (
                <tr key={m.id}>
                  <td className="num">{m.id}</td>
                  <td className="font-bold">{m.name}</td>
                  {FLAGS.map(([k]) => (
                    <td key={k} className="text-center">
                      <input
                        type="checkbox"
                        checked={Boolean(m[k])}
                        disabled={busy}
                        onChange={(e) =>
                          act(
                            () =>
                              api.put(`/api/v1/admin/section-modes/${m.id}`, {
                                name: m.name,
                                [k]: e.target.checked,
                              }),
                            at.saved,
                          )
                        }
                      />
                    </td>
                  ))}
                  <td className="num">{m.categories}</td>
                  <td className="text-right">
                    {m.id !== 1 && (
                      <button
                        className="cmgmt-act cmgmt-act--danger"
                        disabled={busy}
                        onClick={() =>
                          act(
                            () =>
                              api.del(`/api/v1/admin/section-modes/${m.id}`),
                            at.modeDeleted,
                          )
                        }
                      >
                        {at.del}
                      </button>
                    )}
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>

        {/* 维度管理（0085：站方自定义质量维度，NP 自定义 Section 口径） */}

        {/* 维度字典（拆至 ./admin-sections-dict.tsx） */}
        <SectionDictPanel
          kind={kind}
          setKind={setKind}
          kinds={kinds}
          dicts={dicts}
          dName={dName}
          setDName={setDName}
          dSort={dSort}
          setDSort={setDSort}
          busy={busy}
          act={act}
        />

        {/* 分类归属/自动过审（拆至 ./admin-sections-dict.tsx） */}
        <SectionCatsPanel cats={cats} modes={modes} busy={busy} act={act} />
      </div>
    </>
  );
}
