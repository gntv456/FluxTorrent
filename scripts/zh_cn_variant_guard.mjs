// 简繁混排门禁：zh-CN.ts 里出现繁体字 / zh-TW.ts 里出现简体字 = 串味。
//
// 背景：i18n_guard.mjs 只管「硬编码中文」，不管**字形对不对**。
// 2026-10-20 实测抓到三处串味：
//   zh-CN  adminTasks.penaltyHint —— "补救期**過後**…**時自動執行**，**與**…"
//   zh-CN  "**餘額**不足"（insufficient，应为「余额不足」）
//   zh-TW  "掛載模組（**可选**）"、"此項即從前台**下线**"（应为「可選」「下線」）
// 简体站点里突然冒出繁体字 = 典型事故（客服/SEO/截图全受影响），
// 而 i18n_guard 永远不会报：它管的是"文案该不该进 i18n"，不是"进了 i18n 之后字形对不对"。
//
// 清单来源：**不是**外部词表，而是从本仓 zh-TW.ts / zh-CN.ts 逐字对齐
// 抽出的「本项目词汇里繁体独有 / 简体独有」的单字集：
//   · 两份 locale 的字符串按顺序对齐，只有**字形确实不同**的位置才计入；
//   · 取出现次数 ≥2 的稳定映射（一次性的对齐噪声不算）；
//   · 两边字形完全相同地出现过的字（"余量"这种两边通用字）一律排除——
//     否则「群发/余量」这种合法用法会被误报。
// 重建方法见本文件末尾注释（scripts/_derive_variant_chars.py）。
//
// 用法：node scripts/zh_cn_variant_guard.mjs（CI web job 一步；exit 1 = 有串味）。
import { readFileSync } from "node:fs";
import path from "node:path";

const ROOT = path.resolve(import.meta.dirname, "..", "apps/web", "i18n");

/** 本项目词汇里的「繁体独有字」——zh-CN.ts 里不该出现 */
const TRAD_ONLY =
  "並佇佈佔併來依係個們側偵備傳債僅價儀償優儲兌內兩冊冪凍別刪則剛創劇動務勝勢勳勵匣" +
  "匯區協參員唯問啟單嗶嘗嚮嚴國圍圓圖執報場塊塗壇夾娛學宣宮實審寫寬寵寶將專尋對導層" +
  "屬崗帳帶幣幫幹庫廢廣廳張強彈後徑從復態憑應懸戰戲戶拋掃掛採換援搶擁擇擊擬擰擲擾攔" +
  "敗數斷於旨時晉暫曆曉會杖棄業榮構樂樓標樣樹機橫檔檢檻欄權歡歷氣決沒況洩淨減測湊準" +
  "滾滿漁澆濟濾瀏瀕為無熱營牆牠狀獎獨獲現環產異當疊發盡盤眾確碼礎禮稅種稱積穫筆節範" +
  "篩簡簽籌籤糧紀紅納純級細組絕絡給統綁經綠維網綴線編緩縮總績繫繼續罰義聯職聽臨與舉" +
  "舊萬著蓋薦藝處虛號術衝補裝裡製複見規視覽觀觸訂計訊討訖記設許訴註評詞詢試話該詳誌" +
  "認誤說課調請論謝證識譯議護譽讀變讓讚負貨責貴買貸費貼資賓賞賠賣質賬賺購賽贈贏趨躍" +
  "軌軟較載輕輪輯輸轉辦農這週進遊運過達違遠適遷選還邊郵釋針釣鈕鉤銀銜銷錄錢錯鍵鎖鏈" +
  "鐘鑑鑰長門閉開間閘閱閾關陣隊階際隨險隱雙離靜響頁頂項順須預領頭頻顆題額願類顯風飽" +
  "飾養餌餓餘餵饋駁驗驚魚鮮麼點齊齡龍";

/** 本项目词汇里的「简体独有字」——zh-TW.ts 里不该出现 */
const SIMP_ONLY =
  "万与专业两严个临为举么义乐买于产仅从仪们价众优会传体侧债偿储兑关养册写农冲决况冻" +
  "净减凑凭击则刚创删别剧办务动励势勋区协单卖占厅历参双发变叠号后听启员周响哔喂围国" +
  "图圆场坛墙声处备复头夹奖娱学宝实宠审宫宽宾对导将尝尽层属岗币帐带帮幂干并广库应废" +
  "开异弃张弹强归当录径志态总悬惊愿戏战户执扫扰抛抢护报拟拥拦拧择挂换据掷数断无旧时" +
  "显晋晓暂术机权条来构标栏树样档检楼槛横欢气汇没泄浇测济浏涂渔游滚满滤濒点热状独环" +
  "盖盘着码础确礼离种积称税笔筛筹签简类粘粮红级纪纯纳组细经绑结给络绝统继绩续维绿缀" +
  "缓编缩网罚职联胜脚艺节范荐荣获营虑虚衔补见观规视览触誉计订认讨让议讯记许论设访证" +
  "评识诉词译试话询该详语误说请读课调谢负责败账货质购贴贵贷费资赏赔赚赛赞赠赢趋跃踢" +
  "轨转轮软轻载较辑输辨边达迁过运还这进远违连适邮采释鉴针钓钟钥钩钮钱银销锁错键长门" +
  "闭问间闸阅阈队阵际险随隐静页顶项顺须预领频颗题额风饰饱饵饿馈驳验鱼鲜齐龄龙";

const CJK = /[\u3400-\u9fff]/;

/** 抽出文件里的**字符串字面量**（跳过注释行；注释里的繁体不算串味） */
function literalsOf(file) {
  const lines = readFileSync(path.join(ROOT, file), "utf-8").split("\n");
  const out = [];
  lines.forEach((l, i) => {
    const t = l.trim();
    if (t.startsWith("//") || t.startsWith("*") || t.startsWith("/*")) return;
    const re = /"((?:[^"\\]|\\.)*)"|'((?:[^'\\]|\\.)*)'|`((?:[^`\\]|\\.)*)`/g;
    let m;
    while ((m = re.exec(l)) !== null) {
      out.push({ line: i + 1, text: m[1] ?? m[2] ?? m[3] ?? "" });
    }
  });
  return out;
}

function scan(file, banned, label) {
  const bad = [];
  for (const { line, text } of literalsOf(file)) {
    if (!CJK.test(text)) continue;
    for (const ch of new Set(text)) {
      if (banned.includes(ch)) bad.push({ line, ch });
    }
  }
  return bad;
}

let failed = 0;
for (const [file, banned, label] of [
  ["zh-CN.ts", TRAD_ONLY, "繁体字（zh-CN 是简体）"],
  ["zh-TW.ts", SIMP_ONLY, "简体字（zh-TW 是繁体）"],
]) {
  const bad = scan(file, banned);
  if (!bad.length) {
    console.log(`简繁门禁通过：${file}`);
    continue;
  }
  failed = 1;
  console.error(`简繁门禁：${file} 出现${label}：`);
  for (const b of bad) console.error(`  ${file}:${b.line}  「${b.ch}」`);
  console.error("  修法：按对应语种的字形改写该字（简体站用简体、繁体站用繁体）。");
}

process.exit(failed);

// ── 怎么重建两份清单 ──────────────────────────────────────────────
// 1) 把 zh-TW.ts / zh-CN.ts 的字符串字面量按出现顺序抽出来；
// 2) difflib 对齐两条 CJK 字符序列，只取**字形不同**且长度相同的位置；
// 3) 记 (繁体→简体) 与 (简体→繁体) 的出现次数；
// 4) 取次数 ≥2 的稳定映射；两边字形完全相同的字（equal 段）一律排除；
// 5) 「繁体独有」= 只在繁体侧出现、且从不是简体侧；反之亦然。
// 见 scripts/_derive_variant_chars.py（一次性脚本，结果贴在 guard 顶部）。
