import { describe, expect, it } from "vitest";

/** 双栏选中态 URL 同步（M4，方案 §4.4 形态切换不丢上下文）：
 *  TorrentSplitView 的 select() 逻辑镜像——?selected= 增删保持其余参数。
 *  （组件依赖 matchMedia/router，此处测 URL 操作纯函数口径；
 *  真交互走 768px 无头验收。） */

function applySelected(search: string, id: number | null): string {
  const sp = new URLSearchParams(search);
  if (id) sp.set("selected", String(id));
  else sp.delete("selected");
  sp.delete("cursor");
  const q = sp.toString();
  return q ? `/torrents?${q}` : "/torrents";
}

describe("split view selected URL", () => {
  it("无参时设置 selected 不携带多余问号", () => {
    expect(applySelected("", 42)).toBe("/torrents?selected=42");
  });
  it("保留既有筛选并清游标", () => {
    expect(applySelected("alive=2&cursor=abc", 42)).toBe(
      "/torrents?alive=2&selected=42",
    );
  });
  it("清除 selected 时其余参数保留、无 selected 残留", () => {
    expect(applySelected("alive=2&selected=42", null)).toBe(
      "/torrents?alive=2",
    );
  });
  it("重复选择同一项幂等", () => {
    expect(applySelected("selected=42", 42)).toBe("/torrents?selected=42");
  });
});
