"use client";

import { useState } from "react";
import type { ReactNode } from "react";
import { Modal } from "@/components/modal";

/** 论坛首页手机端工具条（≤768px）：四个按钮一排，各自开一个弹窗。
 *
 *  按钮：筛选（显示当前档名）/ 版块 / 更多 / 搜帖。
 *  对应弹窗内容全部由服务端渲染好传进来（RSC 节点作 props）：
 *   - filter：四档筛选（全部话题/只看公告/只看置顶/只看精华）竖排选择；
 *   - nav：整条左列（用户卡 + 版块导航 + 我的）；
 *   - more：关注流/发主题两个按钮置顶 + 右列四块（统计/热帖/最新回复/必读）；
 *   - search：搜帖表单。
 *
 *  弹窗用站内标准件 Modal（portal 到 body，ESC/点遮罩关闭、锁滚动、
 *  焦点圈定都在里面）。桌面端整条工具条 display:none，只有 ≤768 可见。 */
export type MobilePanel = "filter" | "nav" | "more" | "search";

export function ForumMobileBar({
  filterLabel,
  labels,
  titles,
  panels,
}: {
  /** 筛选按钮上显示的当前档名（全部话题/只看公告/…） */
  filterLabel: string;
  labels: { nav: string; more: string; search: string };
  titles: Record<MobilePanel, string>;
  panels: Record<MobilePanel, ReactNode>;
}) {
  const [open, setOpen] = useState<MobilePanel | null>(null);
  const close = () => setOpen(null);

  const btn =
    "fh-mbar__b flex min-h-[36px] items-center justify-center gap-1 " +
    "rounded-full border border-line bg-[var(--brand-paper)] px-2 " +
    "text-xs font-bold text-ink active:scale-[0.98]";
  const btnOn =
    "fh-mbar__b flex min-h-[36px] items-center justify-center gap-1 " +
    "rounded-full border border-sky bg-[var(--sky-soft)] px-2 " +
    "text-xs font-bold text-sky-deep active:scale-[0.98]";

  return (
    <>
      <div className="fh-mbar">
        <button
          type="button"
          className={open === "filter" ? btnOn : btn}
          onClick={() => setOpen("filter")}
        >
          {filterLabel}
          <span aria-hidden="true">▾</span>
        </button>
        <button
          type="button"
          className={open === "nav" ? btnOn : btn}
          onClick={() => setOpen("nav")}
        >
          {labels.nav}
        </button>
        <button
          type="button"
          className={open === "more" ? btnOn : btn}
          onClick={() => setOpen("more")}
        >
          {labels.more}
        </button>
        <button
          type="button"
          className={open === "search" ? btnOn : btn}
          onClick={() => setOpen("search")}
        >
          {labels.search}
        </button>
      </div>

      <Modal
        open={open === "filter"}
        onClose={close}
        title={titles.filter}
        size="sm"
      >
        {panels.filter}
      </Modal>
      <Modal
        open={open === "nav"}
        onClose={close}
        title={titles.nav}
        size="sm"
      >
        <div className="fh-left">{panels.nav}</div>
      </Modal>
      <Modal
        open={open === "more"}
        onClose={close}
        title={titles.more}
        size="sm"
      >
        {panels.more}
      </Modal>
      <Modal
        open={open === "search"}
        onClose={close}
        title={titles.search}
        size="sm"
      >
        {panels.search}
      </Modal>
    </>
  );
}
