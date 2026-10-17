import Link from "next/link";
import { getDict } from "@/i18n/server";
import { Icon } from "@/components/icons";
import { OfflineRetry } from "@/components/offline-retry";

/** 离线回退页（M26：SW 导航失败时的兜底）。
 *
 *  2026-10-17 对齐样图 § offline：原先只有一句文案 +「回首页」，样图那页给的是
 *  「预计恢复时间 + 倒计时 + 联系管理组 / 查看 FAQ」。倒计时这栏**没做**，
 *  因为本站没有任何维护窗口数据源（site_settings 里没有相关键、api 也没有
 *  维护状态端点，全仓搜 maint/site_offline/维护 零业务命中）——造一个假 ETA
 *  或写死「2026-10-18 06:00」会比少一个倒计时伤害大得多，这条挂账等站长拍板。
 *  其余出口都补齐：重新连接（真 reload）、联系管理组、常见问题、回首页。 */
export default async function OfflinePage() {
  const { dict } = await getDict();
  const t = dict.offline;
  return (
    <main
      id="main-content"
      className="mx-auto flex max-w-md flex-col items-center gap-4 py-16 text-center"
    >
      <Icon name="seed" size={64} className="text-[var(--sky)]" />
      <h1 className="font-display text-2xl">{t.title}</h1>
      <p className="text-sm text-sub">{t.subtitle}</p>

      <div className="flex flex-wrap items-center justify-center gap-3">
        <OfflineRetry />
        <Link href="/" className={GHOST_LINK}>
          {t.home}
        </Link>
      </div>

      <p className="text-xs text-sub">{t.needNet}</p>
      <div className="flex flex-wrap items-center justify-center gap-4 text-sm">
        <Link href="/contactstaff" className={TEXT_LINK}>
          {t.contact}
        </Link>
        <Link href="/faq" className={TEXT_LINK}>
          {t.faq}
        </Link>
      </div>
    </main>
  );
}

const GHOST_LINK =
  "min-h-[44px] inline-flex items-center rounded-full border border-line px-6 font-bold";
const TEXT_LINK = "font-bold text-[var(--sky)] underline";
