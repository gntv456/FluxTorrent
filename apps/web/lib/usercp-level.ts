import { formatBytes } from "@/lib/format";

/** 会员中心「等级进度」卡的折算（P-516）。
 *
 *  `/me/overview` 的 `next_class` 现按 worker `class_adj.rs` 升降级权威谓词的
 *  四项条件下发，键名与公开主页 `auth_http/profile.rs` 逐字相同。卡上只有
 *  一条进度，所以显示**达标率最低**的那一项——其余三项再漂亮也不升职。
 *  抽成纯函数的理由同 `me/exams/_parts/exam-metrics.ts`：宿主文件顶在门禁
 *  上，而这段是「键名 → 单位口径」的映射，界面里没法便宜地钉。 */

/** 下一等级的四项条件现值与门槛（null = 没有下一档，职员档即如此） */
export interface NextClass {
  name: string;
  uploaded: number;
  uploaded_need: number;
  download_count: number;
  download_count_need: number;
  seed_hours: number;
  seed_hours_need: number;
  account_age_days: number;
  account_age_days_need: number;
}

export interface LevelProgress {
  /** 最低达标率（0-100）；门槛 ≤0 视为该项已达标（与公开主页 prog 同式） */
  pct: number;
  /** 该项的「现值 / 门槛」，单位随条件 */
  text: string;
  /** 该项还差多少（同一单位，负差按 0 收） */
  gap: string;
  /** 决定这条进度的条件名（上传量 / 完成数 / 做种时长 / 账龄） */
  label: string;
}

type Criterion = {
  label: string;
  cur: number;
  need: number;
  fmt: (n: number) => string;
};

/** 达标率：与 users/[id] 公开主页 `prog()` 同一个式子，两处不许分叉 */
function pctOf(c: Criterion): number {
  return c.need <= 0
    ? 100
    : Math.min(100, Math.round((c.cur / c.need) * 100));
}

/** 四项条件里最慢的那一项；`nc` 为空（已达最高级）回 null。
 *  单位词（小时/天）与三语文案由调用方注入，本函数不碰字典。 */
export function levelProgress(
  nc: NextClass | null | undefined,
  d: {
    up: string;
    dl: string;
    seed: string;
    age: string;
    hours: string;
    days: string;
  },
): LevelProgress | null {
  if (!nc) return null;
  // 只认「两个数都拿得到」的条件：镜像只滚了前端而 API 还按旧契约下发
  // （没有四项键）时，这里不收进列表，卡片退回「已达最高级」而不是
  // 把 NaN 画进宽度里。
  const list: Criterion[] = [
    {
      label: d.up,
      cur: nc.uploaded,
      need: nc.uploaded_need,
      fmt: (n) => formatBytes(n),
    },
    {
      label: d.dl,
      cur: nc.download_count,
      need: nc.download_count_need,
      fmt: (n) => String(n),
    },
    {
      label: d.seed,
      cur: nc.seed_hours,
      need: nc.seed_hours_need,
      fmt: (n) => `${n} ${d.hours}`,
    },
    {
      label: d.age,
      cur: nc.account_age_days,
      need: nc.account_age_days_need,
      fmt: (n) => `${n} ${d.days}`,
    },
  ];
  const usable = list.filter(
    (c) => Number.isFinite(c.cur) && Number.isFinite(c.need),
  );
  if (!usable.length) return null;
  const worst = usable.reduce(
    (a, b) => (pctOf(b) < pctOf(a) ? b : a),
    usable[0],
  );
  return {
    pct: pctOf(worst),
    text: `${worst.fmt(worst.cur)} / ${worst.fmt(worst.need)}`,
    gap: worst.fmt(Math.max(0, worst.need - worst.cur)),
    label: worst.label,
  };
}
