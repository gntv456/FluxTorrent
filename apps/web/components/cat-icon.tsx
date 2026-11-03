"use client";

/** CatIcon · 多色位图分类图标组件（V3 山海经套 / V4 Aurora 套承载层）。
 *
 * 为什么不进 icons.tsx：PATHS 是单色 currentColor 矢量口径；本组件承载的
 * 是 256px 位图（PNG 内嵌 SVG），颜色随套固定、不随 currentColor 变化，
 * 主题切换靠「换图标集」而非「换色值」。
 *
 * 解析链：categoryKey（categories.key，稳定标识）→ iconKey（旧 icon_key
 * 词表）→ cat-default →（加载失败）cat-other →（再失败）首字色块。
 *
 * 主题来源：默认读 <html data-theme>（与 layout.tsx no-flash 脚本同源），
 * 主题切换时经 MutationObserver 自动换套；测试/SSR 可传 theme 覆盖。
 *
 * 两态说明：active=true 切 <file>-active 资产（aurora 主色加深反白 /
 * 山海经金箔描边加亮，与默认态同剪影同尺寸）；active 文件加载失败
 * 先退同分类默认态文件，再走 cat-default → cat-other → 首字色块。 */

import { useEffect, useState } from "react";
import {
  catIconSetForTheme,
  catIconUrl,
  resolveCatIconFile,
  FALLBACK_FILE,
  FALLBACK_FILE_ALT,
} from "@/lib/cat-icon-sets";

export interface CatIconProps {
  /** 分类 key（categories.key）；暂无 key 的调用方可只传 iconKey */
  categoryKey?: string | null;
  /** 旧 icon_key（film/tv/music/...），次级解析路径 */
  iconKey?: string | null;
  /** 分类名：首字色块回退与 aria-label 用 */
  name?: string;
  /** 分类色（categories.bg_color）：首字色块回退的底色 */
  color?: string;
  /** 边长 px（主用位 22；筛选 chips 16） */
  size?: number;
  /** 选中态（暂无独立资产，用描边环表达） */
  active?: boolean;
  /** 主题覆盖；缺省跟随 <html data-theme> */
  theme?: string;
  /** 承载信息时传 title（自动 role=img + aria-label）；纯装饰缺省 aria-hidden */
  title?: string;
  className?: string;
}

function useDocTheme(override?: string): string {
  const [t, setT] = useState(() => override ?? "");
  useEffect(() => {
    if (override) {
      setT(override);
      return;
    }
    const el = document.documentElement;
    const read = () => setT(el.dataset.theme ?? "");
    read();
    const ob = new MutationObserver(read);
    ob.observe(el, { attributes: true, attributeFilter: ["data-theme"] });
    return () => ob.disconnect();
  }, [override]);
  return t;
}

export function CatIcon({
  categoryKey,
  iconKey,
  name,
  color,
  size = 22,
  active = false,
  theme,
  title,
  className,
}: CatIconProps) {
  const t = useDocTheme(theme);
  const set = catIconSetForTheme(t);
  const primary = resolveCatIconFile({ categoryKey, iconKey });
  // 加载失败回退链：（active 缺失先退同文件默认态）→ cat-default → cat-other → 首字色块
  const [stage, setStage] = useState<0 | 1 | 2>(0);
  const [exhausted, setExhausted] = useState(false);
  const [activeMiss, setActiveMiss] = useState(false);
  useEffect(() => {
    setStage(0);
    setExhausted(false);
    setActiveMiss(false);
  }, [primary, set, active]);

  const file = stage === 0 ? primary : stage === 1 ? FALLBACK_FILE : FALLBACK_FILE_ALT;
  const useActive = active && !activeMiss && stage === 0;

  if (exhausted) {
    return (
      <span
        aria-hidden={title ? undefined : true}
        role={title ? "img" : undefined}
        aria-label={title}
        className={className}
        style={{
          display: "inline-flex",
          alignItems: "center",
          justifyContent: "center",
          width: size,
          height: size,
          borderRadius: 4,
          background: color || "var(--text-muted)",
          color: "var(--text-on-accent)",
          fontSize: Math.max(11, Math.round(size * 0.55)),
          fontWeight: 700,
          lineHeight: 1,
        }}
      >
        {(name ?? "?").slice(0, 1)}
      </span>
    );
  }

  return (
    // 位图图标（多档 PNG 按 size 选取）；与 poster-image 一致沿用 <img>
    // eslint-disable-next-line @next/next/no-img-element
    <img
      src={catIconUrl(set, file, size, useActive)}
      width={size}
      height={size}
      alt={title ?? ""}
      aria-hidden={title ? undefined : true}
      role={title ? "img" : undefined}
      className={className}
      style={{
        display: "inline-block",
        borderRadius: Math.max(3, Math.round(size * 0.18)),
      }}
      onError={() => {
        if (useActive) setActiveMiss(true);
        else if (stage === 0 && primary !== FALLBACK_FILE) setStage(1);
        else if (stage <= 1 && file !== FALLBACK_FILE_ALT) setStage(2);
        else setExhausted(true);
      }}
    />
  );
}
