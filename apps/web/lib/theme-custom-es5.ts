/** no-flash 内联脚本的 ES5 片段（2026-11-14 自选颜色档；2026-11-15 二轮
 *  表面派生对齐 pastel 的 hue 派生口径——卡面 5% 起步、纱层色相化）。
 *
 * layout.tsx 的 ThemeNoFlash 内联脚本不过 SWC 转译、必须 ES5（旧内核
 * Chrome 57-91 裸跑，见 LegacyRuntimePolyfill 注释），故派生函数以字符串
 * 常量存在这里，由 layout 拼进 <script>。
 *
 * **同构纪律**：__cHex/__cDerive 与 lib/theme-custom.ts 的 hexToHsl/
 * deriveCustomVars 必须逐字同构（同输入同输出）——改任何一处都要同步
 * 另一处，tests/theme-custom.test.ts 有两版同输入同输出的回归门禁。 */

/** ES5 派生函数源码：__cHex（hex→HSL）+ __clamp + __cDerive（hex→令牌组） */
export const CUSTOM_DERIVE_ES5 =
  "function __cHex(h){if(!/^#[0-9a-fA-F]{6}$/.test(h))return null;" +
  "var r=parseInt(h.slice(1,3),16)/255,g=parseInt(h.slice(3,5),16)/255," +
  "b=parseInt(h.slice(5,7),16)/255,mx=Math.max(r,g,b),mn=Math.min(r,g,b)," +
  "l=(mx+mn)/2;if(mx===mn)return[0,0,Math.round(l*100)];" +
  "var d=mx-mn,s=d/(1-Math.abs(2*l-1)),hh=0;" +
  "if(mx===r)hh=((g-b)/d)%6;else if(mx===g)hh=(b-r)/d+2;else hh=(r-g)/d+4;" +
  "hh=hh*60;if(hh<0)hh+=360;" +
  "return[Math.round(hh),Math.round(s*100),Math.round(l*100)];}" +
  "function __clamp(v,lo,hi){return Math.min(hi,Math.max(lo,v));}" +
  "function __cDerive(h){" +
  "var t=__cHex(h);if(!t)return null;var H=t[0],rawS=t[1];" +
  "var S=rawS<12?56:__clamp(rawS,30,68);" +
  "function f(ss,ll,a){return a===undefined?'hsl('+H+', '+ss+'%, '+ll+'%)'" +
  ":'hsla('+H+', '+ss+'%, '+ll+'%, '+a+')';}" +
  "var sky=f(S,38),surf=f(S,50),deep=f(__clamp(S,32,58),26)," +
  "soft=f(__clamp(Math.round(S*0.62),20,46),92)," +
  "mist=f(__clamp(Math.round(S*0.55),18,42),90)," +
  "haze=f(__clamp(Math.round(S*0.45),14,36),96)," +
  "hovL=__clamp(Math.round(S*0.55),18,42)," +
  "sunL=__clamp(Math.round(S*0.5),16,38)," +
  "veilH=__clamp(Math.round(S*0.6),20,44);" +
  "var o={};o['--sky']=sky;o['--sky-deep']=deep;o['--sky-soft']=soft;" +
  "o['--indigo']=deep;o['--indigo-soft']=soft;o['--text-brand']=deep;" +
  "o['--info']=sky;o['--compat-glow']=surf;" +
  "o['--compat-glow-veil']=f(S,50,0.08);o['--focus-ring']=surf;" +
  "o['--focus-ring-soft']=f(S,50,0.12);" +
  "o['--accent-shadow']=f(S,45,0.24);o['--accent-shadow-soft']=f(S,45,0.16);" +
  "o['--accent-rule']=f(S,45,0.45);o['--compat-orange']=sky;" +
  "o['--compat-orange-bright']=f(S,52);o['--compat-orange-dark']=deep;" +
  "o['--grad-aurora']='linear-gradient(135deg, '+deep+' 0%, '+sky+' 55%, '+" +
  "f(S,55)+' 100%)';" +
  "o['--grad-rainbow']='linear-gradient(90deg, '+deep+', '+sky+', '+f(S,55)+" +
  "', '+f(S,66)+', '+f(S,78)+')';o['--ribbon']='var(--grad-rainbow)';" +
  "o['--surface-page']=haze;o['--surface-cloud']=mist;" +
  "o['--surface-card']=f(S,95);o['--surface-raised']=f(S,92);" +
  "o['--surface-hover']=f(hovL,88);o['--surface-sunken']=f(sunL,84);" +
  "o['--surface-cream']=f(__clamp(Math.round(S*0.45),14,36),96);" +
  "o['--head-a']=soft;o['--head-b']=haze;" +
  "o['--border-soft']=mist;o['--line-soft']=f(sunL,91);" +
  "o['--border-deep']=f(S,70);o['--muted-soft']=f(sunL,90);" +
  "o['--compat-line']=f(S,66);" +
  "o['--row-veil']=f(veilH,94,0.97);o['--row-veil-soft']=f(S,90,0.9);" +
  "o['--row-veil-mist']=f(S,88,0.8);" +
  "o['--chip-veil']=f(veilH,92,0.93);o['--chip-veil-thin']=f(S,90,0.72);" +
  "o['--promo-free-bg']=f(S,88);o['--promo-x2-bg']=f(43,90);" +
  "o['--tide-sea']=surf;o['--tide-deep']=deep;o['--tide-surf']=f(S,55);" +
  "o['--tide-foam']=f(S,66);o['--tide-mist']=mist;" +
  "o['--tide-haze']=f(hovL,88);" +
  "o['--tide-grid']=f(S,50,0.07);return o;}";
