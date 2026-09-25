import React from "react";
import { VideoEmbedBlock } from "@/components/forum-video-embed";
import type { EmbedRule } from "@/lib/data-forum";

/**
 * 安全 Markdown 子集渲染器（零依赖）。
 * 关键：不使用 dangerouslySetInnerHTML —— 全部经 React 元素输出，天然防 XSS。
 * 支持：``` 代码块 / # 标题 / > 引用 / - 无序 / 1. 有序 / --- 分隔线 /
 *       行内 **粗** *斜* ~~删~~ `代码` [文字](链接) @提及 / 段落软换行。
 * 链接仅允许 http(s)/mailto/站内绝对路径，其余当纯文本（防 javascript: 等）。
 * 视频内嵌（0189）：!video(URL) 显式语法 + 整段恰为 URL 的段落自动转换；
 * 命中白名单规则才渲染播放器，否则降级为普通链接（fail-closed）。
 */

const SAFE_URL = /^(https?:\/\/|mailto:|\/)/i;

/** !video(URL) 显式语法（独立行；URL 限 http(s)/站内绝对路径）。 */
const VIDEO_RE = /^!video\(([^)\s]+)\)$/;
/** 整段恰为一个 URL（Onebox 式自动转换；非 http(s)/站内路径不触发）。 */
const BARE_URL_RE = /^(https?:\/\/[^\s]+|\/[^\s]*)$/;

const INLINE_RE =
  /(`[^`\n]+`)|(\*\*[^*\n]+\*\*)|(__[^_\n]+__)|(~~[^~\n]+~~)|(\*[^*\n]+\*)|(_[^_\n]+_)|(\[[^\]\n]+\]\([^)\n]+\))|(@[A-Za-z0-9_\-\u4e00-\u9fa5]{2,})/g;

function renderInline(text: string, keyBase: string): React.ReactNode[] {
  const nodes: React.ReactNode[] = [];
  let last = 0;
  let i = 0;
  for (const m of text.matchAll(INLINE_RE)) {
    const idx = m.index ?? 0;
    if (idx > last) nodes.push(text.slice(last, idx));
    const raw = m[0];
    const key = `${keyBase}-i${i++}`;
    if (raw.startsWith("`")) {
      nodes.push(
        <code
          key={key}
          className="rounded bg-[var(--surface-sunken)] px-1 py-0.5 font-mono text-[0.85em]"
        >
          {raw.slice(1, -1)}
        </code>,
      );
    } else if (raw.startsWith("**") || raw.startsWith("__")) {
      nodes.push(<strong key={key}>{raw.slice(2, -2)}</strong>);
    } else if (raw.startsWith("~~")) {
      nodes.push(<del key={key}>{raw.slice(2, -2)}</del>);
    } else if (raw.startsWith("*") || raw.startsWith("_")) {
      nodes.push(<em key={key}>{raw.slice(1, -1)}</em>);
    } else if (raw.startsWith("[")) {
      const mm = raw.match(/^\[([^\]]+)\]\(([^)]+)\)$/);
      if (mm) {
        const href = mm[2].trim();
        if (SAFE_URL.test(href)) {
          nodes.push(
            <a
              key={key}
              href={href}
              target="_blank"
              rel="noopener noreferrer nofollow"
              className="text-sky-deep hover:underline"
            >
              {mm[1]}
            </a>,
          );
        } else {
          nodes.push(mm[1]);
        }
      } else {
        nodes.push(raw);
      }
    } else if (raw.startsWith("@")) {
      nodes.push(
        <span key={key} className="font-bold text-sky-deep">
          {raw}
        </span>,
      );
    } else {
      nodes.push(raw);
    }
    last = idx + raw.length;
  }
  if (last < text.length) nodes.push(text.slice(last));
  return nodes;
}

function heading(level: number, children: React.ReactNode, key: string) {
  const cls =
    level <= 2
      ? "mt-4 mb-2 font-display text-xl text-ink"
      : "mt-3 mb-1.5 font-display text-base text-ink";
  const n = Math.min(Math.max(level, 1), 6);
  const Tag = `h${n}` as keyof React.JSX.IntrinsicElements;
  return (
    <Tag key={key} className={cls}>
      {children}
    </Tag>
  );
}

const isFence = (l: string) => /^\s*(```|~~~)/.test(l);
const isHeading = (l: string) => /^\s*#{1,6}\s+/.test(l);
const isQuote = (l: string) => /^\s*>\s?/.test(l);
const isUl = (l: string) => /^\s*[-*+]\s+/.test(l);
const isOl = (l: string) => /^\s*\d+[.)]\s+/.test(l);
const isHr = (l: string) => /^\s*(-{3,}|\*{3,}|_{3,})\s*$/.test(l);

/** 视频块识别（0189）：!video(URL) 显式 / 整段恰为一个 URL 的自动转换。
 *  返回 URL（SAFE_URL 已过）或 null（继续按普通块解析）。 */
function videoLine(
  line: string,
): string | null {
  const t = line.trim();
  const vm = t.match(VIDEO_RE);
  if (vm && SAFE_URL.test(vm[1])) return vm[1];
  const bm = t.match(BARE_URL_RE);
  if (bm && SAFE_URL.test(bm[1])) return bm[1];
  return null;
}

function parseBlocks(
  src: string,
  rules: EmbedRule[] | null,
): React.ReactNode[] {
  const lines = src.replace(/\r\n?/g, "\n").split("\n");
  const out: React.ReactNode[] = [];
  let i = 0;
  let k = 0;

  while (i < lines.length) {
    const line = lines[i];

    const fence = line.match(/^\s*(```|~~~)(.*)$/);
    if (fence) {
      const marker = fence[1];
      const buf: string[] = [];
      i++;
      while (i < lines.length && !lines[i].trimStart().startsWith(marker)) {
        buf.push(lines[i]);
        i++;
      }
      i++; // 跳过结束围栏
      const key = `b${k++}`;
      out.push(
        <pre
          key={key}
          className="my-3 overflow-x-auto rounded-[var(--r-sm)] bg-[var(--surface-sunken)] p-3 text-xs"
        >
          <code className="font-mono">{buf.join("\n")}</code>
        </pre>,
      );
      continue;
    }

    if (line.trim() === "") {
      i++;
      continue;
    }

    // 视频内嵌（0189）：!video(URL) 显式语法 + 整段恰为 URL 的自动转换。
    // 未命中规则时 VideoEmbedBlock 内部降级为链接（fail-closed）。
    const v = videoLine(line);
    if (v) {
      out.push(<VideoEmbedBlock key={`v${k++}`} url={v} rules={rules} />);
      i++;
      continue;
    }

    if (isHr(line)) {
      out.push(<hr key={`b${k++}`} className="my-4 border-line" />);
      i++;
      continue;
    }

    const h = line.match(/^\s*(#{1,6})\s+(.*)$/);
    if (h) {
      out.push(heading(h[1].length, renderInline(h[2], `b${k}`), `b${k++}`));
      i++;
      continue;
    }

    if (isQuote(line)) {
      const buf: string[] = [];
      while (i < lines.length && isQuote(lines[i])) {
        buf.push(lines[i].replace(/^\s*>\s?/, ""));
        i++;
      }
      out.push(
        <blockquote
          key={`b${k++}`}
          className="my-3 border-l-4 border-line pl-3 text-sub"
        >
          {renderInline(buf.join(" "), `b${k}`)}
        </blockquote>,
      );
      continue;
    }

    if (isUl(line)) {
      const items: string[] = [];
      while (i < lines.length && isUl(lines[i])) {
        items.push(lines[i].replace(/^\s*[-*+]\s+/, ""));
        i++;
      }
      const key = `b${k++}`;
      out.push(
        <ul key={key} className="my-2 list-disc space-y-0.5 pl-5 text-sm">
          {items.map((t, j) => (
            <li key={j}>{renderInline(t, `${key}-${j}`)}</li>
          ))}
        </ul>,
      );
      continue;
    }

    if (isOl(line)) {
      const items: string[] = [];
      while (i < lines.length && isOl(lines[i])) {
        items.push(lines[i].replace(/^\s*\d+[.)]\s+/, ""));
        i++;
      }
      const key = `b${k++}`;
      out.push(
        <ol key={key} className="my-2 list-decimal space-y-0.5 pl-5 text-sm">
          {items.map((t, j) => (
            <li key={j}>{renderInline(t, `${key}-${j}`)}</li>
          ))}
        </ol>,
      );
      continue;
    }

    // 段落：连续非空、非块级起始行合并；软换行保留为 <br/>
    const buf: string[] = [];
    while (
      i < lines.length &&
      lines[i].trim() !== "" &&
      !isFence(lines[i]) &&
      !isHeading(lines[i]) &&
      !isQuote(lines[i]) &&
      !isUl(lines[i]) &&
      !isOl(lines[i]) &&
      !isHr(lines[i])
    ) {
      buf.push(lines[i]);
      i++;
    }
    if (buf.length) {
      const key = `b${k++}`;
      out.push(
        <p
          key={key}
          className="my-1.5 whitespace-pre-wrap break-words text-sm leading-relaxed"
        >
          {buf.map((t, j) => (
            <React.Fragment key={j}>
              {j > 0 && <br />}
              {renderInline(t, `${key}-${j}`)}
            </React.Fragment>
          ))}
        </p>,
      );
    }
  }
  return out;
}

export function MarkdownRenderer({
  source,
  embedRules = null,
}: {
  source: string;
  /** 视频内嵌白名单规则（0189）；null/undefined = 未启用（视频语法降级链接） */
  embedRules?: EmbedRule[] | null;
}) {
  if (!source) return null;
  return <div className="text-ink">{parseBlocks(source, embedRules)}</div>;
}
