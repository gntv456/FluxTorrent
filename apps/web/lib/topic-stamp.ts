/** 论坛话题行的 MM-DD HH:mm 时间格式（样图话题行口径）。
 *
 *  为什么住在 lib 而不是 forum-topic-row.tsx：行组件是 "use client"，
 *  而首页话题流（RSC 页面）也要用同一格式——从 client 模块导入函数
 *  会被 rsc_boundary_guard 拦（服务端拿到的是 client reference）。
 *  抽进无指令的中性模块，两边各自导入（同 rel-time.ts 的先例）。
 *
 *  本地时区渲染：容器跑 UTC、浏览器在本地时区，两端文本必然不同——
 *  这是有意的差异（与站内其它时间列同口径），渲染处须带
 *  suppressHydrationWarning。 */
export function topicStamp(iso: string | null | undefined) {
  if (!iso) return null;
  const d = new Date(iso);
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(d.getMonth() + 1)}-${p(d.getDate())} ${p(d.getHours())}:${p(
    d.getMinutes(),
  )}`;
}
