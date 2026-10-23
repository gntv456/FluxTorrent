import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";

import { TorrentManage } from "@/components/torrent-manage";
import { LocaleProvider } from "@/i18n/client";
import { zhCN } from "@/i18n/zh-CN";

/**
 * P-623：种子详情页把「编辑 / 站方删除表单」对全体登录会员渲染——
 * 组件原本不看归属，后端 PUT/DELETE 才回 403。用户看到的是别人种子
 * 上的删除按钮，点开是一张操作员表单。
 *
 * 修复：编辑/删除入口按 canManage（种主或管理组）显隐；补种请求
 * 后端对全体会员开放（torrents/reseed.rs 只查种子状态），必须保留。
 * 负控：回放 HEAD 版本（组件不认识 canManage）⇒ 第一条红成按钮仍在。
 */

vi.mock("next/navigation", () => ({
  useRouter: () => ({ refresh: vi.fn(), push: vi.fn() }),
}));

const T = zhCN.torrentManage2;
const R = zhCN.reseed2;

function mount(canManage: boolean, seeders: number) {
  return render(
    <LocaleProvider dict={zhCN} locale="zh-CN" currency="魔力">
      <TorrentManage
        torrentId={42}
        name="示例种子"
        smallDescr={null}
        descr={null}
        anonymous={false}
        categoryId={2}
        price={0}
        posterUrl={null}
        mediainfo={null}
        sections={{}}
        secKinds={[]}
        secDict={{}}
        cats={[]}
        seeders={seeders}
        imdbId={null}
        tagDict={[]}
        tagMine={[]}
        posState={0}
        posStateUntil={null}
        pickType={0}
        isStaff={false}
        autoOpen={false}
        canManage={canManage}
      />
    </LocaleProvider>,
  );
}

describe("P-623 详情页管理入口按归属显隐", () => {
  it("非种主/非管理组：没有编辑与删除，补种仍在", () => {
    mount(false, 0);
    expect(screen.queryByRole("button", { name: new RegExp(T.edit) })).toBe(
      null,
    );
    expect(screen.queryByRole("button", { name: new RegExp(T.del) })).toBe(
      null,
    );
    expect(
      screen.getByRole("button", { name: new RegExp(R.btn) }),
    ).toBeTruthy();
  });

  it("非种主/非管理组且有种：整块无按钮（补种也不该出现）", () => {
    mount(false, 3);
    expect(screen.queryAllByRole("button")).toHaveLength(0);
  });

  it("种主/管理组：编辑与删除都在（正控）", () => {
    mount(true, 0);
    expect(
      screen.getByRole("button", { name: new RegExp(T.edit) }),
    ).toBeTruthy();
    expect(
      screen.getByRole("button", { name: new RegExp(T.del) }),
    ).toBeTruthy();
  });

  it("非种主用 ?edit=1 深链也打不开编辑表单", () => {
    render(
      <LocaleProvider dict={zhCN} locale="zh-CN" currency="魔力">
        <TorrentManage
          torrentId={42}
          name="示例种子"
          smallDescr={null}
          descr={null}
          anonymous={false}
          categoryId={2}
          price={0}
          posterUrl={null}
          mediainfo={null}
          sections={{}}
          secKinds={[]}
          secDict={{}}
          cats={[]}
          seeders={0}
          imdbId={null}
          tagDict={[]}
          tagMine={[]}
          posState={0}
          posStateUntil={null}
          pickType={0}
          isStaff={false}
          autoOpen
          canManage={false}
        />
      </LocaleProvider>,
    );
    expect(screen.queryByRole("dialog")).toBe(null);
  });
});
