import { dateLocale, type Locale } from "@/i18n/config";

/** 账户健康页的日期显示：只到日，空值统一破折号。 */
export function day(locale: Locale, iso: string | null | undefined) {
  if (!iso) return "—";
  return new Date(iso).toLocaleDateString(dateLocale(locale), {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
  });
}
