import type { ReactNode } from "react";

/**
 * BBCode 子集渲染器（NP 口径：b/i/u/s/color/size/font/url/img/quote/code/center/hr）。
 * 直接产出 React 节点，不走 dangerouslySetInnerHTML——URL 白名单 + style 传参，天然免疫 XSS。
 * 与 markdown-lite 并存：文本含 BBCode 标签时走本渲染器，否则维持原 markdown-lite。
 */

const TAG_RE =
  /\[(\/?)(b|i|u|s|color|size|font|url|img|quote|code|center|hr)(?:=([^\]]*))?\]/gi;

const SIZES: Record<string, string> = {
  "1": "12px",
  "2": "14px",
  "3": "17px",
  "4": "22px",
  "5": "30px",
};

export function hasBBCode(text: string): boolean {
  TAG_RE.lastIndex = 0;
  return TAG_RE.test(text);
}

function safeUrl(u: string): string | null {
  const t = u.trim();
  return /^https?:\/\//i.test(t) ? t : null;
}

function safeSize(p: string): string | undefined {
  const t = p.trim();
  if (SIZES[t]) return SIZES[t];
  if (/^\d{1,3}$/.test(t)) return `${Math.min(Number(t), 100)}px`;
  return undefined;
}

interface Frame {
  tag: string;
  param: string;
  children: ReactNode[];
}

function frameNode(f: Frame, key: string): ReactNode {
  const text = f.children.map((c) => (typeof c === "string" ? c : "")).join("");
  switch (f.tag) {
    case "b":
      return <b key={key}>{f.children}</b>;
    case "i":
      return <i key={key}>{f.children}</i>;
    case "u":
      return <u key={key}>{f.children}</u>;
    case "s":
      return <s key={key}>{f.children}</s>;
    case "color":
      return <span key={key} style={{ color: f.param.trim() || undefined }}>{f.children}</span>;
    case "size":
      return <span key={key} style={{ fontSize: safeSize(f.param) }}>{f.children}</span>;
    case "font":
      return <span key={key} style={{ fontFamily: f.param.trim() || undefined }}>{f.children}</span>;
    case "url": {
      const u = safeUrl(f.param || text);
      return u ? (
        <a key={key} href={u} target="_blank" rel="noreferrer noopener" className="text-sky hover:underline break-all">
          {f.children}
        </a>
      ) : (
        <span key={key}>{f.children}</span>
      );
    }
    case "img": {
      const u = safeUrl(text);
      return u ? (
        <img key={key} src={u} alt="" className="my-1 max-w-full rounded-[var(--r-sm)]" />
      ) : (
        <span key={key}>[img]</span>
      );
    }
    case "quote":
      return (
        <blockquote key={key} className="my-2 border-l-4 border-[var(--baozi-line)] bg-[var(--head-b)] px-3 py-2 text-sub">
          {f.children}
        </blockquote>
      );
    case "center":
      return <div key={key} className="text-center">{f.children}</div>;
    case "code":
      return <pre key={key} className="td-nfo my-2 overflow-x-auto rounded-[var(--r-sm)] bg-[var(--head-b)] p-3 text-xs">{text}</pre>;
    default:
      return <span key={key}>{f.children}</span>;
  }
}

/** [code] 内容不解析子标签（原始输出），主循环里特判 */
function renderBBCodeImpl(text: string): ReactNode[] {
  const root: ReactNode[] = [];
  const stack: Frame[] = [];
  const cur = () => (stack.length ? stack[stack.length - 1].children : root);
  let key = 0;
  let last = 0;
  const pushText = (t: string) => {
    if (t) cur().push(t);
  };
  TAG_RE.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = TAG_RE.exec(text))) {
    pushText(text.slice(last, m.index));
    const [full, closing, tagRaw, param] = m;
    const tag = tagRaw.toLowerCase();
    if (tag === "hr") {
      cur().push(<hr key={`e${key++}`} className="my-2 border-[var(--baozi-line)]" />);
      last = m.index + full.length;
      continue;
    }
    if (!closing) {
      if (tag === "code") {
        const lower = text.toLowerCase();
        const end = lower.indexOf("[/code]", m.index + full.length);
        if (end !== -1) {
          const raw = text.slice(m.index + full.length, end);
          cur().push(
            <pre key={`e${key++}`} className="td-nfo my-2 overflow-x-auto rounded-[var(--r-sm)] bg-[var(--head-b)] p-3 text-xs">
              {raw}
            </pre>,
          );
          last = end + "[/code]".length;
          TAG_RE.lastIndex = last;
          continue;
        }
      }
      stack.push({ tag, param: param ?? "", children: [] });
      last = m.index + full.length;
      continue;
    }
    // 闭合：找最近的同名开标签；孤立闭合按字面输出
    let idx = -1;
    for (let i = stack.length - 1; i >= 0; i--) {
      if (stack[i].tag === tag) {
        idx = i;
        break;
      }
    }
    if (idx === -1) {
      pushText(full);
      last = m.index + full.length;
      continue;
    }
    while (stack.length > idx) {
      const f = stack.pop()!;
      cur().push(frameNode(f, `e${key++}`));
    }
    last = m.index + full.length;
  }
  pushText(text.slice(last));
  while (stack.length) {
    const f = stack.pop()!;
    cur().push(frameNode(f, `e${key++}`));
  }
  return root;
}

export function renderBBCode(text: string): ReactNode[] {
  return renderBBCodeImpl(text);
}
