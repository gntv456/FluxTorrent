import { describe, expect, it } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import {
  TorrentsColSettings,
} from "@/app/(main)/torrents/_parts/torrents-col-settings";
import { zhCN } from "@/i18n/zh-CN";

/** 站点级隐藏列必须置灰，个人级隐藏只取消勾选。 */
describe("列表列设置", () => {
  it("站点隐藏与个人隐藏分开呈现", () => {
    render(
      <TorrentsColSettings
        initialHidden={["seeders"]}
        siteHidden={["comments"]}
      />,
    );
    fireEvent.click(
      screen.getByRole("button", {
        name: new RegExp(zhCN.torrents.colSettings),
      }),
    );
    const comments = screen.getByRole("checkbox", {
      name: zhCN.torrents.colNames.comments,
    });
    const seeders = screen.getByRole("checkbox", {
      name: zhCN.torrents.colNames.seeders,
    });
    const cat = screen.getByRole("checkbox", {
      name: zhCN.torrents.colNames.cat,
    });
    expect(comments).toBeDisabled();
    expect(comments).not.toBeChecked();
    expect(seeders).not.toBeDisabled();
    expect(seeders).not.toBeChecked();
    expect(cat).not.toBeDisabled();
    expect(cat).toBeChecked();
  });
});
