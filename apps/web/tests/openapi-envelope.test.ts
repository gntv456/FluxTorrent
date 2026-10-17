import { describe, it, expect } from "vitest";

import { groupEndpoints, unwrapSpec, VERBS } from "@/lib/openapi-groups";

/**
 * openapi.json 的「信封漏剥」钉桩。
 *
 * /api-docs 页此前读 `json.paths`，而后站每个端点都套 `{code,message,data,
 * request_id}` 信封 ⇒ 现网 163 个 operation 被算成 0 个，整页只剩一句
 * 「API 暂不可达」，而接口本身完全是好的。这类洞在界面上不报错、不红，
 * 只是永远空——所以必须在纯函数层钉住，且三种输入都要覆盖：
 * 带信封、不带信封、paths 里的非 HTTP 动词键。
 */

const ENVELOPED = {
  code: 0,
  message: "ok",
  request_id: "r1",
  data: {
    info: { title: "FluxTorrent Open API", version: "1.4.0" },
    paths: {
      "/torrents": {
        get: { tags: ["种子"], summary: "种子列表" },
        parameters: [{ name: "limit" }],
      },
      "/me": { get: { tags: ["我的数据"], summary: "当前用户" } },
      "/auth/login": { post: { tags: ["认证"], summary: "登录" } },
    },
  },
};

describe("openapi 取数与分组", () => {
  it("带 ok() 信封时从 data 里取", () => {
    const spec = unwrapSpec(ENVELOPED);
    expect(Object.keys(spec.paths ?? {})).toHaveLength(3);
    expect(spec.info?.version).toBe("1.4.0");
  });

  it("没有信封的部署也照收（不能只认一种形状）", () => {
    const bare = { paths: { "/x": { get: { tags: ["种子"] } } }, info: {} };
    expect(Object.keys(unwrapSpec(bare).paths ?? {})).toEqual(["/x"]);
  });

  it("坏输入不炸：null / 字符串 / 空对象都回空 spec", () => {
    for (const bad of [null, undefined, "nope", 7]) {
      expect(unwrapSpec(bad)).toEqual({});
    }
  });

  it("分组计数只算 HTTP 动词，parameters 这类键不算端点", () => {
    const { groups, total } = groupEndpoints(unwrapSpec(ENVELOPED), "未分组");
    expect(total).toBe(3);
    const byTag = Object.fromEntries(
      groups.map((g) => [g.tag, g.items.length]),
    );
    expect(byTag).toEqual({ 种子: 1, 我的数据: 1, 认证: 1 });
  });

  it("没有 tags 的端点落到兜底组名（不是丢进 undefined 组）", () => {
    const spec = unwrapSpec({
      paths: { "/y": { get: { summary: "无 tag" } } },
    });
    const { groups, total } = groupEndpoints(spec, "未分组");
    expect(total).toBe(1);
    expect(groups[0].tag).toBe("未分组");
  });

  it("组内按路径排、组间按条数从多到少", () => {
    const spec = unwrapSpec({
      paths: {
        "/b": { get: { tags: ["大组"] } },
        "/a": { get: { tags: ["大组"] } },
        "/c": { get: { tags: ["小组"] } },
      },
    });
    const { groups } = groupEndpoints(spec, "未分组");
    expect(groups.map((g) => g.tag)).toEqual(["大组", "小组"]);
    expect(groups[0].items.map((i) => i.path)).toEqual(["/a", "/b"]);
  });

  it("动词表就是这五个（PATCH/DELETE 不能漏，徽章样式按它生成）", () => {
    expect([...VERBS].sort()).toEqual(
      ["delete", "get", "patch", "post", "put"].sort(),
    );
  });

  it("方法名大写输出（界面徽章用 class 名依赖大小写无关的原文）", () => {
    const spec = unwrapSpec({ paths: { "/z": { delete: { tags: ["种子"] } } } });
    const { groups } = groupEndpoints(spec, "未分组");
    expect(groups[0].items[0].method).toBe("DELETE");
  });
});
