/**
 * FluxTorrent · TIDE 图标系统（P3）
 *
 * 设计规格（新增图标前必读）：
 *  - 画布 24×24 / 安全区 2px；线性描边 fill=none、stroke=currentColor、默认 1.5px
 *  - linecap=round / linejoin=round —— 与主题「细描边 + hairline」语言一致
 *  - **不内置颜色**：颜色一律继承 currentColor，故浅色/夜间主题自动适配
 *  - 命名走业务语义（`preserve` 保种、`resurrect` 断种复活），不走形状
 *
 * 用法：
 *   <Icon name="seed" />                    // 默认 20
 *   <Icon name="download" size={16} />
 *   <Icon name="spark" className="text-[var(--sky)]" />
 *   <Icon name="preserve" title="认领保种" /> // 承载信息时传 title（自动 role=img + <title>）
 *
 * 这是纯展示组件（无 hook / 无事件），**刻意不加 "use client"**：
 * 这样它在 RSC（如 layout.tsx 顶栏与底部 TabBar）里按服务端组件渲染，
 * 在 "use client" 组件（如 theme-toggle）里也能正常被打包使用。
 */
import type { SVGProps } from "react";

const PATHS: Record<string, React.ReactNode> = {
  /* ---------- 导航 / 核心资源 ---------- */
  home: (
    <>
      <path d="M3.5 10.6 12 3.5l8.5 7.1" />
      <path d="M5.5 9.4V20h13V9.4" />
      <path d="M9.6 20v-5.6h4.8V20" />
    </>
  ),
  seed: (
    <>
      <path d="M12 3.2c3.4 3.9 4.9 6.5 4.9 8.9a4.9 4.9 0 0 1-9.8 0c0-2.4 1.5-5 4.9-8.9Z" />
      <path d="M12 21v-3.6" />
      <path d="M12 17.4c-1.7 0-3-1.2-3-2.8 1.7 0 3 1.2 3 2.8Z" />
    </>
  ),
  library: (
    <>
      <path d="M4 4.5v15" />
      <path d="M8.2 4.5v15" />
      <path d="M4 7.2 8.2 6l1 13.2L4 19.8z" />
      <path d="M13 6.4h7v12h-7z" />
      <path d="M13 10.3h7" />
    </>
  ),
  forums: (
    <>
      <path d="M3.6 5.4h11.2v8.4H8.4L3.6 17V5.4Z" />
      <path d="M20.4 9.2h-4.2v5.6" />
      <path d="M6.8 9.2h5.2M6.8 12h3.4" />
    </>
  ),
  topic: (
    <>
      <path d="M4 5.6h16v9.6H9.6L4 18.6V5.6Z" />
      <path d="M8 9h8M8 12h5" />
    </>
  ),
  pin: (
    <>
      <path d="M9 3.6h6l-1 5.2 3.4 2.6v1.6H5.6v-1.6L9 8.8z" />
      <path d="M12 13v7.4" />
    </>
  ),
  ranking: (
    <>
      <path d="M4.5 20h15" />
      <path d="M7.2 20v-6.4M12 20V5.6M16.8 20v-9.2" />
      <path d="M12 5.6 8.6 9h6.8z" />
    </>
  ),
  upload: (
    <>
      <path d="M12 16.4V4.6" />
      <path d="M7.4 9.2 12 4.6l4.6 4.6" />
      <path d="M4.4 15.6v3.2h15.2v-3.2" />
    </>
  ),
  download: (
    <>
      <path d="M12 4.6v11.8" />
      <path d="M7.4 11.8 12 16.4l4.6-4.6" />
      <path d="M4.4 15.6v3.2h15.2v-3.2" />
    </>
  ),
  search: (
    <>
      <circle cx="10.6" cy="10.6" r="6.1" />
      <path d="m15.2 15.2 4.6 4.6" />
    </>
  ),
  filter: <path d="M3.6 5.4h16.8l-6.6 7.8v5.6l-3.6-2.2v-3.4z" />,
  sort: (
    <>
      <path d="M7 4.6v15" />
      <path d="M4.2 16.6 7 19.6l2.8-3" />
      <path d="M12.6 7.4h7.2M12.6 12h5M12.6 16.6h3" />
    </>
  ),
  magnet: (
    <>
      <path d="M5.8 4.4v7.2a6.2 6.2 0 0 0 12.4 0V4.4" />
      <path d="M5.8 4.4h3.6v7.2a2.6 2.6 0 0 1-5.2 0z" />
      <path d="M14.6 4.4h3.6v7.2a2.6 2.6 0 0 1-5.2 0z" />
    </>
  ),
  disc: (
    <>
      <circle cx="12" cy="12" r="8.2" />
      <circle cx="12" cy="12" r="2.4" />
    </>
  ),
  file: (
    <>
      <path d="M6.2 3.6h7.4l4.2 4.2v12.6H6.2z" />
      <path d="M13.4 3.6v4.4h4.4" />
      <path d="M9 13h6M9 16.4h4" />
    </>
  ),
  folder: (
    <>
      <path d="M3.6 6.4h5.6l1.8 2.2h9.4V19H3.6z" />
      <path d="M3.6 6.4V19" />
    </>
  ),
  subtitle: (
    <>
      <rect x="3.2" y="5.2" width="17.6" height="13.6" rx="2.4" />
      <path d="M9 10.4a2.6 2.6 0 1 0 0 3.2" />
      <path d="M15.4 10.4a2.6 2.6 0 1 0 0 3.2" />
    </>
  ),
  request: (
    <>
      <path d="M4.2 9.4v5.2h3l5.4 3.4V6L7.2 9.4z" />
      <path d="M16.4 9.2a4 4 0 0 1 0 5.6" />
      <path d="M19 6.8a7.4 7.4 0 0 1 0 10.4" />
    </>
  ),
  offer: (
    <>
      <path d="M4.4 8.6h15.2l-1.1 10.8H5.5z" />
      <path d="M3.4 5.6h17.2v3H3.4z" />
      <path d="M12 5.6v13.8" />
      <path d="M12 5.6C10.6 3.8 8.8 4.6 9.4 5.6 10 6.6 11.4 6.2 12 5.6Z" />
      <path d="M12 5.6c1.4-1.8 3.2-1 2.6 0-.6 1-2 .6-2.6 0Z" />
    </>
  ),
  preserve: (
    <>
      <path d="M12 3.4 19 6v5.8c0 4.2-2.9 6.8-7 8.8-4.1-2-7-4.6-7-8.8V6z" />
      <path d="M9.2 11.8 11.4 14l3.6-3.8" />
    </>
  ),
  resurrect: <path d="M3.2 12.4h3.6l1.8-4.2 3 8.4 1.9-4.2h7.3" />,
  endangered: (
    <>
      <path d="M12 3.2c2.6 2.6 4.4 4.8 4.4 7.6a4.4 4.4 0 0 1-8.8 0c0-1.6.8-3 2.2-4.6" />
      <path d="M12 20.8a8.4 8.4 0 0 0 8.4-8.4" />
      <path d="M4.6 12.4a8.4 8.4 0 0 0 1.6 4.9" />
    </>
  ),
  textbook: (
    <>
      <path d="M4.4 5.2h6.2a2.4 2.4 0 0 1 2.4 2.4V19a2 2 0 0 0-2-2H4.4z" />
      <path d="M19.6 5.2h-6.2A2.4 2.4 0 0 0 11 7.6V19a2 2 0 0 1 2-2h6.6z" />
    </>
  ),

  /* ---------- 经济 ---------- */
  shop: (
    <>
      <path d="M5 8.4h14l-1.2 11.2H6.2z" />
      <path d="M9 8.4V6.6a3 3 0 0 1 6 0v1.8" />
    </>
  ),
  bank: (
    <>
      <path d="M3.4 9.2 12 4.4l8.6 4.8" />
      <path d="M5.6 10.4V18M10 10.4V18M14 10.4V18M18.4 10.4V18" />
      <path d="M3.4 20.4h17.2" />
    </>
  ),
  spark: (
    <>
      <path d="M12 3.4 13.7 9 19.4 10.7 13.7 12.4 12 18.1 10.3 12.4 4.6 10.7 10.3 9z" />
      <path d="M18.2 4.2v3M19.7 5.7h-3" />
    </>
  ),
  pool: (
    <>
      <path d="M12 3.6c2.6 3 3.8 5 3.8 6.8a3.8 3.8 0 0 1-7.6 0c0-1.8 1.2-3.8 3.8-6.8Z" />
      <path d="M4.6 15.4c1.6-1.4 3-1.4 4.6 0s3 1.4 4.6 0 3-1.4 4.6 0" />
      <path d="M4.6 19c1.6-1.4 3-1.4 4.6 0s3 1.4 4.6 0 3-1.4 4.6 0" />
    </>
  ),
  donate: (
    <path d="M12 20.2S3.8 15.4 3.8 9.6A4.4 4.4 0 0 1 12 7.4a4.4 4.4 0 0 1 8.2 2.2c0 5.8-8.2 10.6-8.2 10.6Z" />
  ),
  medal: (
    <>
      <circle cx="12" cy="9.4" r="5.4" />
      <path d="M9.4 14.2 7.6 21l4.4-2.4L16.4 21l-1.8-6.8" />
    </>
  ),
  trophy: (
    <>
      <path d="M8 4.4h8v4.8a4 4 0 0 1-8 0z" />
      <path d="M8 6.2H5.4v1.6a3 3 0 0 0 3 3M16 6.2h2.6v1.6a3 3 0 0 1-3 3" />
      <path d="M12 13.2v3.4M9 20.4h6M10.2 20.4 12 16.6l1.8 3.8" />
    </>
  ),
  task: (
    <>
      <rect x="4.4" y="4" width="15.2" height="16" rx="2.4" />
      <path d="M8 9.6l1.6 1.6 3-3.2" />
      <path d="M10.6 15h5" />
    </>
  ),
  gift: (
    <>
      <rect x="3.8" y="8.6" width="16.4" height="4" rx="1" />
      <path d="M5.4 12.6V20h13.2v-7.4" />
      <path d="M12 8.6v11.4" />
    </>
  ),
  voucher: (
    <>
      <path d="M3.6 7.6h16.8v3.2a1.6 1.6 0 0 0 0 3.2v3.2H3.6v-3.2a1.6 1.6 0 0 0 0-3.2z" />
      <path d="M12 7.6v10.8" />
    </>
  ),
  dressup: (
    <>
      <path d="M12 5.6a2.4 2.4 0 1 0 0-4.8" />
      <path d="M12 5.6 6.6 9.4 4.4 19.6h15.2L17.4 9.4z" />
    </>
  ),
  frame: (
    <>
      <path d="M4.4 4.4h15.2v15.2H4.4z" />
      <path d="M4.4 4.4 19.6 19.6M19.6 4.4 4.4 19.6" />
    </>
  ),

  /* ---------- 成长 ---------- */
  exam: (
    <>
      <path d="M5.6 3.8h12.8v16.4H5.6z" />
      <path d="M9 8.4h6M9 12h6" />
      <path d="M9.4 16.6l1.4 1.4 2.8-3" />
    </>
  ),
  gauge: (
    <>
      <path d="M4 17.6a8.6 8.6 0 0 1 16 0" />
      <path d="M12 17.6 16 10.8" />
      <circle cx="12" cy="17.6" r="1.3" />
    </>
  ),
  invite: (
    <>
      <rect x="3.4" y="5.6" width="17.2" height="12.8" rx="2" />
      <path d="m3.8 6.4 8.2 6 8.2-6" />
    </>
  ),
  level: (
    <>
      <path d="M3.6 19.4h5.6v-4h5.6v-4h5.6v-6" />
      <path d="M20.4 5.4h-5.6" />
    </>
  ),

  /* ---------- 社交 / 管理 ---------- */
  messages: (
    <>
      <path d="M3.6 6.4h16.8v10.4H9.6L4.6 20v-3.2H3.6z" />
      <path d="M7.4 10h9.2M7.4 13h5.6" />
    </>
  ),
  mail: (
    <>
      <rect x="3.2" y="5.4" width="17.6" height="13.2" rx="2.2" />
      <path d="m3.8 6.6 8.2 5.8 8.2-5.8" />
    </>
  ),
  friends: (
    <>
      <circle cx="9" cy="8.6" r="3.2" />
      <path d="M3.4 19.6c0-3.1 2.5-5.4 5.6-5.4s5.6 2.3 5.6 5.4" />
      <path d="M16 6.2a3.2 3.2 0 0 1 0 6" />
      <path d="M17.4 14.6c2 .7 3.4 2.6 3.4 5" />
    </>
  ),
  teams: (
    <>
      <circle cx="12" cy="7.4" r="3" />
      <path d="M6.6 19.6c0-3 2.4-5.2 5.4-5.2s5.4 2.2 5.4 5.2" />
      <path d="M4.4 9.2a2.4 2.4 0 1 0 0-4.8" />
      <path d="M2.6 18.4c0-2.4 1.6-4.2 3.8-4.6" />
      <path d="M19.6 9.2a2.4 2.4 0 1 1 0-4.8" />
      <path d="M21.4 18.4c0-2.4-1.6-4.2-3.8-4.6" />
    </>
  ),
  user: (
    <>
      <circle cx="12" cy="8.2" r="3.8" />
      <path d="M4.8 20.2c0-4 3.2-6.8 7.2-6.8s7.2 2.8 7.2 6.8" />
    </>
  ),
  users: (
    <>
      <circle cx="9.4" cy="8.4" r="3.2" />
      <path d="M3.2 19.4c0-3.4 2.8-5.8 6.2-5.8s6.2 2.4 6.2 5.8" />
      <path d="M16.2 5.8a3.2 3.2 0 0 1 0 6" />
      <path d="M17.4 14c2.6.6 4.4 2.6 4.4 5.4" />
    </>
  ),
  staff: (
    <>
      <rect x="4.4" y="8.6" width="15.2" height="11" rx="2" />
      <path d="M9 4.4 12 8.6l3-4.2" />
      <circle cx="12" cy="13" r="2.1" />
      <path d="M9 17.4c.9-.9 1.8-1.3 3-1.3s2.1.4 3 1.3" />
    </>
  ),
  shield: (
    <path d="M12 3.4 19 6v5.8c0 4.2-2.9 6.8-7 8.8-4.1-2-7-4.6-7-8.8V6z" />
  ),
  settings: (
    <>
      <circle cx="12" cy="12" r="3.2" />
      <path d="M12 3.6v3M12 17.4v3M3.6 12h3M17.4 12h3M6 6l2.1 2.1M15.9 15.9 18 18M18 6l-2.1 2.1M8.1 15.9 6 18" />
    </>
  ),

  /* ---------- 娱乐 ---------- */
  game: (
    <>
      <rect x="2.8" y="7.6" width="18.4" height="9.6" rx="4.8" />
      <path d="M7.4 10.6v3.4M5.7 12.3h3.4" />
      <circle cx="15.6" cy="11.4" r="1" />
      <circle cx="18" cy="14" r="1" />
    </>
  ),
  farm: (
    <>
      <path d="M12 20.4V8.6" />
      <path d="M12 8.6c-2.8 0-4.6-1.8-4.6-4.6 2.8 0 4.6 1.8 4.6 4.6Z" />
      <path d="M12 12.4c2.6 0 4.2-1.6 4.2-4.2-2.6 0-4.2 1.6-4.2 4.2Z" />
      <path d="M12 16c2.4 0 4-1.6 4-4-2.4 0-4 1.6-4 4Z" />
    </>
  ),
  gomoku: (
    <>
      <path d="M4.4 4.4h15.2v15.2H4.4z" />
      <path d="M8.6 4.4v15.2M12 4.4v15.2M15.4 4.4v15.2" />
      <path d="M4.4 8.6h15.2M4.4 12h15.2M4.4 15.4h15.2" />
      <circle cx="8.6" cy="8.6" r="1.6" fill="currentColor" stroke="none" />
      <circle cx="15.4" cy="15.4" r="1.6" />
    </>
  ),
  dice: (
    <>
      <rect x="4" y="4" width="16" height="16" rx="3.4" />
      <circle cx="8.6" cy="8.6" r="1.1" fill="currentColor" stroke="none" />
      <circle cx="15.4" cy="15.4" r="1.1" fill="currentColor" stroke="none" />
      <circle cx="12" cy="12" r="1.1" fill="currentColor" stroke="none" />
    </>
  ),
  scratch: (
    <>
      <rect x="3.4" y="5.6" width="17.2" height="12.8" rx="2" />
      <path d="M6.6 14.6c2-2.6 3.4 1.4 5.4-1.2s3 2 5.4-.6" />
      <path d="M6.6 9.2h4" />
    </>
  ),
  contest: (
    <>
      <path d="M6.4 3.6v16.8" />
      <path d="M6.4 5.2h11.2l-2.4 3.4 2.4 3.4H6.4" />
      <path d="M4.2 20.4h4.4" />
    </>
  ),

  /* ---------- 数据 / 状态 ---------- */
  chartBar: (
    <>
      <path d="M4 20h16" />
      <path d="M7 20v-5.6M11.4 20V8.6M15.8 20v-8.4" />
    </>
  ),
  chartLine: (
    <>
      <path d="M4 20h16" />
      <path d="m5.4 16.4 4-4.6 3.2 2.6 5.6-6.4" />
      <path d="M18.2 8v4.2h-4.2" />
    </>
  ),
  ratio: (
    <>
      <path d="M12 4.4v15.2" />
      <path d="M5.6 8.4h12.8" />
      <path d="M5.6 8.4 3.2 13h4.8z" />
      <path d="M18.4 8.4 16 13h4.8z" />
      <path d="M8.6 19.6h6.8" />
    </>
  ),
  peers: (
    <>
      <circle cx="12" cy="5.6" r="2.2" />
      <circle cx="5.6" cy="16.4" r="2.2" />
      <circle cx="18.4" cy="16.4" r="2.2" />
      <path d="M10.4 7.5 7.2 14.6M13.6 7.5l3.2 7.1M7.8 16.4h8.4" />
    </>
  ),
  speed: (
    <>
      <path d="M4.4 17.6a8.2 8.2 0 0 1 15.2 0" />
      <path d="M12 17.6 16.4 9.8" />
      <path d="M4.4 17.6h15.2" />
    </>
  ),
  clock: (
    <>
      <circle cx="12" cy="12" r="8.2" />
      <path d="M12 7.4V12l3.4 2" />
    </>
  ),
  refresh: (
    <>
      <path d="M20 12a8 8 0 1 1-2.6-5.9" />
      <path d="M20 4.4V9h-4.6" />
    </>
  ),
  trendUp: (
    <>
      <path d="m4 16.4 5.2-5.2 3.2 3.2L20 7.2" />
      <path d="M15.4 7.2H20v4.6" />
    </>
  ),
  trendDown: (
    <>
      <path d="m4 7.6 5.2 5.2 3.2-3.2L20 16.8" />
      <path d="M15.4 16.8H20v-4.6" />
    </>
  ),

  /* ---------- 通用动作 ---------- */
  check: <path d="m5 12.6 4.6 4.6L19 7.2" />,
  checkCircle: (
    <>
      <circle cx="12" cy="12" r="8.4" />
      <path d="m8.4 12.2 2.6 2.6 4.8-5" />
    </>
  ),
  close: <path d="M6.4 6.4l11.2 11.2M17.6 6.4 6.4 17.6" />,
  plus: <path d="M12 5.4v13.2M5.4 12h13.2" />,
  minus: <path d="M5.4 12h13.2" />,
  edit: (
    <>
      <path d="M4.6 19.4h3.4l10-10-3.4-3.4-10 10z" />
      <path d="M14.6 5.6 18.4 9.4" />
    </>
  ),
  trash: (
    <>
      <path d="M4.8 7.4h14.4" />
      <path d="M9.4 7.4V5.2h5.2v2.2" />
      <path d="M6.6 7.4l1 12.2h8.8l1-12.2" />
      <path d="M10.4 11v5.6M13.6 11v5.6" />
    </>
  ),
  eye: (
    <>
      <path d="M2.8 12S6.4 6.4 12 6.4 21.2 12 21.2 12 17.6 17.6 12 17.6 2.8 12 2.8 12Z" />
      <circle cx="12" cy="12" r="2.8" />
    </>
  ),
  eyeOff: (
    <>
      <path d="M4 4l16 16" />
      <path d="M9.6 6.9A9.7 9.7 0 0 1 12 6.4c5.6 0 9.2 5.6 9.2 5.6a17 17 0 0 1-2.6 3.3" />
      <path d="M6.3 8.4A16.7 16.7 0 0 0 2.8 12S6.4 17.6 12 17.6c1 0 1.9-.2 2.8-.4" />
    </>
  ),
  star: (
    <path d="m12 4 2.5 5.3 5.5.7-4 3.9 1 5.5-5-2.9-5 2.9 1-5.5-4-3.9 5.5-.7z" />
  ),
  bell: (
    <>
      <path d="M6.4 10.4a5.6 5.6 0 0 1 11.2 0c0 4.2 1.6 5.8 1.6 5.8H4.8s1.6-1.6 1.6-5.8Z" />
      <path d="M10 19.2a2.2 2.2 0 0 0 4 0" />
    </>
  ),
  lock: (
    <>
      <rect x="5.2" y="10.4" width="13.6" height="9.6" rx="2.2" />
      <path d="M8.4 10.4V7.8a3.6 3.6 0 0 1 7.2 0v2.6" />
    </>
  ),
  warning: (
    <>
      <path d="M12 4.4 21 19.6H3z" />
      <path d="M12 10v4.2M12 17.2h.01" />
    </>
  ),
  info: (
    <>
      <circle cx="12" cy="12" r="8.4" />
      <path d="M12 11v5.2M12 8.2h.01" />
    </>
  ),
  help: (
    <>
      <circle cx="12" cy="12" r="8.4" />
      <path d="M9.8 9.6a2.3 2.3 0 1 1 3.4 2c-.8.5-1.2 1-1.2 1.9" />
      <path d="M12 17h.01" />
    </>
  ),
  copy: (
    <>
      <rect x="8.6" y="8.6" width="11" height="11" rx="2" />
      <path d="M15.4 8.6V6.4a2 2 0 0 0-2-2H6.4a2 2 0 0 0-2 2v7a2 2 0 0 0 2 2h2.2" />
    </>
  ),
  link: (
    <>
      <path d="M10 13.8a3.6 3.6 0 0 0 5.1 0l2.8-2.8a3.6 3.6 0 0 0-5.1-5.1l-1 1" />
      <path d="M14 10.2a3.6 3.6 0 0 0-5.1 0l-2.8 2.8a3.6 3.6 0 0 0 5.1 5.1l1-1" />
    </>
  ),
  external: (
    <>
      <path d="M14 4.6h5.4V10" />
      <path d="M19.4 4.6 11.6 12.4" />
      <path d="M18 14.6v4.8H4.6V6h4.8" />
    </>
  ),
  rss: (
    <>
      <path d="M5 5.6a13 13 0 0 1 13 13" />
      <path d="M5 11.4a7.2 7.2 0 0 1 7.2 7.2" />
      <circle cx="5.8" cy="18.4" r="1.3" fill="currentColor" stroke="none" />
    </>
  ),
  more: (
    <>
      <circle cx="6" cy="12" r="1.4" fill="currentColor" stroke="none" />
      <circle cx="12" cy="12" r="1.4" fill="currentColor" stroke="none" />
      <circle cx="18" cy="12" r="1.4" fill="currentColor" stroke="none" />
    </>
  ),
  menu: <path d="M4 7h16M4 12h16M4 17h16" />,
  chevronDown: <path d="m6.6 9.8 5.4 5.4 5.4-5.4" />,
  chevronRight: <path d="m9.8 6.6 5.4 5.4-5.4 5.4" />,
  arrowUp: <path d="M12 19V5.6M6.6 11 12 5.6 17.4 11" />,
  arrowDown: <path d="M12 5v13.4M6.6 13 12 18.6 17.4 13" />,
  logout: (
    <>
      <path d="M14.6 4.6H6.6a2 2 0 0 0-2 2v10.8a2 2 0 0 0 2 2h8" />
      <path d="M16.6 8.6 20 12l-3.4 3.4M20 12h-9" />
    </>
  ),
  sun: (
    <>
      <circle cx="12" cy="12" r="4.2" />
      <path d="M12 3v2.4M12 18.6V21M3 12h2.4M18.6 12H21M5.6 5.6l1.7 1.7M16.7 16.7l1.7 1.7M18.4 5.6l-1.7 1.7M7.3 16.7l-1.7 1.7" />
    </>
  ),
  moon: <path d="M19.4 14.6A8 8 0 0 1 9.4 4.6a8.4 8.4 0 1 0 10 10Z" />,
  globe: (
    <>
      <circle cx="12" cy="12" r="8.4" />
      <path d="M3.6 12h16.8" />
      <path d="M12 3.6c2.4 2.4 3.6 5.2 3.6 8.4s-1.2 6-3.6 8.4c-2.4-2.4-3.6-5.2-3.6-8.4S9.6 6 12 3.6Z" />
    </>
  ),
};

export type IconName = keyof typeof PATHS;

export interface IconProps extends Omit<SVGProps<SVGSVGElement>, "name"> {
  /** 图标名（见 PATHS）；未注册的名字静默返回 null，不抛错打断页面 */
  name: IconName | string;
  /** 边长 px，默认 20 */
  size?: number;
  /** 描边宽度，默认 1.5 */
  strokeWidth?: number;
  /** 承载信息时传入：渲染 <title> 并以 role="img" 暴露；不传则 aria-hidden */
  title?: string;
}

export function Icon({
  name,
  size = 20,
  strokeWidth = 1.5,
  title,
  className = "",
  ...rest
}: IconProps) {
  const glyph = PATHS[name];
  if (!glyph) return null;
  return (
    <svg
      viewBox="0 0 24 24"
      width={size}
      height={size}
      fill="none"
      stroke="currentColor"
      strokeWidth={strokeWidth}
      strokeLinecap="round"
      strokeLinejoin="round"
      className={className}
      aria-hidden={title ? undefined : true}
      role={title ? "img" : undefined}
      focusable="false"
      {...rest}
    >
      {title ? <title>{title}</title> : null}
      {glyph}
    </svg>
  );
}

/** 已注册图标名（供设计稿对照与单测枚举，防拼错） */
export const ICON_NAMES = Object.keys(PATHS) as IconName[];

export default Icon;
