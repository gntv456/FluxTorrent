// 把设计稿 <style> 主块转成 /gacha 路由专用 CSS：
// - 所有选择器加 .gx 作用域前缀（避免 .card/.panel/.btn 等通用类与站内样式互相污染）
// - 去掉样图评审栏/四端画框相关规则（生产页没有这些元素）
// - @container 保留（.gx 自己就是容器），把 .screen 的容器属性补到 .gx 上
import fs from "node:fs";
import path from "node:path";
import postcss from "postcss";

const SRC = "D:/FluxTorrent/design/gacha/抽卡页-高保真样图.html";
const OUT = "D:/FluxTorrent/apps/web/app/styles/gacha-stage.css";

const html = fs.readFileSync(SRC, "utf8");
const start = html.lastIndexOf("<style");
const end = html.lastIndexOf("</style>");
const css = html.slice(start + 7, end);

const DROP_PREFIX = [
  ".review-bar", ".vbtn", ".jbtn", ".frame", ".screen", ".wrap",
  ".vbar", ".frame-cap", ".shot",
];

const root = postcss.parse(css);
root.walkRules((rule) => {
  if (rule.parent?.type === "atrule" && /keyframes/i.test(rule.parent.name)) return;
  const sels = rule.selectors.map((s) => s.trim());
  // 命中画框/评审栏的一律删掉
  if (sels.every((s) => DROP_PREFIX.some((p) => s.startsWith(p)))) {
    if (/^\.screen\[data-w|^\.screen$/.test(sels[0]) || sels.every((s) => DROP_PREFIX.some((p) => s.startsWith(p)))) {
      rule.remove();
      return;
    }
  }
  rule.selectors = sels
    .filter((s) => !DROP_PREFIX.some((p) => s === p || s.startsWith(p + "[") || s.startsWith(p + ":") || s.startsWith(p + " ") || s.startsWith(p + ".")))
    .map((s) => {
      if (s === ":root" || s === "html" || s === "body") return ".gx";
      if (s.startsWith(".gx")) return s;
      return ".gx " + s;
    });
  if (!rule.selectors.length) rule.remove();
});

// 页脚统一补一条容器型（原样图里是 .screen 承担）
const extra = postcss.parse(`
.gx{container-type:inline-size}
.gx .app{height:100%;max-height:100%;overflow:hidden}
`);
root.append(extra);

const header = `/* gacha-stage.css —— /gacha 抽卡页（星轨卡册）生产样式
   来源：design/gacha/抽卡页-高保真样图.html 的设计系统，全量选择器
   加 .gx 作用域后落到路由；样图的画框/评审栏规则已剔除。
   由 scripts 生成一次后手工维护：改样式请直接改本文件，勿再从样图覆盖。 */
`;
fs.writeFileSync(OUT, header + root.toString());
console.log("written", OUT, (header + root.toString()).length);
