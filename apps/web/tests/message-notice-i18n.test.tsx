import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { MessageTable } from "@/components/message-center-table";
import { LocaleProvider } from "@/i18n/client";
import { en } from "@/i18n/en";

// setup 把 useI18n 全局钉成 zh-CN；本用例要验英文现取，就地覆盖。
vi.mock("@/i18n/client", async (importOriginal) => {
  const orig = await importOriginal<typeof import("@/i18n/client")>();
  return {
    ...orig,
    useI18n: () => ({ dict: en, locale: "en", currency: "Magic" }),
  };
});

/** P-584：厂牌/字幕组订阅推送带 kind 进消息中心，必须按当前语言现取文案
 *  （此前只登记了三种 kind，这两类在英文界面回落后端存的中文成品文本）。 */

const rows = [
  {
    id: 1,
    counterpart: null,
    subject: "订阅的出品方有新片",
    body: "你订阅的出品方「A24」有新种子过审：#99 Movie.X。",
    read_at: null,
    created_at: "2026-10-22T00:00:00Z",
    is_system: true,
    kind: "network_new_release",
    params: { kind: "network", entity: "A24", id: 99, name: "Movie.X" },
  },
  {
    id: 2,
    counterpart: null,
    subject: "订阅的字幕组有新作",
    body: "你订阅的字幕组「北极字幕组」有新种子过审：#100 Show.Y。",
    read_at: null,
    created_at: "2026-10-22T00:00:00Z",
    is_system: true,
    kind: "subgroup_new_release",
    params: {
      kind: "subtitle_group",
      entity: "北极字幕组",
      id: 100,
      name: "Show.Y",
    },
  },
];

function wrap(openId: number) {
  return render(
    <LocaleProvider dict={en} locale="en">
      <MessageTable
        box="inbox"
        rows={rows}
        selected={[]}
        onToggle={vi.fn()}
        openId={openId}
        onOpen={vi.fn()}
        onReply={vi.fn()}
        onForward={vi.fn()}
      />
    </LocaleProvider>,
  );
}

describe("MessageTable 订阅推送 kind 渲染", () => {
  it("英文界面按 kind 现取主题", () => {
    wrap(0);
    expect(
      screen.getByText(en.messages.kindNetworkNewRelease),
    ).toBeTruthy();
    expect(screen.getByText(en.messages.kindSubgroupNewRelease)).toBeTruthy();
  });

  it("出品方正文用实体名 + 种子名插值", async () => {
    wrap(1);
    expect(await screen.findByText(/Studio “A24”/)).toBeTruthy();
  });

  it("字幕组正文用实体名 + 种子名插值", async () => {
    wrap(2);
    expect(await screen.findByText(/Subtitle group “北极字幕组”/))
      .toBeTruthy();
  });
});
