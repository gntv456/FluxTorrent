"use client";

/** 版块管理·介质类型 kinds 区（从 admin-sections.tsx 按域拆出）。 */
import { api } from "@/lib/api-client";
import { type SectionKindMeta, FIELD_INPUT_CLS } from "./admin-sections-shared";

export function SectionKindsPanel(props: {
  kinds: SectionKindMeta[];
  kindCounts: Record<string, number>;
  newKind: string;
  setNewKind: (v: string) => void;
  newKindLabel: string;
  setNewKindLabel: (v: string) => void;
  busy: boolean;
  act: (fn: () => Promise<unknown>, okMsg: string) => void;
}) {
  const {
    kinds,
    kindCounts,
    newKind,
    setNewKind,
    newKindLabel,
    setNewKindLabel,
    busy,
    act,
  } = props;
  return (
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
  );
}
