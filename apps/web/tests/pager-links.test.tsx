import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { Pager } from "@/components/ui/pager";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * Pager 的链接档（hrefs）——为版块页补的那一半。
 *
 * 版块页是 RSC，传不了 onStep 函数；原先这类页面只能各写一份翻页条
 * （或者干脆不翻页，于是 LIMIT 50 之外的主题永远打不开）。
 * 钉住：hrefs 档渲染真链接、首尾页的禁用态、且不回落到按钮档。
 */

const c = zhCN.common;
// hrefs 按页序（下标 = page-1）；末页那一格刻意给空串，验禁用档不给 href
const HREFS_MID = ["/forums/1", "/forums/1?page=2", "/forums/1?page=3"];
const HREFS_FIRST = ["/forums/1", "/forums/1?page=2"];
const HREFS_LAST = ["/forums/1?page=2", "/forums/1?page=3", ""];

function mountLink() {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <Pager page={2} pages={3} total={120} hrefs={HREFS_MID} />
    </LocaleProvider>,
  );
}

describe("Pager 链接档", () => {
  it("渲染的是链接不是按钮，且指向各自页", () => {
    mountLink();
    const prev = screen.getByRole("link", { name: c.prevPage });
    const next = screen.getByRole("link", { name: c.nextPage });
    expect(prev.getAttribute("href")).toBe("/forums/1");
    expect(next.getAttribute("href")).toBe("/forums/1?page=3");
    expect(screen.queryByRole("button")).toBeNull();
  });

  it("第 1 页时「上一页」不可点，末页时「下一页」不可点", () => {
    const { unmount } = render(
      <LocaleProvider dict={zhCN} locale="zh-CN">
        <Pager page={1} pages={3} total={120} hrefs={HREFS_FIRST} />
      </LocaleProvider>,
    );
    // 禁用档刻意不给 href：没有 href 的 <a> 不进可及性树的 link 角色，
    // 于是既点不动也不被读屏器报成「链接」，只剩一句禁用文案。
    expect(screen.queryByRole("link", { name: c.prevPage })).toBeNull();
    expect(screen.getByText(c.prevPage)).toBeInTheDocument();
    unmount();
    render(
      <LocaleProvider dict={zhCN} locale="zh-CN">
        <Pager page={3} pages={3} total={120} hrefs={HREFS_LAST} />
      </LocaleProvider>,
    );
    expect(screen.queryByRole("link", { name: c.nextPage })).toBeNull();
    expect(screen.getByText(c.nextPage)).toBeInTheDocument();
    expect(screen.getByText(/120/)).toBeInTheDocument();
  });

  it("不传 hrefs 时仍是回调档（老调用方不受影响）", async () => {
    const user = userEvent.setup();
    const onStep = vi.fn();
    render(
      <LocaleProvider dict={zhCN} locale="zh-CN">
        <Pager page={1} pages={2} onStep={onStep} />
      </LocaleProvider>,
    );
    expect(screen.queryByRole("link")).toBeNull();
    await user.click(screen.getByRole("button", { name: c.nextPage }));
    expect(onStep).toHaveBeenCalledWith(2);
  });
});
