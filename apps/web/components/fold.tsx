"use client";

import { useState } from "react";

/** 折叠分区（同类站通用口径：默认收起，summary 带计数）。
 *
 *  2026-11-11 提速批改为客户端门卫件：body 只在「初始 open 或用户展开过」
 *  后才挂载。此前 body 是无条件渲染的——收起的 <details> 里 Peers 这类
 *  「挂载即拉 API」的组件照样发请求、FileTree 照样进 eager 包。门卫化后：
 *  - 收起态：零请求、零 chunk 下载（重内容配 torrent-detail-lazy 的
 *    dynamic 懒壳，展开瞬间才拉代码）
 *  - 展开过就保持挂载：收合再展开不重复拉取、不丢滚动位置
 *  - props 驱动的纯文本 body（NFO/MediaInfo/日志）不受影响：数据本就在
 *    flight 里，门卫只是推迟渲染时机
 *
 *  锚点语义不变：id 挂在 <details> 上，跳转定位到 summary；body 在用户
 *  点开瞬间渲染（动态件先出一帧骨架）。 */
export function Fold({
  title,
  count,
  children,
  open,
  id,
}: {
  title: string;
  count?: number;
  children: React.ReactNode;
  open?: boolean;
  /** 锚点 id（侧栏「查看全部」定位目标，scroll-margin-top 见 base.css） */
  id?: string;
}) {
  const [revealed, setRevealed] = useState(!!open);
  return (
    <details
      className="td-fold"
      open={open}
      id={id}
      onToggle={(e) => {
        if ((e.target as HTMLDetailsElement).open) setRevealed(true);
      }}
    >
      <summary>
        <h2>{title}</h2>
        {count !== undefined && (
          <span className="td-fold__count num">{count}</span>
        )}
        <span className="td-fold__chev" aria-hidden />
      </summary>
      <div className="td-fold__body">{revealed ? children : null}</div>
    </details>
  );
}
