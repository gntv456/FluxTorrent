/** 主题 no-flash 内联脚本（SSR 组件）。从 app/layout.tsx 拆出（行数门禁，
 *  2026-11-14 自选颜色批）；行为与原内联位置完全一致——组件仍在 <body>
 *  顶部首组件位由 layout 挂载，SSR 输出不变。
 *
 *  职责：首绘前读 localStorage（缺省跟随系统 prefers-color-scheme）写入
 *  <html data-theme> 防白闪；同步 PWA theme-color；按需注入/移除山海经
 *  <link>（ES5 等价 syncThemeCss）；监听系统偏好变化（auto 档实时跟随）。
 *
 *  2026-11-14 custom 档：flux-theme="custom" 时 base 落 brand（日间亮底），
 *  再从 flux-custom-color 的 #rrggbb 派生整组令牌以 inline style 写 <html>
 *  （inline > 任何 [data-theme] 块）。派生算法必须与 lib/theme-custom.ts 的
 *  hexToHsl/deriveCustomVars **逐字同构**（同输入同输出，测试回归）；ES5
 *  源码在 lib/theme-custom-es5.ts（本脚本不过 SWC 转译，旧内核裸跑）。 */

import {
  THEMES,
  THEME_IDS,
  THEME_STORAGE_KEY,
  CUSTOM_COLOR_STORAGE_KEY,
  themeCssManifest,
  themeIdByScheme,
} from "@/lib/themes";
import { CUSTOM_DERIVE_ES5 } from "@/lib/theme-custom-es5";

export function ThemeNoFlash() {
  const CH = JSON.stringify(
    Object.fromEntries(Object.entries(THEMES).map(([k, v]) => [k, v.chrome])),
  );
  const ids = JSON.stringify(THEME_IDS);
  // 明暗族 → 主题 id（scheme 字段唯一真源，不做 IDS[0]/IDS[1] 位置假设）
  const SC = JSON.stringify({
    light: themeIdByScheme("light"),
    dark: themeIdByScheme("dark"),
  });
  // 山海经 CSS 的注入清单（[themeId 前缀, fontsUrl, themeUrl]；同步
  // scripts/gen-theme-css.mjs 的 manifest 键）
  const SHJ = JSON.stringify([
    "shanhaijing",
    themeCssManifest.fonts,
    themeCssManifest.theme,
  ]);
  const script =
    `(function(){var CH=${CH},IDS=${ids},SC=${SC},SHJ=${SHJ};` +
    CUSTOM_DERIVE_ES5 +
    `var CK='${CUSTOM_COLOR_STORAGE_KEY}';var LAST=[];` +
    `function chrome(t){var m=document.querySelector(` +
    `'meta[name="theme-color"]');` +
    `if(!m){m=document.createElement('meta');m.name='theme-color';` +
    `document.head.appendChild(m);}m.content=CH[t];}` +
    `function css(t){var want=(t&&t.indexOf(SHJ[0])===0)?[SHJ[1],SHJ[2]]:[];` +
    `var ls=document.querySelectorAll('link[data-theme-css]');` +
    `for(var i=0;i<want.length;i++){var ok=false;` +
    `for(var j=0;j<ls.length;j++){var h=ls[j].getAttribute('href');` +
    `if(h===want[i]){ok=true;break;}}` +
    `if(!ok){var l=document.createElement('link');l.rel='stylesheet';` +
    `l.href=want[i];l.setAttribute('data-theme-css','');` +
    `document.head.appendChild(l);}}` +
    `for(var k=ls.length-1;k>=0;k--){var hh=ls[k].getAttribute('href');` +
    `if(want.indexOf(hh)<0)ls[k].parentNode.removeChild(ls[k]);}}` +
    `function clearCustom(){var r=document.documentElement;` +
    `for(var i=0;i<LAST.length;i++)r.style.removeProperty(LAST[i]);LAST=[];` +
    `if(r.removeAttribute){r.removeAttribute('data-custom-themed');}}` +
    `function applyCustom(hex){clearCustom();var v=__cDerive(hex);` +
    `if(!v)return;var r=document.documentElement;` +
    `for(var k in v){if(!Object.prototype.hasOwnProperty.call(v,k))continue;` +
    `r.style.setProperty(k,v[k]);LAST.push(k);}}` +
    `function apply(t){document.documentElement.dataset.theme=t;` +
    `chrome(t);css(t);}` +
    `function sys(){return window.matchMedia&&` +
    `window.matchMedia('(prefers-color-scheme: dark)').matches` +
    `?SC.dark:SC.light;}` +
    `function pick(v){return (v==='auto'||IDS.indexOf(v)<0)?sys():v;}` +
    `try{var saved=localStorage.getItem('${THEME_STORAGE_KEY}');` +
    `var cc=null;` +
    `if(saved==='custom'){cc=localStorage.getItem(CK);}` +
    `if(saved==='custom'&&cc&&/^#[0-9a-fA-F]{6}$/.test(cc)){` +
    `apply(pick('custom'));applyCustom(cc);}` +
    `else{apply(pick(saved));}` +
    `if(!saved||saved==='auto'){` +
    `var mq=window.matchMedia('(prefers-color-scheme: dark)');` +
    `if(mq.addEventListener){mq.addEventListener('change',function(){` +
    `try{var cur=localStorage.getItem('${THEME_STORAGE_KEY}');` +
    `if(!cur||cur==='auto'){apply(sys());clearCustom();}}catch(_){}});}}` +
    `}catch(e){}})();`;
  return <script dangerouslySetInnerHTML={{ __html: script }} />;
}
