import Link from "next/link";
import { getDict } from "@/i18n/server";
import { getSiteProfile } from "@/lib/site-profile";
import { Icon } from "@/components/icons";
import { MaintenanceCountdown } from "@/components/maintenance-countdown";

export const dynamic = "force-dynamic";

/** 维护页（0462，NP site_down_for_maintenance / Unit3D 503 同口径）。
 *
 *  数据全部来自 site-profile 的 maintenance 块（该端点在 API 维护闸门
 *  白名单里）：on/message/until。until 为空不显示倒计时——不造假 ETA
 *  的口径与 offline 页那条挂账一致（本页正是它的数据源落地）。
 *  tracker 不受影响：做种/下载继续，文案里明说，别让用户卸客户端。
 *  staff 不落这页（(main) 布局只重定向非 staff），但直链进来也照常
 *  渲染——登录/登出入口对 staff 保留。 */
export default async function MaintenancePage() {
  const { dict } = await getDict();
  const profile = await getSiteProfile().catch(() => null);
  const m = profile?.maintenance;
  const t = dict.maintenance;
  const message = m?.message?.trim() || t.defaultMessage;
  return (
    <main
      id="main-content"
      className="mx-auto flex max-w-md flex-col items-center gap-4
        py-16 text-center"
    >
      <Icon name="seed" size={64} className="text-[var(--sky)]" />
      <h1 className="font-display text-2xl">{t.title}</h1>

      <p className="whitespace-pre-line text-sm text-sub">{message}</p>

      {m?.until ? (
        <>
          <MaintenanceCountdown until={m.until} />
          <p className="text-xs text-sub">{t.countdownNote}</p>
        </>
      ) : null}

      <p className="text-sm text-sub">{t.seedNote}</p>

      <div className="flex flex-wrap items-center justify-center gap-3">
        <Link href="/login" className={GHOST_LINK}>
          {t.login}
        </Link>
        <Link href="/" className={GHOST_LINK}>
          {t.home}
        </Link>
      </div>

      <p className="text-xs text-sub">{t.footerNote}</p>
    </main>
  );
}

const GHOST_LINK =
  "min-h-[44px] inline-flex items-center rounded-full border border-line" +
  " px-6 font-bold";
