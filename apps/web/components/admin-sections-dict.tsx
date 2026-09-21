"use client";

/**
 * 维度管理·维度字典与分类归属子面板（从 components/admin-sections.tsx
 * 按域拆出）：维度字典 CRUD 与分类归属模式/自动过审开关表。
 * 数据与动作回调留在父组件注入。
 */

import { api } from "@/lib/api-client";
import type {
  CategoryRow,
  DictRow,
  ModeRow,
  SectionKindMeta,
} from "./admin-sections-shared";
import {
  FIELD_INPUT_CLS,
  MODE_SELECT_CLS,
  SELECT_FIELD_CLS,
} from "./admin-sections-shared";

interface DictPanelProps {
  kind: string;
  setKind: (v: string) => void;
  kinds: SectionKindMeta[];
  dicts: DictRow[];
  dName: string;
  setDName: (v: string) => void;
  dSort: number;
  setDSort: (v: number) => void;
  busy: boolean;
  act: (fn: () => Promise<unknown>, okMsg: string) => Promise<void>;
}

/** 维度字典 */
export function SectionDictPanel(props: DictPanelProps) {
  const { kind, setKind, kinds, dicts, dName, setDName, dSort, setDSort } =
    props;
  const { busy, act } = props;
  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-2 text-base font-bold">维度字典</h2>
      <div className="mb-2 flex flex-wrap items-end gap-2">
        <label className="flex flex-col gap-1 text-xs">
          维度
          <select
            value={kind}
            onChange={(e) => setKind(e.target.value)}
            className={SELECT_FIELD_CLS}
          >
            {kinds.map((k) => (
              <option key={k.kind} value={k.kind}>
                {k.label}
              </option>
            ))}
          </select>
        </label>
        <input
          value={dName}
          onChange={(e) => setDName(e.target.value)}
          placeholder="名称"
          className={`w-36 ${FIELD_INPUT_CLS}`}
        />
        <input
          type="number"
          value={dSort}
          onChange={(e) => setDSort(Number(e.target.value))}
          placeholder="排序"
          className={`w-20 ${FIELD_INPUT_CLS}`}
        />
        <button
          disabled={busy || !dName.trim()}
          className="baozi-button"
          onClick={() =>
            act(async () => {
              await api.post("/api/v1/admin/section-dict", {
                kind,
                name: dName,
                sort: dSort,
              });
              setDName("");
            }, "已添加")
          }
        >
          添加
        </button>
      </div>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">名称</td>
            <td className="colhead">排序</td>
            <td className="colhead text-right">操作</td>
          </tr>
        </thead>
        <tbody>
          {dicts.map((d) => (
            <tr key={d.id}>
              <td className="num">{d.id}</td>
              <td>{d.name}</td>
              <td className="num">{d.sort}</td>
              <td className="text-right">
                <button
                  className="cmgmt-act"
                  disabled={busy}
                  onClick={() => {
                    const nn = window.prompt("新名称", d.name);
                    if (nn && nn !== d.name)
                      act(
                        () =>
                          api.put(`/api/v1/admin/section-dict/${d.id}`, {
                            kind,
                            name: nn,
                            sort: d.sort,
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
                  onClick={() =>
                    act(
                      () =>
                        api.del(
                          `/api/v1/admin/section-dict/${d.id}?kind=${kind}`,
                        ),
                      "已删除",
                    )
                  }
                >
                  删除
                </button>
              </td>
            </tr>
          ))}
          {dicts.length === 0 && (
            <tr>
              <td colSpan={4} className="py-6 text-center text-sub">
                暂无字典项
              </td>
            </tr>
          )}
        </tbody>
      </table>
    </section>
  );
}

interface CatsPanelProps {
  cats: CategoryRow[];
  modes: ModeRow[];
  busy: boolean;
  act: (fn: () => Promise<unknown>, okMsg: string) => Promise<void>;
}

/** 分类归属模式 / 自动过审 */
export function SectionCatsPanel({ cats, modes, busy, act }: CatsPanelProps) {
  return (
    <section className="baozi-panel p-4">
      <h2 className="mb-2 text-base font-bold">分类归属模式 / 自动过审</h2>
      <p className="mb-3 text-xs text-sub">
        「自动过审」打开后，发布到该分类的种子直接过审（Auto Approval Settings
        口径）。
      </p>
      <table className="nexus-table text-xs">
        <thead>
          <tr>
            <td className="colhead">ID</td>
            <td className="colhead">分类</td>
            <td className="colhead">种子数</td>
            <td className="colhead">归属模式</td>
            <td className="colhead">自动过审</td>
          </tr>
        </thead>
        <tbody>
          {cats.map((c) => (
            <tr key={c.id}>
              <td className="num">{c.id}</td>
              <td className="font-bold">{c.name}</td>
              <td className="num">{c.torrents}</td>
              <td>
                <select
                  value={c.mode_id ?? 1}
                  disabled={busy}
                  className={MODE_SELECT_CLS}
                  onChange={(e) =>
                    act(
                      () =>
                        api.put(`/api/v1/admin/categories/${c.id}/flags`, {
                          mode_id: Number(e.target.value),
                        }),
                      "已保存",
                    )
                  }
                >
                  {modes.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.name}
                    </option>
                  ))}
                </select>
              </td>
              <td className="text-center">
                <input
                  type="checkbox"
                  disabled={busy}
                  checked={Boolean(c.auto_approve)}
                  onChange={(e) =>
                    act(
                      () =>
                        api.put(`/api/v1/admin/categories/${c.id}/flags`, {
                          auto_approve: e.target.checked,
                        }),
                      e.target.checked ? "已开启自动过审" : "已关闭自动过审",
                    )
                  }
                />
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}
