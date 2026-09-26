"use client";

import { useEffect, useState } from "react";

/** 媒体查询 hooks（移动端方案 M1）：组件层唯一的断点判定入口。
 *  SSR 安全：首渲染返回 fallback（默认桌面态 false），挂载后校正——
 *  与 #418 hydration 先例同纪律，宁可首帧多渲染一版桌面，不冒
 *  服务端/客户端 markup 不一致的风险。
 *
 *  断点语义见 base.css「断点体系」注释：<sm 手机 / md 折叠展开 /
 *  lg 平板 / xl 桌面。组件不要再自行 matchMedia 裸像素。 */

/** 通用媒体查询。fallback 必传（SSR 首帧值），避免调用方隐式依赖 false。 */
export function useMediaQuery(query: string, fallback: boolean): boolean {
  const [matches, setMatches] = useState(fallback);
  useEffect(() => {
    const mq = window.matchMedia(query);
    const update = () => setMatches(mq.matches);
    update();
    if (mq.addEventListener) {
      mq.addEventListener("change", update);
      return () => mq.removeEventListener("change", update);
    }
    mq.addListener(update);
    return () => mq.removeListener(update);
  }, [query]);
  return matches;
}

/** 紧凑态（<md，手机竖屏）：行卡片/底 Tab/抽屉的判定源。 */
export function useIsCompact(): boolean {
  // SSR 默认 false（桌面）：与首帧桌面渲染一致，挂载后校正。
  return useMediaQuery("(max-width: 767px)", false);
}

export type Posture = {
  /** 逻辑屏段数：1=普通设备，2=双折叠展开，3=三折叠全展。 */
  segments: 1 | 2 | 3;
  /** 是否处于折叠态（有铰链且合拢）：@media (folded) 判定。 */
  folded: boolean;
};

/** 形态感知（折叠屏/三折叠，M4 消费但 hook 先行落地）：
 *  - horizontal-viewport-segments（CSS Viewport Segments）：
 *    双折/三折展开的横向分段数；不支持时 matchMedia 返回 false → segments=1。
 *  - (folded)：铰链避让（layout.css .hinge-safe）启用条件。
 *  SSR 首帧 { segments: 1, folded: false }（桌面默认），挂载后校正；
 *  折展是连续 resize，消费方状态须 URL 化（方案 §4.4），本 hook 不承担状态保持。 */
export function usePosture(): Posture {
  const two = useMediaQuery(
    "(horizontal-viewport-segments: 2)",
    false,
  );
  const three = useMediaQuery(
    "(horizontal-viewport-segments: 3)",
    false,
  );
  const folded = useMediaQuery("(folded)", false);
  const segments = three ? 3 : two ? 2 : 1;
  return { segments: segments as 1 | 2 | 3, folded };
}
