import { describe, expect, it } from "vitest";
import type {
  TorrentListItem,
  WorkWithSeeds,
} from "@fluxtorrent/domain-types";
import { workFlags } from "@/components/work-flags";
import { zhCN } from "@/i18n/zh-CN";

function seed(
  over: Partial<TorrentListItem> = {},
): TorrentListItem {
  return {
    id: 1,
    info_hash: "x",
    name: "Movie.2024.1080p.WEB-DL",
    small_descr: null,
    category_id: 1,
    medium_id: null,
    grade_id: null,
    edition_id: null,
    size: 1,
    seeders: 0,
    leechers: 0,
    times_completed: 0,
    comments: 0,
    official: false,
    anonymous: false,
    approval_status: 1,
    sticky: false,
    promotion: null,
    promotion_ends_at: null,
    rating: null,
    poster: null,
    imdb_id: null,
    owner_name: null,
    created_at: "2026-10-28T00:00:00Z",
    ...over,
  };
}

function work(over: Partial<WorkWithSeeds> = {}): WorkWithSeeds {
  return {
    key: "g1",
    name: "Movie.2024.2160p.WEB-DL",
    poster: null,
    rating: "7.9",
    category_id: 1,
    synopsis: null,
    detail: null,
    imdb_id: null,
    tmdb_id: "123",
    tmdb_url: null,
    douban_id: "456",
    meta: {
      source: "tmdb",
      name: null,
      year: 2024,
      rating: 8.2,
      runtime: 45,
      region: "中国大陆",
      director: ["导演A"],
      cast_list: ["演员B", "演员C"],
      genres: ["剧情", "爱情"],
      poster: null,
      synopsis: null,
    },
    year: 2024,
    torrents: 2,
    seeders: 10,
    leechers: 2,
    size: 100,
    latest_at: "2026-10-28T00:00:00Z",
    top_id: 1,
    tags: [],
    seeds: [
      seed({
        id: 1,
        name: "Movie.2024.2160p.WEB-DL",
        official: true,
        promotion: "free",
      }),
      seed({
        id: 2,
        name: "Movie.2024.1080p.BluRay",
        promotion: "x2free",
      }),
    ],
    ...over,
  };
}

describe("workFlags", () => {
  it("种子画质/促销/官方与作品评分/主创合并", () => {
    const f = workFlags(work(), zhCN);
    expect(f.qualities).toEqual(["4K", "1080P"]);
    expect(f.promos.map((p) => p.key)).toEqual(["free", "x2free"]);
    expect(f.promos.map((p) => p.tone)).toEqual([
      "success",
      "danger",
    ]);
    expect(f.official).toBe(true);
    expect(f.ratings).toEqual([
      { label: "豆瓣", value: "7.9" },
      { label: "TMDB", value: "8.2" },
    ]);
    expect(f.genres).toEqual(["剧情", "爱情"]);
    expect(f.region).toBe("中国大陆");
    expect(f.runtime).toBe(45);
    expect(f.director).toEqual(["导演A"]);
    expect(f.cast).toEqual(["演员B", "演员C"]);
  });

  it("无 meta/种子时不猜主创，评分仍从作品行取", () => {
    const f = workFlags(
      work({
        name: "Plain Movie",
        meta: null,
        seeds: [],
        rating: "6.5",
        douban_id: null,
      }),
      zhCN,
    );
    expect(f.qualities).toEqual([]);
    expect(f.promos).toEqual([]);
    expect(f.official).toBe(false);
    expect(f.genres).toEqual([]);
    expect(f.region).toBeNull();
    expect(f.director).toEqual([]);
    expect(f.cast).toEqual([]);
    expect(f.ratings).toEqual([{ label: "评分", value: "6.5" }]);
  });
});
