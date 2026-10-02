/**
 * 火花流水 kind → 人话（2026-09-30「字段名直出」全站收口批）。
 *
 * 键=后端 earn_spark/spend_spark 调用点的 kind 字面量全集
 * （apps/api 全仓 grep + 本库 spark_ledger distinct 双向核对）。
 * 同名簇归并：游戏四玩法/抽卡/娱乐屋统一「游戏娱乐」类，银行四流水
 * 统一「银行」类——用户看的是来源大类，不必区分内部动作细节。
 *
 * 使用：me/sparks 用户流水页与后台用户详情 SparkPanel 共用。
 * 未收录 kind 回落原始 key，不静默。
 */

export const SPARK_KIND_LABELS: Record<string, string> = {
  // 签到与奖励
  attendance: "签到",
  seeding_reward: "做种收益",
  task_reward: "任务奖励",
  achievement: "成就奖励",
  jixiao_reward: "考核奖励",
  initial_grant: "初始发放",
  levelup: "升级奖励",
  week: "周常奖励",
  month: "月度奖励",

  // 商店与消费
  shop: "商店消费",
  shop_buy: "商店消费",
  promo_buy: "促销购买",
  donation_tier: "捐赠档位",
  voucher: "券核销",

  // 游戏 / 娱乐屋 / 抽卡
  game: "游戏娱乐",
  games: "游戏娱乐",
  arcade: "娱乐屋",
  gacha: "抽卡",
  fishing: "钓鱼",
  farm: "农场",

  // 论坛
  forum: "论坛奖励",
  "forum-like": "点赞",
  "forum-post-del": "删帖扣除",
  "forum-topic-del": "删主题扣除",
  forum_bounty: "发布悬赏",
  forum_bounty_refund: "悬赏退还",
  forum_lottery: "论坛抽奖",
  forum_lottery_refund: "抽奖退还",
  forum_tip: "打赏",
  vote: "投票",

  // 字幕区
  subtitle: "字幕奖励",
  subtitle_award: "字幕评选奖励",
  subtitle_bounty: "字幕悬赏",
  subtitle_report: "字幕上报",
  subtitle_request: "发起求字幕",
  subtitle_request_refund: "求字幕退还",
  subtitle_violation: "字幕违规扣除",

  // 银行
  bank: "银行",
  bank_deposit: "银行存款",
  bank_withdraw: "银行取款",
  bank_demand_in: "活期转入",
  bank_demand_out: "活期转出",
  bank_loan_payout: "贷款放款",
  bank_loan_repay: "贷款偿还",
  bank_loan_repay_partial: "贷款部分偿还",

  // H&R / 求种
  hr_pardon: "H&R 赦免",
  hr_pardon_refund: "H&R 赦免退还",
  request_bounty: "求种悬赏",

  // 众筹 / 池
  funding: "众筹参与",
  funding_refund: "众筹退还",
  pool_donate: "魔力池捐赠",

  // 管理
  admin: "管理调整",
  adjust: "管理调整",
  increment_bulk: "批量发放",
  bonus: "奖励发放",
  amountbonus: "魔力奖励",
  refund: "退款",
  gift: "赠送",
};
