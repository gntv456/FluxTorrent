"use client";

/**
 * /gacha 图标 sprite（2026-11-27 从 gacha-stage.tsx 拆出守行数门禁）：
 * 设计稿 SVG path 表 + <Icon> 引用 + 页面根部一次性注入的 <Sprite>。
 * 纯静态件——path 数据前缀 gi- 防与站点图标重名。
 */

/** 长 SVG path 折行拼接，避免单行超宽 */
const j = (...p: string[]) => p.join("");

/* ---------- 图标（设计稿 sprite，前缀 gi- 防与站点图标重名） ---------- */
export const ICONS: Record<string, string> = {
  back: "M15 18l-6-6 6-6",
  ticket: j(
    "M3 9.5V8a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v1.5",
    "a2.5 2.5 0 0 0 0 5V16a2 2 0 0 1-2 2H5",
    "a2 2 0 0 1-2-2v-1.5a2.5 2.5 0 0 0 0-5Z",
    "|M14 7v1.6M14 11.2v1.6M14 15.4V17",
  ),
  shard: "M12 3l5 8-5 10-5-10z|M7 11h10",
  plus: "M12 5.5v13M5.5 12h13",
  info: "M12 11v5.2M12 7.9h.01",
  clock: "M12 7.5V12l3 1.8",
  arrow: "M9 6l6 6-6 6",
  chev: "M6 9.5l6 6 6-6",
  search: "M16 16l3.4 3.4",
  gift: j(
    "M3.2 9.6h17.6M12 9.6V20M12 9.6S9.6 4 7 5.4",
    "s0 4.2 5 4.2M12 9.6s2.4-5.6 5-4.2 0 4.2-5 4.2",
  ),
  task: "M8.4 12.2l2.6 2.6 4.6-5",
  bag: "M5 8h14l1 12H4z|M8.6 8V6.4a3.4 3.4 0 0 1 6.8 0V8",
  bank: j(
    "M3.6 9.6L12 4l8.4 5.6|M5.4 10.4v7.6M18.6 10.4v7.6",
    "M9.6 10.4v7.6M14.4 10.4v7.6M3 20.4h18",
  ),
  cards: j(
    "M3.4 6.4h10.4v14H3.4z|M8 6.4V4.4a1.8 1.8 0 0 1 1.8-1.8",
    "h9a1.8 1.8 0 0 1 1.8 1.8v10.6a1.8 1.8 0 0 1-1.8 1.8h-2.4",
  ),
  gauge: "M4.4 17.4a8.2 8.2 0 1 1 15.2 0|M12 17.4l4.2-5.4",
};

export function Icon({ name, size = 13 }: { name: string; size?: number }) {
  return (
    <svg
      width={size}
      height={size}
      aria-hidden="true"
      style={{ flex: "0 0 auto" }}
    >
      <use href={`#gi-${name}`} />
    </svg>
  );
}

export function Sprite() {
  const views: Record<string, string> = {
    info: "M12 22a10 10 0 1 0 0-20 10 10 0 0 0 0 20",
    clock: "M12 21.4a9.4 9.4 0 1 0 0-18.8 9.4 9.4 0 0 0 0 18.8",
    gauge: "M2.6 17.4a9.4 9.4 0 1 1 18.8 0",
    search: "M17.4 17.4a6.4 6.4 0 1 0 0-12.8 6.4 6.4 0 0 0 0 12.8",
    ticket: j(
      "M3 9.5V8a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v1.5a2.5 2.5",
      "0 0 0 0 5V16a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-1.5",
      "a2.5 2.5 0 0 0 0-5Z",
    ),
    shard: "M12 3l5 8-5 10-5-10z",
    task: "M4 4h16v16H4z",
    gift: "M4 9.6h16v10.4H4z",
  };
  return (
    <svg
      width="0"
      height="0"
      aria-hidden="true"
      style={{ position: "absolute" }}
    >
      {Object.entries(ICONS).map(([k, d]) => (
        <symbol
          key={k}
          id={`gi-${k}`}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.7"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          {d.split("|").map((seg, i) => (
            <path key={i} d={seg} />
          ))}
        </symbol>
      ))}
      {Object.entries(views).map(([k, d]) => (
        <symbol
          key={`v-${k}`}
          id={`gi-${k}`}
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          {k === "info" && <path d="M12 11v5.2M12 7.9h.01" />}
          {k === "clock" && <path d="M12 7.5V12l3 1.8" />}
          {k === "gauge" && (
            <>
              <path d={d} />
              <path d="M12 17.4l4.2-5.4" />
              <circle
                cx="12"
                cy="17.4"
                r="1.2"
                fill="currentColor"
                stroke="none"
              />
            </>
          )}
          {k === "ticket" && <path d="M14 7v1.6M14 11.2v1.6M14 15.4V17" />}
          {k === "shard" && (
            <>
              <path d={d} />
              <path d="M7 11h10" opacity=".5" />
            </>
          )}
          {k === "task" && (
            <>
              <path d={d} />
              <path d="M8.4 12.2l2.6 2.6 4.6-5" />
            </>
          )}
          {k === "gift" && <path d={d} />}
        </symbol>
      ))}
    </svg>
  );
}
