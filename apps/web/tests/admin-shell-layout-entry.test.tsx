import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/react";

import { AdminShell, type PanelEntry } from "@/components/admin-shell";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * 0471：首页排版/视图布局的兜底导航补注必须按 min_class 收口。
 * 此前补注无条件注入：98 档以下职员看得到入口、点保存却 403
 * （入口与后端执法不是同一把钥匙）。保存端点自 settings.manage(99)
 * 拆为 layout.manage(98)，补注同步只给 ≥98 档看。
 *
 * 负控：回放 HEAD admin-shell（inject 不判 classId）⇒ 第一条红
 * （95 档也画出了「首页排版」按钮）；其余正控两边都绿。
 */

function mount(
  classId: number | undefined,
  entries: PanelEntry[] = [],
  tool = "homelayout",
) {
  render(
    <LocaleProvider dict={zhCN} locale="zh-CN">
      <AdminShell
        entries={entries}
        tool={tool}
        onTool={() => {}}
        badges={{}}
        role="administrator"
        classId={classId}
      >
        <div />
      </AdminShell>
    </LocaleProvider>,
  );
}

const navBtn = (name: string) =>
  screen.queryByRole("button", { name });

describe("管理导航兜底补注的档位收口（0471）", () => {
  it("95 档（主管）看不到排版入口，运维入口仍在（补注本身活着）", () => {
    mount(95, [], "ops");
    expect(navBtn(zhCN.homeLayout.title)).toBeNull();
    expect(navBtn(zhCN.viewLayout.title)).toBeNull();
    // 运维 min_class=90：同一次补注机制产物，证明不是整段没渲染
    expect(navBtn(zhCN.adminops.entry)).toBeTruthy();
  });

  it("98 档（维护开发员）看得到两条排版入口", () => {
    mount(98);
    expect(navBtn(zhCN.homeLayout.title)).toBeTruthy();
    expect(navBtn(zhCN.viewLayout.title)).toBeTruthy();
  });

  it("后端已返回条目时不重复补注（99 档只有一条）", () => {
    mount(99, [
      {
        section: "system",
        name: zhCN.homeLayout.title,
        url: "/admin?tool=homelayout",
        info: zhCN.homeLayout.hint,
        tab_key: "homelayout",
        min_class: 98,
      },
    ]);
    const btns = screen.getAllByRole("button", {
      name: zhCN.homeLayout.title,
    });
    expect(btns).toHaveLength(1);
  });

  it("组内条目全未登记子分类时不双份渲染（loose 回退不再叠 items）", () => {
    // 0421 旧形：section=content，而 content 子分类没登记 homelayout
    mount(99, [
      {
        section: "content",
        name: zhCN.homeLayout.title,
        url: "/admin?tool=homelayout",
        info: zhCN.homeLayout.hint,
        tab_key: "homelayout",
        min_class: 98,
      },
    ]);
    expect(
      screen.getAllByRole("button", { name: zhCN.homeLayout.title }),
    ).toHaveLength(1);
  });

  it("身份未加载（classId 缺省）不补注，避免闪现打不开的入口", () => {
    mount(undefined);
    expect(navBtn(zhCN.homeLayout.title)).toBeNull();
    expect(navBtn(zhCN.viewLayout.title)).toBeNull();
  });
});