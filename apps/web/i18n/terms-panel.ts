/**
 * 「术语表」面板自己的文案（0205 / 四审 L7）。
 *
 * 三种语言放在一起、由 tsc 强制结构对齐（后两份标注 `typeof termsPanelZh`），
 * 这样加一个键就会在另外两份里立刻报错，不会等到运行时才发现缺字。
 *
 * 注意：面板文案本身**不**过术语改写吗？——过的。它就是普通字典叶子，
 * 因此「术语」面板里的「原词/改成本站叫法」这些字也会跟着站上其它规则走；
 * 这是有意为之（没有哪一块文案该被排除在词汇表之外），
 * 但也意味着别把「种子」注册成规则后又指望这个面板还显示「种子」二字。
 */

export const termsPanelZh = {
  hint:
    "把界面与提示语里写死的固有词（种子 / 魔力 / 保种 / 邀请…）换成本站的叫法。" +
    "改的是「词」，不是内容：用户发的帖子、种子描述里的文字不会被改写。",
  how:
    "生效面两处：前端全部界面文案、后端返回的提示语。规则按最长词优先，" +
    "且每处只替换一次——替换出来的词不会再被别的规则二次改写。",
  add: "新增规则",
  canonical: "原词",
  replacement: "改成本站叫法",
  sort: "排序",
  status: "状态",
  saved: "术语规则已保存，下一个请求即生效",
  deleted: "规则已删除",
  empty: "还没有术语规则：全站文案按字典原样显示",
  on: "启用中",
  off: "已停用",
  pause: "停用",
  resume: "启用",
  delConfirm: "删除规则「{w}」？删掉后相关文案立刻回到原词。",
  sameWord: "原词与本站叫法相同：这条规则什么都不会改",
  tryIt: "试替换",
  tryHint: "输入一句话，看当前规则会把它改成什么",
  trySame: "（这段文字里没有命中任何原词）",
  count: "启用中 {n} 条 / 共 {m} 条",
};

export const termsPanelTw: typeof termsPanelZh = {
  hint:
    "把介面與提示語裡寫死的固有詞（種子 / 魔力 / 保種 / 邀請…）換成本站的叫法。" +
    "改的是「詞」，不是內容：使用者發的帖子、種子描述裡的文字不會被改寫。",
  how:
    "生效面兩處：前端全部介面文案、後端回傳的提示語。規則最長詞優先，" +
    "且每處只替換一次——替換出來的詞不會再被別的規則二次改寫。",
  add: "新增規則",
  canonical: "原詞",
  replacement: "改成本站叫法",
  sort: "排序",
  status: "狀態",
  saved: "術語規則已儲存，下一個請求即生效",
  deleted: "規則已刪除",
  empty: "還沒有術語規則：全站文案依字典原樣顯示",
  on: "啟用中",
  off: "已停用",
  pause: "停用",
  resume: "啟用",
  delConfirm: "刪除規則「{w}」？刪除後相關文案立刻回到原詞。",
  sameWord: "原詞與本站叫法相同：這條規則什麼都不會改",
  tryIt: "試替換",
  tryHint: "輸入一句話，看目前規則會把它改成什麼",
  trySame: "（這段文字裡沒有命中任何原詞）",
  count: "啟用中 {n} 條 / 共 {m} 條",
};

export const termsPanelEn: typeof termsPanelZh = {
  hint:
    "Replace the hard-coded words in the UI and in server messages " +
    "(torrent / spark / preserve / invite…) with your own wording. " +
    "Only the words change — user posts and torrent descriptions are not touched.",
  how:
    "Applies in two places: every UI string on the front end, and the messages " +
    "returned by the API. Longest match wins, and each match is replaced once — " +
    "replacement text is never re-scanned by another rule.",
  add: "Add rule",
  canonical: "Original word",
  replacement: "Replace with",
  sort: "Sort",
  status: "Status",
  saved: "Term saved — effective from the next request",
  deleted: "Term deleted",
  empty: "No terms yet: the built-in wording is used as is",
  on: "Enabled",
  off: "Disabled",
  pause: "Disable",
  resume: "Enable",
  delConfirm: 'Delete the rule "{w}"? Copy falls back immediately.',
  sameWord: "Original and replacement are identical: nothing would change",
  tryIt: "Try it",
  tryHint: "Type a sentence to see how the current rules rewrite it",
  trySame: "(this text contains none of the registered words)",
  count: "{n} enabled / {m} total",
};
