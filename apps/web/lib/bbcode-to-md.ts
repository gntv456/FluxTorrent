/** BBCode → Markdown 一次性转换（0424，同类站对标小件）。
 *
 *  场景：从他站（NexusPHP 系）转种时，简介整段是 BBCode——手动逐标签
 *  改写是发布组最高频的摩擦点之一。本模块只做**单向、尽力**转换：
 *  覆盖 NP 常用标签（b/i/u/s/color/size/url/img/quote/code/list），
 *  不认识的标签原样保留，转换结果给用户过目再提交——不做「静默完美」
 *  的假承诺。
 *
 *  纯函数、无依赖：上传表单 300 行门禁下放 lib 层。 */

export function bbcodeToMarkdown(src: string): string {
  let s = src;
  // 代码块先行（内部不再做其它替换）
  s = s.replace(/\[code\]([\s\S]*?)\[\/code\]/gi, (_m, c: string) =>
    `\`\`\`\n${c.replace(/\n+$/, "")}\n\`\`\``,
  );
  // 粗体/斜体/删除线（下划线 MD 无原生语法，用 <u> 直排——渲染端支持内联 HTML 的场景兜得住）
  s = s.replace(/\[b\]([\s\S]*?)\[\/b\]/gi, "**$1**");
  s = s.replace(/\[i\]([\s\S]*?)\[\/i\]/gi, "*$1*");
  s = s.replace(/\[u\]([\s\S]*?)\[\/u\]/gi, "<u>$1</u>");
  s = s.replace(/\[s\]([\s\S]*?)\[\/s\]/gi, "~~$1~~");
  // 颜色/字号：MD 无对应，降级为加粗提示文本（保留语义不丢内容）
  s = s.replace(/\[color=[^\]]*\]([\s\S]*?)\[\/color\]/gi, "$1");
  s = s.replace(/\[size=[^\]]*\]([\s\S]*?)\[\/size\]/gi, "$1");
  s = s.replace(/\[font=[^\]]*\]([\s\S]*?)\[\/font\]/gi, "$1");
  // 链接两种形态：[url]x[/url] 与 [url=x]text[/url]
  s = s.replace(/\[url\]([^[]*)\[\/url\]/gi, "$1");
  s = s.replace(/\[url=([^\]]*)\]([\s\S]*?)\[\/url\]/gi, "[$2]($1)");
  // 图片
  s = s.replace(/\[img\]([^[]*)\[\/img\]/gi, "![]($1)");
  // 引用：逐行加 >
  s = s.replace(/\[quote\]([\s\S]*?)\[\/quote\]/gi, (_m, q: string) =>
    q
      .split("\n")
      .map((l) => `> ${l}`)
      .join("\n"),
  );
  s = s.replace(
    /\[quote=([^\]]*)\]([\s\S]*?)\[\/quote\]/gi,
    (_m, who: string, q: string) =>
    `> **${who}**\n` +
    q
      .split("\n")
      .map((l) => `> ${l}`)
      .join("\n"),
  );
  // 列表（[*] 与 [li] 两种口径）
  s = s.replace(/\[list\]([\s\S]*?)\[\/list\]/gi, (_m, l: string) =>
    l
      .split(/\[\*\]|\[li\]/)
      .map((it) => it.trim())
      .filter(Boolean)
      .map((it) => `- ${it}`)
      .join("\n"),
  );
  // 有序列表
  s = s.replace(/\[list=[^\]]*\]([\s\S]*?)\[\/list\]/gi, (_m, l: string) =>
    l
      .split(/\[\*\]|\[li\]/)
      .map((it) => it.trim())
      .filter(Boolean)
      .map((it, i) => `${i + 1}. ${it}`)
      .join("\n"),
  );
  return s;
}
