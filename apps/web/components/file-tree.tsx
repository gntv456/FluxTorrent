"use client";

import { useMemo, useState } from "react";
import { useI18n } from "@/i18n/client";
import { formatBytes } from "@/lib/format";

export interface FileRowItem {
  file_index: number;
  path: string;
  size: number;
}

interface DirGroup {
  dir: string;
  files: FileRowItem[];
  total: number;
}

/** 按路径第一段分组（NP filelist 树形口径的最小实现）：
 *  目录行可折叠，目录行显示聚合大小；无目录前缀（单文件种子）退化为平铺。 */
function groupByDir(files: FileRowItem[]): DirGroup[] {
  const map = new Map<string, DirGroup>();
  for (const f of files) {
    const sep = f.path.lastIndexOf("/");
    const dir = sep >= 0 ? f.path.slice(0, sep) : "";
    let g = map.get(dir);
    if (!g) {
      g = { dir, files: [], total: 0 };
      map.set(dir, g);
    }
    g.files.push(f);
    g.total += f.size;
  }
  const groups = [...map.values()];
  // 单一「无前缀」组且组内文件少时不折叠，保持单文件种子的旧观感
  return groups.sort((a, b) => a.dir.localeCompare(b.dir));
}

/** 文件列表（树形折叠）：目录行 = 第一层前缀；数千文件的剧集包不再渲染超长平铺表 */
export function FileTree({ files }: { files: FileRowItem[] }) {
  const { dict } = useI18n();
  const groups = useMemo(() => groupByDir(files), [files]);
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const plain = groups.length === 1 && groups[0].dir === "";

  function toggle(dir: string) {
    setCollapsed((prev) => {
      const next = new Set(prev);
      if (next.has(dir)) next.delete(dir);
      else next.add(dir);
      return next;
    });
  }

  if (plain) {
    return (
      <table className="td-files">
        <tbody>
          {groups[0].files.map((f) => (
            <tr key={f.file_index}>
              <td className="min-w-0 truncate">{f.path}</td>
              <td className="num shrink-0 text-right text-sub">{formatBytes(f.size)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    );
  }

  return (
    <table className="td-files">
      <tbody>
        {groups.map((g) => {
          const isCollapsed = collapsed.has(g.dir);
          return (
            <>
              <tr key={g.dir} className="td-files__dir">
                <td>
                  <button
                    type="button"
                    onClick={() => toggle(g.dir)}
                    className="flex min-h-[28px] w-full items-center gap-1 text-left font-bold text-sky-deep"
                    aria-expanded={!isCollapsed}
                  >
                    <span aria-hidden>{isCollapsed ? "▸" : "▾"}</span>
                    {g.dir || "（根目录）"}
                    <span className="num text-xs font-normal text-sub">
                      （{g.files.length} {dict.torrent.fileUnit}）
                    </span>
                  </button>
                </td>
                <td className="num shrink-0 text-right text-xs text-sub">
                  {formatBytes(g.total)}
                </td>
              </tr>
              {!isCollapsed &&
                g.files.map((f) => (
                  <tr key={f.file_index}>
                    <td className="min-w-0 truncate pl-6">{f.path.slice(g.dir.length + 1) || f.path}</td>
                    <td className="num shrink-0 text-right text-sub">{formatBytes(f.size)}</td>
                  </tr>
                ))}
            </>
          );
        })}
      </tbody>
    </table>
  );
}
