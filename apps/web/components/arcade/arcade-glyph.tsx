/**
 * 娱乐屋线条图标本（样图①⑩口径）：彩块上是**白色线条图标**，
 * 不是彩色 emoji。stroke 用 currentColor —— 彩块里由 .gc-glyph 给白色，
 * 页头等浅底场景自动继承文字色。
 *
 * 纯哑件：只画形状，不参与任何结算；key 对齐 lib/games-registry 的游戏 key，
 * DB 下发的 RegRow 也带同名 key，找不到时回退九宫格格纹。
 */
const S = {
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 1.8,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
};

/** 每枚图标一个 24×24 视框；dots 类元素用 fill=currentColor 的实心点 */
const GLYPHS: Record<string, React.ReactNode> = {
  // 猜大小：圆角骰 + 2×2 点
  bigsmall: (
    <>
      <rect x="4" y="4" width="16" height="16" rx="4.5" {...S} />
      <circle cx="9.2" cy="9.2" r="1.3" fill="currentColor" />
      <circle cx="14.8" cy="9.2" r="1.3" fill="currentColor" />
      <circle cx="9.2" cy="14.8" r="1.3" fill="currentColor" />
      <circle cx="14.8" cy="14.8" r="1.3" fill="currentColor" />
    </>
  ),
  // 刮刮乐：票根（两侧缺口 + 中缝虚线）
  scratch: (
    <>
      <path
        d="M3 9.5V8a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v1.5a2.5 2.5 0 0 0 0 5V16a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-1.5a2.5 2.5 0 0 0 0-5Z"
        {...S}
      />
      <path d="M14 7v1.4M14 11.3v1.4M14 15.6V17" {...S} />
    </>
  ),
  // 九宫格：2×2 圆角格（样图①的格纹图标）
  jgg: (
    <>
      <rect x="4" y="4" width="7" height="7" rx="2" {...S} />
      <rect x="13" y="4" width="7" height="7" rx="2" {...S} />
      <rect x="4" y="13" width="7" height="7" rx="2" {...S} />
      <rect x="13" y="13" width="7" height="7" rx="2" {...S} />
    </>
  ),
  // 扭蛋机：蛋形
  capsule: (
    <path
      d="M12 3.5c-3.6 0-6.2 4.6-6.2 9.3a6.2 6.2 0 0 0 12.4 0c0-4.7-2.6-9.3-6.2-9.3Z"
      {...S}
    />
  ),
  // 大转盘：摩天轮（轮辐 + 中心毂）
  wheel: (
    <>
      <circle cx="12" cy="12" r="8.5" {...S} />
      <circle cx="12" cy="12" r="2.4" {...S} />
      <path
        d="M12 3.5v6.1M12 14.4v6.1M3.5 12h6.1M14.4 12h6.1M6 6l4.3 4.3M13.7 13.7 18 18M18 6l-4.3 4.3M10.3 13.7 6 18"
        {...S}
      />
    </>
  ),
  // 农场：双叶苗
  farm: (
    <>
      <path d="M12 21v-8.5" {...S} />
      <path d="M12 12.5C8.6 12.5 6.2 10.3 6 6.5c3.8.2 6 2.6 6 6Z" {...S} />
      <path d="M12 10.5c.2-3.4 2.5-5.6 6-5.8-.2 3.8-2.6 6-6 5.8Z" {...S} />
    </>
  ),
  // 抽卡：两张错叠卡
  gacha: (
    <>
      <rect x="3.5" y="7" width="12.5" height="14" rx="2.4" {...S} />
      <path d="M8 4h10a2.5 2.5 0 0 1 2.5 2.5V17" {...S} />
    </>
  ),
  // 宠物：爪印（三趾 + 掌垫）
  pet: (
    <>
      <circle cx="6.2" cy="9.6" r="1.7" {...S} />
      <circle cx="12" cy="7.4" r="1.7" {...S} />
      <circle cx="17.8" cy="9.6" r="1.7" {...S} />
      <path
        d="M12 12.2c-3.1 0-5.6 2.3-5.6 4.7 0 1.7 1.3 2.9 3 2.9 1 0 1.8-.4 2.6-.4s1.6.4 2.6.4c1.7 0 3-1.2 3-2.9 0-2.4-2.5-4.7-5.6-4.7Z"
        {...S}
      />
    </>
  ),
  // 钓鱼：鱼（尾在左 + 眼点）
  fishing: (
    <>
      <path
        d="M6.8 12c2-3.2 5.2-4.9 8.3-3.8 2 .7 3.9 2 5.4 3.8-1.5 1.8-3.4 3.1-5.4 3.8-3.1 1.1-6.3-.6-8.3-3.8Z"
        {...S}
      />
      <path d="M6.8 12 3 9.4v5.2Z" {...S} />
      <circle cx="16.4" cy="11" r="1" fill="currentColor" />
    </>
  ),
};

export function ArcadeGlyph({ k }: { k: string }) {
  return (
    <svg
      viewBox="0 0 24 24"
      width="22"
      height="22"
      aria-hidden
      focusable="false"
    >
      {GLYPHS[k] ?? GLYPHS.jgg}
    </svg>
  );
}
