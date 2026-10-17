import { describe, it, expect } from "vitest";

import {
  codesLine,
  groupEndpoints,
  opDetail,
  paramsLine,
  unwrapSpec,
  VERBS,
  type RawOp,
  type RawParam,
} from "@/lib/openapi-groups";

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

/** 「展开」那一栏（样图 § api-docs 第四列）。这三条钉的是同一族风险：
 *  规范里没写的东西，界面不能假装写了——尤其不能拿 200 兜底成「有响应声明」。 */
describe("openapi 端点详情", () => {
  const P = (
    name: string,
    loc: string,
    extra: Partial<RawParam> = {},
  ): RawParam => ({ name, in: loc, ...extra });

  it("path 级参数并进该 path 下每个方法（写在 path item 上的 limit 不会消失）", () => {
    const spec = unwrapSpec({
      paths: {
        "/torrents": {
          parameters: [P("limit", "query")],
          get: { tags: ["种子"], parameters: [P("q", "query")] },
          post: { tags: ["种子"] },
        },
      },
    });
    const { groups } = groupEndpoints(spec, "未分组");
    const get = groups[0].items.find((e) => e.method === "GET");
    const post = groups[0].items.find((e) => e.method === "POST");
    expect(get?.detail.params.map((p) => p.name)).toEqual(["limit", "q"]);
    expect(post?.detail.params.map((p) => p.name)).toEqual(["limit"]);
  });

  it("同名同位置只留一条，required / type 按规范原样带出", () => {
    const d = opDetail([P("limit", "query", { schema: { type: "integer" } })], {
      parameters: [
        P("limit", "query"),
        P("id", "path", { required: true, schema: { type: "string" } }),
      ],
    });
    expect(d.params).toEqual([
      { name: "limit", loc: "query", required: false, type: "integer" },
      { name: "id", loc: "path", required: true, type: "string" },
    ]);
  });

  it("没有 responses 就回空数组，界面那句「未声明」才有依据（不兜底成 200）", () => {
    expect(opDetail(undefined, { tags: ["x"] }).codes).toEqual([]);
    expect(opDetail(undefined, { responses: { "200": {}, "404": {} } }).codes)
      .toEqual(["200", "404"]);
  });

  it("responses 里混进的动词键不算状态码", () => {
    const d = opDetail(undefined, { responses: { get: {}, "201": {} } });
    expect(d.codes).toEqual(["201"]);
  });

  it("name 缺失的脏参数条目被跳过，不输出「undefined（query）」", () => {
    const d = opDetail([{ in: "query" } as RawParam], {
      parameters: [P("", "query"), P("ok", "query")],
    });
    expect(d.params.map((p) => p.name)).toEqual(["ok"]);
  });
});

describe("openapi 详情文案", () => {
  const p = (
    name: string,
    loc = "query",
    required = false,
    type = "",
  ) => ({ name, loc, required, type });

  it("空参数回占位句，有参数逐条拼「name（in, required, type）」", () => {
    expect(paramsLine([], "无参数")).toBe("无参数");
    expect(
      paramsLine([p("q"), p("id", "path", true, "integer")], "无参数"),
    ).toBe("q（query）、id（path, required, integer）");
    expect(paramsLine([p("k", "header", false, "string")], "无参数")).toBe(
      "k（header, string）",
    );
  });

  it("空状态码回占位句；非空带标签前缀", () => {
    expect(codesLine([], "规范未声明响应码", "响应码")).toBe("规范未声明响应码");
    expect(codesLine(["200", "404"], "规范未声明响应码", "响应码")).toBe(
      "响应码: 200, 404",
    );
  });
});
