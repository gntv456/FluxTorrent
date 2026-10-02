/** 相对时间（求种/字幕列表共用）。
 *
 * ZT81（2026-10-02）：从 request-board.tsx 内联函数抽成共享模块——移动端卡片视图
 * 与桌面表格视图都要用，内联会导致两处复制或循环依赖。
 *
 * 2026-10-03 联调批：文案收编进 i18n（common.timeM/timeHm/timeDh/timeMoD）——
 * 此前这里硬编码简体中文，而字幕区另有一套英文缩写实现
 * （subtitle-board-format.ts 的 timeAgo），两套都不走三语字典：
 * en/zh-TW 用户在求种页看到简中、在字幕页看到英文。现统一走本模块 +
 * dict.common 键；字幕区实现保留为无 dict 场景的回落（渐次迁移）。
 */

export type TimeAgoDict = {
  timeM: string;
  timeHm: string;
  timeDh: string;
  timeMoD: string;
};

function fill(tpl: string, n: number, m = 0): string {
  return tpl.replaceAll("{n}", String(n)).replaceAll("{m}", String(m));
}

export function timeAgo(iso: string, t: TimeAgoDict): string {
  const diff = Date.now() - new Date(iso).getTime();
  const m = Math.floor(diff / 60000);
  if (m < 60) return fill(t.timeM, m);
  const h = Math.floor(m / 60);
  if (h < 24) return fill(t.timeHm, h, m % 60);
  const d = Math.floor(h / 24);
  if (d < 30) return fill(t.timeDh, d, h % 24);
  const mo = Math.floor(d / 30);
  return fill(t.timeMoD, mo, d % 30);
}
