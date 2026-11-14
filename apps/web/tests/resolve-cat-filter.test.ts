import { describe, expect, it } from "vitest";

import { resolveCatFilter } from "@/app/(main)/torrents/_parts/torrents-utils";

/**
 * 默认分类初值解析（2026-11-14 救活 browsecat 批）：URL 未给 category_id
 * 时按用户偏好（browsecat）初始化。钉住四条：
 *  1. URL 显式值永远优先（导航落地不被动偏好覆盖）；
 *  2. 偏好过滤到现分类集——旧站 401-410 存量（id 对不上）与站长已删
 *     分类不下发（后端 parse_category_ids 对未知 id 是 ANY 空命中，
 *     下发等于列表全空）；
 *  3. 偏好全失效 / 空串 / 缺省 → undefined（不筛选，不是全空列表）；
 *  4. 偏好里的空白/脏字符容错。
 */

const SITE_CATS = [1, 2, 3, 10];

describe("resolveCatFilter（默认分类偏好初值）", () => {
  it("URL 显式 category_id 优先于偏好", () => {
    expect(resolveCatFilter("2,3", "1", SITE_CATS)).toBe("2,3");
    // URL 给了但不在分类集里：URL 仍是权威（后端自行裁决），不做二次过滤
    expect(resolveCatFilter("999", "1", SITE_CATS)).toBe("999");
  });

  it("URL 缺省时按偏好过滤到现分类集", () => {
    expect(resolveCatFilter(undefined, "1,2", SITE_CATS)).toBe("1,2");
    // 混合：有效 id + 旧站残留 401 + 已删分类 7 → 只留有效
    expect(resolveCatFilter(undefined, "1,401,7,2", SITE_CATS)).toBe("1,2");
  });

  it("偏好全失效/空/缺省 → undefined（不筛选）", () => {
    expect(resolveCatFilter(undefined, "401,402", SITE_CATS)).toBeUndefined();
    expect(resolveCatFilter(undefined, "", SITE_CATS)).toBeUndefined();
    expect(resolveCatFilter(undefined, null, SITE_CATS)).toBeUndefined();
    expect(resolveCatFilter(undefined, undefined, SITE_CATS)).toBeUndefined();
    // 站点分类清单拉取失败（空集）→ 偏好整体失效，不筛选
    expect(resolveCatFilter(undefined, "1,2", [])).toBeUndefined();
  });

  it("偏好空白与脏字符容错", () => {
    expect(resolveCatFilter(undefined, " 1 , 2 ,", SITE_CATS)).toBe("1,2");
    expect(resolveCatFilter(undefined, "a,b,2", SITE_CATS)).toBe("2");
  });
});
