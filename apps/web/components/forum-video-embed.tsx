"use client";

/**
 * 论坛视频内嵌 V1（0189）：白名单 embed 渲染器。
 * 安全模型（策划案 §3.2）：
 *  - 用户正文永远产不出任意 iframe src——只有命中白名单规则的 URL 才生成播放器；
 *  - 生成后二次校验 src 必须落在该规则 embed_origin 前缀内（纵深防御）；
 *  - 未命中 / 校验失败 → 降级为普通链接（fail-closed iframe / fail-open 链接）；
 *  - 两段式懒加载：首屏只渲染封面占位，点击才挂 iframe（一页多视频不拖慢首屏）。
 * 规则来自 GET /api/v1/forums/embed-rules（总开关关闭时后端返回空数组，
 * 全部视频语法自动降级为链接）。
 */
import { useState } from "react";
import type { EmbedRule } from "@/lib/data-forum";
import { useI18n } from "@/i18n/client";

const ASPECT_CLASS: Record<string, string> = {
  "16:9": "aspect-video",
  "4:3": "aspect-[4/3]",
  "1:1": "aspect-square",
};

/** 把模板 $1..$9 替换为正则捕获组。 */
function fillTemplate(
  tpl: string,
  m: RegExpMatchArray,
  extra: string | null | undefined,
): string {
  let s = tpl.replace(/\$([1-9])/g, (_, d) => m[Number(d)] ?? "");
  if (extra) {
    const sep = extra.startsWith("&") || extra.startsWith("?") ? "" : "?";
    s += sep + extra;
  }
  return s;
}

/** 命中测试：返回 (规则, 匹配结果)。正则来自服务端白名单表（保存时已编译校验）。 */
export function matchEmbedRule(
  url: string,
  rules: EmbedRule[] | null,
): { rule: EmbedRule; src: string } | null {
  if (!rules || rules.length === 0 || !url) return null;
  for (const rule of rules) {
    let re: RegExp;
    try {
      re = new RegExp(rule.url_pattern, "i");
    } catch {
      continue; // 坏规则跳过（保存侧已拦，运行时兜底）
    }
    const m = url.match(re);
    if (!m) continue;
    const src = fillTemplate(rule.embed_template, m, rule.extra_params);
    // 二次校验：生成物必须落在 embed_origin 前缀内，否则整条降级
    if (!src.startsWith(rule.embed_origin)) return null;
    return { rule, src };
  }
  return null;
}

/** 两段式懒加载 iframe 占位（点击后才加载平台脚本）。 */
function LazyIframe({
  src,
  aspect,
  name,
  href,
}: {
  src: string;
  aspect: string;
  name: string;
  href: string;
}) {
  const { dict } = useI18n();
  const t = dict.forums;
  const [play, setPlay] = useState(false);
  const R = "relative my-3 w-full overflow-hidden rounded-[var(--r-md)] " +
    "border border-line bg-[var(--surface-sunken)]";
  const box = `${ASPECT_CLASS[aspect] ?? "aspect-video"} ${R}`;
  if (play) {
    return (
      <div className={box}>
        <iframe
          src={src}
          title={name}
          className="absolute inset-0 h-full w-full"
          sandbox="allow-scripts allow-same-origin allow-presentation"
          loading="lazy"
          referrerPolicy="strict-origin-when-cross-origin"
          allowFullScreen
        />
      </div>
    );
  }
  const PLAY_BTN =
    "flex h-12 w-12 items-center justify-center rounded-full " +
    "bg-[var(--sky)] text-xl text-white shadow-[var(--shadow-hover)] " +
    "transition group-hover:scale-110";
  const ORIGIN_LINK =
    "absolute right-2 top-2 rounded-full bg-black/40 px-2 py-0.5 " +
    "text-[10px] text-white/90 hover:bg-black/60";
  const CLICK_AREA =
    "group absolute inset-0 flex w-full flex-col items-center " +
    "justify-center gap-2";
  return (
    <div className={box}>
      <button
        type="button"
        onClick={() => setPlay(true)}
        aria-label={t.videoPlay.replace("{name}", name)}
        className={CLICK_AREA}
      >
        <span className={PLAY_BTN}>▶</span>
        <span className="max-w-[90%] truncate px-2 text-xs font-bold text-sub">
          {name} · {t.videoPlay}
        </span>
      </button>
      <a
        href={href}
        target="_blank"
        rel="noopener noreferrer nofollow"
        className={ORIGIN_LINK}
      >
        {t.videoOrigin}
      </a>
    </div>
  );
}

/** 视频块渲染入口：命中 → 播放器；未命中 → 普通 <a>（绝不输出任意 src 的 iframe）。 */
export function VideoEmbedBlock({
  url,
  rules,
}: {
  url: string;
  rules: EmbedRule[] | null;
}) {
  const hit = matchEmbedRule(url, rules);
  if (!hit) {
    return (
      <a
        href={url}
        target="_blank"
        rel="noopener noreferrer nofollow"
        className="my-1 block break-all text-sm text-sky-deep hover:underline"
      >
        {url}
      </a>
    );
  }
  if (hit.rule.render_kind === "video") {
    const V = "my-3 w-full rounded-[var(--r-md)] border border-line bg-black";
    return (
      <video
        src={hit.src}
        controls
        playsInline
        preload="metadata"
        className={`${ASPECT_CLASS[hit.rule.aspect] ?? "aspect-video"} ${V}`}
      />
    );
  }
  return (
    <LazyIframe
      src={hit.src}
      aspect={hit.rule.aspect}
      name={hit.rule.name_zh}
      href={url}
    />
  );
}
