import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";

import { FileTree } from "@/components/file-tree";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 文件清单为空必须出声（批二百一十九，配套提交 ff72c396）。
 *
 * 现网实测：7 枚过审种 `torrents.numfiles`=1/2 却在 `files` 表里一行都没有
 * （它们连原始 .torrent 都没存档）。旧写法把空数组喂进 `FileTree` 会渲染出
 * 一张**没有任何文字**的空表，用户在同一个详情页里看到的是「文件数 1」+
 * 「文件表空白」，只能读成「这颗种没有文件」——而真正的事实是记录坏了。
 *
 * 钉两条（互为对照，防「恒显提示」这种假修）：
 *   1) 空数组 ⇒ 出现下载口同口径那句中文（不是「暂无文件」这种无痛描述）；
 *   2) 非空 ⇒ 那句提示不出现，且文件路径照常渲染。
 *
 * 负控：把 `file-tree.tsx` 里 `files.length === 0` 那个早退分支整块删掉后，
 * 第 1 条必须报红（`getByText` 找不到节点），第 2 条仍绿——这条红才证明
 * 提示是真由该分支产生的，而不是字典里那句话在别处渲染出来的。
 */

function view(node: React.ReactNode) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      {node}
    </LocaleProvider>,
  );
}

describe("文件清单空态", () => {
  it("空数组：必须说出「原始 .torrent 未存档，请联系管理组补档」", () => {
    view(<FileTree files={[]} />);
    const hint = screen.getByText(zhCN.torrent.filesMissing);
    expect(hint.textContent).toContain("未存档");
    expect(zhCN.torrent.filesMissing).toContain("管理组");
  });

  it("非空：不出现该提示，文件路径照常渲染", () => {
    view(
      <FileTree
        files={[
          {
            file_index: 0,
            path: "沙丘.Dune.2021.1080p.BluRay.mkv",
            size: 8_589_934_592,
          },
        ]}
      />,
    );
    expect(
      screen.getByText("沙丘.Dune.2021.1080p.BluRay.mkv"),
    ).toBeInTheDocument();
    expect(screen.queryByText(zhCN.torrent.filesMissing)).toBeNull();
  });
});
