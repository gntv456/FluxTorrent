import type { ReactNode } from "react";

/** 页面头原语（S2 模板化 · 2026-10-16）
 *
 *  统一各页面手写的 `.pghd` 三层结构
 *  （眉标 eyebrow + 主标题 h1 + 可选副标题 / 右侧槽）。调用方只提供内容，
 *  结构与排版由原语决定，新增页面不再逐页复制这段 JSX。
 *  本轮迁移 17 页；games/fun、games/odds（裸 Link 右槽）与 help（内联 p
 *  副标题）结构特殊，暂留原样。
 *
 *  样式复用 layout.css 既有 `.pghd` / `.pg-eyebrow` / `.sub` / `.aside` 族
 *  （类名与视觉零变化；待 S3 收敛阶段迁入 ui.css）。
 *
 *  纯展示组件（无 hook），刻意不加 "use client"：RSC 与 Client 组件里都能
 *  直接用（同 Badge / Icon 口径）。
 *
 *  用法：
 *    <PageHeader eyebrow="Bank" title={dict.bank.title}
 *      sub={dict.bank.subtitle} />
 *    <PageHeader eyebrow="Medals" title={dict.medals.title}
 *      aside={<span className="pill">…</span>} />
 */
export function PageHeader({
  eyebrow,
  title,
  sub,
  aside,
}: {
  /** 眉标：西文小字大写（如 "Bank" / "Top Charts"）；不传则不渲染 */
  eyebrow?: string;
  /** 主标题：渲染为页面唯一 h1 */
  title: ReactNode;
  /** 副标题：弱化小字，可省 */
  sub?: ReactNode;
  /** 右侧槽：药丸 / 操作链接等，可省 */
  aside?: ReactNode;
}) {
  return (
    <div className="pghd">
      <div>
        {eyebrow ? <div className="pg-eyebrow">{eyebrow}</div> : null}
        <h1 className="font-display text-2xl">{title}</h1>
      </div>
      {sub ? <span className="sub">{sub}</span> : null}
      {aside ? <div className="aside">{aside}</div> : null}
    </div>
  );
}
