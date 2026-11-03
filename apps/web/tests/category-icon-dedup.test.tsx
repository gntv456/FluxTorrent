/** 0458 图标解撞回归：三个新图标（shortdrama/audiobook/variety）注册进 PATHS，
 *  且 CatBlock 只认注册名——未注册键回落首字色块（0166 语义）不被破坏。
 *  背景是库里短剧/有声书/综艺曾与电视剧/音乐同图标（tv/music 撞车）。 */
import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import { Icon, ICON_NAMES } from "@/components/icons";
import { CatBlock } from "@/components/torrent-table-badges";

describe("0458 分类图标解撞", () => {
  it("三个专属图标已注册", () => {
    for (const n of ["shortdrama", "audiobook", "variety"]) {
      expect(ICON_NAMES).toContain(n);
    }
  });

  it("与剧集/音乐的旧键并存不互扰", () => {
    // 旧键仍在（电视剧/音乐分类还在用），新键渲染出独立 svg
    for (const n of ["tv", "music", "shortdrama", "audiobook", "variety"]) {
      const { container } = render(<Icon name={n} />);
      expect(container.querySelector("svg")).toBeTruthy();
    }
  });

  it("CatBlock 对新键走位图图标渲染、未知键回落首字", () => {
    // 2026-11 位图接线：内层由单色 <Icon>（svg）换为 <CatIcon>（img 位图），
    // 断言点相应从 svg 改为 img src；0166 首字回落语义不变。
    const known = render(
      <CatBlock ico="shortdrama" name="短剧" color="#c084fc" />,
    );
    const img = known.container.querySelector("img");
    expect(img).toBeTruthy();
    expect(img?.getAttribute("src")).toContain("/cat-icons/");
    expect(img?.getAttribute("src")).toContain("cat-shortdrama");

    const fallback = render(
      <CatBlock ico="not-a-real-icon" name="有声书" color="#f472b6" />,
    );
    expect(fallback.container.querySelector("img")).toBeNull();
    expect(screen.getByText("有")).toBeTruthy();
  });
});
