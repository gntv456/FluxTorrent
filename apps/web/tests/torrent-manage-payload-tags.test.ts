import { describe, it, expect } from "vitest";

import { buildEditPayload } from "@/components/torrent-manage-submit";

/**
 * P-622：聚合口 tags 段读失败时回落 {dict:[],mine:[]}（aggregate.rs），
 * 编辑面板不渲染任何标签控件，旧载荷却发 tag_ids:[]；后端按
 * 「Some([])=清空整组」先 DELETE 再 apply，既有标签（含 official，
 * official_tag 列随之归零）被静默删光。修复=字典为空时省略该键。
 * 负控：回放 HEAD 版本 ⇒ 第一条红成 tag_ids 存在且为 []。
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

describe("P-622 编辑载荷不把标签元数据缺失当成清空", () => {
  it("标签字典缺失而该种有标签：不发 tag_ids 键", () => {
    const body = buildEditPayload({
      ...base,
      tagMine: [5, 9],
      fTags: [],
      dictRows: [],
    });
    expect("tag_ids" in body).toBe(false);
  });

  it("字典在册：普通标签 + 原 official 一并提交（正控）", () => {
    const body = buildEditPayload({
      ...base,
      fTags: [3],
      tagMine: [3, 7],
      dictRows: [
        { id: 3, kind: "attribute" },
        { id: 7, kind: "official" },
      ],
    });
    expect(body.tag_ids).toEqual([3, 7]);
  });

  it("字典只有 official 类：仍回传原 official 不动（正控）", () => {
    const body = buildEditPayload({
      ...base,
      fTags: [],
      tagMine: [7],
      dictRows: [{ id: 7, kind: "official" }],
    });
    expect(body.tag_ids).toEqual([7]);
  });
});
