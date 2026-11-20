import { describe, it, expect } from "vitest";

import { buildEditPayload } from "@/components/torrent-manage-submit";

/**
 * 编辑弹窗对齐发布页（2026-11-20）：豆瓣/TMDB 与 H&R 分成开关此前
 * 只有发种口能写，编辑口静默丢字段——用户改了豆瓣链接保存后原值不动。
 * 口径：douban/tmdb 始终提交（空串=后端清除 media_info 键、非空=归一）；
 * hr_punish 始终提交（后端 None=不动，这里显式带用户选择）。
 */

const base = {
  fName: "示例种子",
  fSub: "",
  fDescr: "",
  fAnon: false,
  fCat: 2,
  fImdb: "",
  fPoster: "",
  fMediainfo: "",
  fNfo: "",
  fDouban: "",
  fTmdb: "",
  fHrPunish: false,
  fSec: {} as Record<string, string>,
  catKinds: [] as {
    kind: string;
    label: string;
    sort: number;
    field_type?: string;
  }[],
  fTags: [] as number[],
  tagMine: [] as number[],
  dictRows: [] as { id: number; kind: string }[],
  fPos: 0,
  fPosUntil: "",
  fPick: 0,
};

describe("编辑载荷对齐发布页 metaIds/H&R", () => {
  it("豆瓣/TMDB 始终提交（空串=清除语义）", () => {
    const body = buildEditPayload({ ...base });
    expect(body.douban).toBe("");
    expect(body.tmdb).toBe("");
  });
  it("填了链接原样交给后端归一（前端不解析）", () => {
    const body = buildEditPayload({
      ...base,
      fDouban: "https://movie.douban.com/subject/1292052/",
      fTmdb: "https://www.themoviedb.org/movie/603",
    });
    expect(body.douban).toBe(
      "https://movie.douban.com/subject/1292052/",
    );
    expect(body.tmdb).toBe("https://www.themoviedb.org/movie/603");
  });
  it("H&R 分成开关按用户选择提交", () => {
    expect(buildEditPayload({ ...base, fHrPunish: true }).hr_punish).toBe(
      true,
    );
    expect(buildEditPayload({ ...base }).hr_punish).toBe(false);
  });
});
