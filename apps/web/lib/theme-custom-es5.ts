/** no-flash 内联脚本的 ES5 片段（2026-11-14 自选颜色档；2026-11-15 二轮
 *  表面派生对齐 pastel 的 hue 派生口径——卡面 5% 起步、纱层色相化；
 *  2026-11-16 山海经模式：__cDerive(h,m,l) 三参，m='shj' 时只产用户
 *  色相位键集（岩彩底/墨字/金环/金箔/纱层不碰），l=1 按宣纸亮档对比度；
 *  L 值经 __shjL 迭代求 WCAG ≥4.5（固定 L 会失守）。）
 *
 * layout.tsx 的 ThemeNoFlash 内联脚本不过 SWC 转译、必须 ES5（旧内核
 * Chrome 57-91 裸跑，见 LegacyRuntimePolyfill 注释），故派生函数以字符串
 * 常量存在这里，由 layout 拼进 <script>。
 *
 * **同构纪律**：__cHex/__cDerive 与 lib/theme-custom.ts 的 hexToHsl/
 * deriveCustomVars 必须逐字同构（同输入同输出）——改任何一处都要同步
 * 另一处，tests/theme-custom.test.ts 有两版同输入同输出的回归门禁。 */

/** ES5 派生函数源码：__cHex（hex→HSL）+ __clamp + __cDerive（hex/mode→令牌组） */
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
  "function __lum(h){if(!/^#[0-9a-fA-F]{6}$/.test(h))return null;" +
  "var c=[parseInt(h.slice(1,3),16)/255,parseInt(h.slice(3,5),16)/255," +
  "parseInt(h.slice(5,7),16)/255];" +
  "for(var i=0;i<3;i++){c[i]=c[i]<=0.03928?c[i]/12.92:" +
  "Math.pow((c[i]+0.055)/1.055,2.4);}" +
  "return 0.2126*c[0]+0.7152*c[1]+0.0722*c[2];}" +
  "function __cr(a,b){var l1=__lum(a),l2=__lum(b);" +
  "if(l1===null||l2===null)return 0;var hi=Math.max(l1,l2)," +
  "lo=Math.min(l1,l2);return (hi+0.05)/(lo+0.05);}" +
  "function __h2x(H,S,L){var hn=((H%360)+360)%360/360,sn=S/100,ln=L/100;" +
  "var c=(1-Math.abs(2*ln-1))*sn,hh=hn*6,x=c*(1-Math.abs(hh%2-1));" +
  "var r=0,g=0,b=0;" +
  "if(hh<1){r=c;g=x;}else if(hh<2){r=x;g=c;}else if(hh<3){g=c;b=x;}" +
  "else if(hh<4){g=x;b=c;}else if(hh<5){r=x;b=c;}else{r=c;b=x;}" +
  "var m=ln-c/2;function to(v){var t=Math.round((v+m)*255).toString(16);" +
  "return t.length<2?'0'+t:t;}" +
  "return '#'+to(r)+to(g)+to(b);}" +
  "function __shjL(H,S,base,dir,other){var l=base;" +
  "for(var i=0;i<24;i++){if(__cr(__h2x(H,S,l),other)>=4.5)break;" +
  "l+=dir;if(l<4||l>96)break;}return Math.round(l);}" +
  "function __cDerive(h,m,lt){" +
  "var t=__cHex(h);if(!t)return null;var H=t[0],rawS=t[1];" +
  "var S=rawS<12?56:__clamp(rawS,30,68);" +
  "function f(ss,ll,a){return a===undefined?'hsl('+H+', '+ss+'%, '+ll+'%)'" +
  ":'hsla('+H+', '+ss+'%, '+ll+'%, '+a+')';}" +
  // design-cov-exempt: 山海经主题块锚定值字面量（同 TS 版豁免注释）
  "if(m==='shj'){var PAPER='#f7f3e8',INK='#0a0f0d',ONA='#101914';" +
  "var skyL=lt?__shjL(H,S,30,-1,PAPER):__shjL(H,S,78,1,ONA)," +
  "deepL=lt?__shjL(H,S,26,-1,PAPER):__shjL(H,S,56,1,INK);" +
  "var sky=f(S,skyL),deep=f(S,deepL),surf=f(S,38);" +
  "var o={};o['--sky']=sky;o['--sky-deep']=deep;" +
  "o['--sky-soft']=lt?f(S,40,0.12):f(S,50,0.14);" +
  "o['--indigo']=deep;o['--indigo-soft']=lt?f(S,40,0.12):f(S,50,0.16);" +
  "o['--candy']=deep;o['--candy-soft']=lt?f(S,40,0.12):f(S,50,0.14);" +
  "o['--text-brand']=deep;o['--info']=deep;" +
  "o['--focus-ring-soft']=lt?f(S,40,0.24):f(S,50,0.28);" +
  "o['--accent-shadow']=f(S,lt?32:42,lt?0.22:0.28);" +
  "o['--accent-shadow-soft']=f(S,lt?32:42,lt?0.14:0.18);" +
  "o['--accent-rule']=f(S,lt?32:42,lt?0.4:0.5);" +
  "o['--compat-glow']=surf;" +
  "o['--compat-line']=lt?f(S,30,0.34):f(S,42,0.26);" +
  "o['--compat-orange']=lt?deep:sky;" +
  "o['--compat-orange-bright']=lt?f(S,38):surf;" +
  "o['--compat-orange-dark']=deep;" +
  "o['--tide-deep']=sky;o['--tide-sea']=sky;" +
  "o['--tide-surf']=lt?f(S,26):f(S,38);o['--tide-foam']=lt?f(S,62):f(S,66);" +
  "o['--tide-signal']=deep;" +
  "o['--tide-grid']=lt?f(S,40,0.1):f(S,50,0.09);" +
  "o['--tide-mist']=lt?f(S,40,0.14):f(S,30,0.2);" +
  "o['--tide-haze']=lt?f(S,40,0.1):f(S,28,0.14);return o;}" +
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
