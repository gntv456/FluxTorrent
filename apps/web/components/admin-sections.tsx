"use client";

import { useCallback, useEffect, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { SectionCatsPanel, SectionDictPanel } from "./admin-sections-dict";
import type {
  CategoryRow,
  DictRow,
  ModeRow,
  SectionKindMeta,
} from "./admin-sections-shared";
import {
  FALLBACK_KINDS,
  FIELD_INPUT_CLS,
  FLAGS,
} from "./admin-sections-shared";

/** 第八轮 P3-12：维度管理（好学站 Section 组口径）
 *  分类模式（维度开关 + 归属分类 + 自动过审）+ 七维字典 CRUD。
 *  字典/分类归属拆至 ./admin-sections-dict.tsx；
 *  类型与常量拆至 ./admin-sections-shared.ts。 */

export function AdminSections() {
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
      setKinds((pub.kinds as SectionKindMeta[] | undefined) ?? FALLBACK_KINDS);
      const counts: Record<string, number> = {};
      for (const [k, v] of Object.entries(pub)) {
        if (k !== "kinds" && k !== "modes" && Array.isArray(v))
          counts[k] = v.length;
      }
      setKindCounts(counts);
    } catch (e) {
      flash(e instanceof ApiError ? e.message : "加载失败");
    }
  }, [kind]);
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
      flash(e instanceof ApiError ? e.message : "操作失败");
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="flex flex-col gap-3">
      {msg && (
        <p className="rounded-[var(--r-md)] bg-sky-soft p-3 text-sm text-ink">
          {msg}
        </p>
      )}

      {/* 分类模式 */}
      <section className="baozi-panel p-4">
        <h2 className="mb-2 text-base font-bold">分类模式</h2>
        <p className="mb-3 text-xs text-sub">
          每个分类归属一个模式；模式内的开关决定发布表单与筛选启用哪些子维度。
        </p>
        <div className="flex flex-wrap items-end gap-2">
          <input
            value={newMode}
            onChange={(e) => setNewMode(e.target.value)}
            placeholder="新模式名称"
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
              }, "已创建")
            }
          >
            新建模式
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
              <td className="colhead">分类数</td>
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
                          "已保存",
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
                          () => api.del(`/api/v1/admin/section-modes/${m.id}`),
                          "已删除（归属分类已回退默认模式）",
                        )
                      }
                    >
                      删除
                    </button>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </section>

      {/* 维度管理（0085：站方自定义质量维度，NP 自定义 Section 口径） */}
      <section className="baozi-panel p-4">
        <h2 className="mb-2 text-base font-bold">维度管理</h2>
        <p className="mb-3 text-xs text-sub">
          维度 = 发布表单「质量」行里的一个下拉框（如
          编码/分辨率/语种）。新增维度后到下方「维度字典」里维护它的选项。
          <b>发布页「质量」与高级搜索「多维筛选」只显示有选项的维度</b>
          ——无选项的维度会自动隐藏，添加选项后立即出现。
        </p>
        <div className="flex flex-wrap items-end gap-2">
          <input
            value={newKind}
            onChange={(e) => setNewKind(e.target.value)}
            placeholder="维度标识（如 resolution）"
            className={`w-48 ${FIELD_INPUT_CLS}`}
          />
          <input
            value={newKindLabel}
            onChange={(e) => setNewKindLabel(e.target.value)}
            placeholder="显示名称（如 分辨率）"
            className={`w-36 ${FIELD_INPUT_CLS}`}
          />
          <button
            disabled={busy || !newKind.trim() || !newKindLabel.trim()}
            className="baozi-button"
            onClick={() =>
              act(async () => {
                await api.post("/api/v1/admin/section-kinds", {
                  kind: newKind,
                  label: newKindLabel,
                });
                setNewKind("");
                setNewKindLabel("");
              }, "维度已创建")
            }
          >
            新建维度
          </button>
        </div>
        <table className="nexus-table mt-3 text-xs">
          <thead>
            <tr>
              <td className="colhead">标识</td>
              <td className="colhead">显示名称</td>
              <td className="colhead">选项数</td>
              <td className="colhead">排序</td>
              <td className="colhead text-right">操作</td>
            </tr>
          </thead>
          <tbody>
            {kinds.map((k) => {
              const n = kindCounts[k.kind] ?? 0;
              return (
                <tr key={k.kind}>
                  <td className="font-mono">{k.kind}</td>
                  <td>{k.label}</td>
                  <td className="num">
                    {n > 0 ? (
                      n
                    ) : (
                      <span className="text-sub">0（发布页/搜索不显示）</span>
                    )}
                  </td>
                  <td className="num">{k.sort}</td>
                  <td className="text-right">
                    <button
                      className="cmgmt-act"
                      disabled={busy}
                      onClick={() => {
                        const nl = window.prompt("新显示名称", k.label);
                        if (nl && nl !== k.label)
                          act(
                            () =>
                              api.put(`/api/v1/admin/section-kinds/${k.kind}`, {
                                kind: k.kind,
                                label: nl,
                                sort: k.sort,
                              }),
                            "已保存",
                          );
                      }}
                    >
                      重命名
                    </button>
                    <button
                      className="cmgmt-act cmgmt-act--danger"
                      disabled={busy}
                      onClick={() => {
                        if (
                          window.confirm(
                            `删除维度「${k.label}」？其下所有字典选项将被清空`,
                          )
                        )
                          act(
                            () =>
                              api.del(`/api/v1/admin/section-kinds/${k.kind}`),
                            "已删除",
                          );
                      }}
                    >
                      删除
                    </button>
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </section>

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
  );
}
