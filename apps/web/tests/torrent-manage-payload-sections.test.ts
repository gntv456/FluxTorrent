import { describe, it, expect } from "vitest";

import { buildEditPayload } from "@/components/torrent-manage-submit";

/**
 * P-620：编辑种子切分类时 section-dict 取数失败 ⇒ catKinds 空而 fSec
 * 仍是旧分类的值。旧载荷发 sections:{}，后端按「整组重建」
 * （clear_absent=true）把该种已有维度全清空。修复=此时省略该键。
 * 负控：回放 HEAD 版本 ⇒ 第一条红成 sections 存在且为 {}。
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

describe("P-620 编辑载荷 sections 不清空既有维度", () => {
  it("维度元数据缺失而旧值还在：不发 sections 键", () => {
    const body = buildEditPayload({
      ...base,
      fSec: { resolution: "5" },
    });
    expect("sections" in body).toBe(false);
  });

  it("维度元数据在册：照常编码 sections（正控）", () => {
    const body = buildEditPayload({
      ...base,
      fSec: { resolution: "5" },
      catKinds: [
        { kind: "resolution", label: "分辨率", sort: 10, field_type: "select" },
      ],
    });
    expect(body.sections).toEqual({ resolution: 5 });
  });

  it("真实无维度的分类：仍发 sections:{} 保留清空语义", () => {
    const body = buildEditPayload({ ...base });
    expect(body.sections).toEqual({});
  });

  it("多选维度仍编成 dict_ids（拆函数后行为不变）", () => {
    const body = buildEditPayload({
      ...base,
      fSec: { genres: "3,7" },
      catKinds: [
        { kind: "genres", label: "类型", sort: 20, field_type: "multiselect" },
      ],
    });
    expect(body.sections).toEqual({ genres: { dict_ids: [3, 7] } });
  });
});
