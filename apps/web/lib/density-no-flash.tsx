import { DENSITY_STORAGE_KEY } from "@/lib/density";

/** 密度 no-flash：首绘前读 localStorage 写 <html data-density>，避免
 *  紧凑档用户先看到舒适档再跳变（同 ThemeNoFlash 的防白闪思路）。
 *  脚本不过 SWC 转译，用 ES5 语法在旧内核裸跑。 */
export function DensityNoFlash() {
  const script =
    '(function(){try{var v=localStorage.getItem("' +
    DENSITY_STORAGE_KEY +
    '");if(v!=="compact"&&v!=="comfortable")return;' +
    'document.documentElement.setAttribute("data-density",v);}catch(e){}})();';
  return <script dangerouslySetInnerHTML={{ __html: script }} />;
}
