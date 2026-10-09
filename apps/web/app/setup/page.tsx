"use client";

import { useEffect, useState } from "react";
import { api, ApiError, setSessionCookie } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { SetupStepFinish } from "./_parts/step-finish";
import { SetupStepSite, type SiteDraft } from "./_parts/step-site";
import type { Status } from "./_parts/setup-types";
import { useSetupStatus } from "./_parts/use-setup-status";
import { SetupBootStatus } from "./_parts/setup-boot-status";

/**
 * 安装向导（U3 §8.3 + C4 四步）：① 选站型 ② 站名+管理员登录
 * ③ 新手运营模板（可选，0226） ④ 合规勾选完成。
 * 消费 GET /api/v1/setup/status 与 POST /api/v1/setup（后端已就绪）。
 * setup_done 已置位时显示已完成态（可重复访问，POST 幂等）。
 * 第二/四步拆至 ./_parts/step-site.tsx、step-finish.tsx（300 行门禁）；
 * 首屏加载（退避重试 + 错误保真）拆至 ./_parts/use-setup-status.ts，
 * 其加载态/失败诊断 UI 拆至 setup-boot-status.tsx。
 */

export default function SetupWizard() {
  const { dict } = useI18n();
  // setup 域文案（三语字典 setup 键；缺键回落中文兜底——向导页面在 i18n 域注册前可用）
  const setupDict = (
    dict as unknown as Record<string, Record<string, string> | undefined>
  ).setup;
  const t = (k: string, fallback: string) => setupDict?.[k] ?? fallback;

  const [step, setStep] = useState(1);
  const [pack, setPack] = useState("");
  // 新手运营模板（C4）："" 跳过 | strict | lenient
  const [preset, setPreset] = useState("");
  // 站点信息草稿（G12）：第二步由 step-site 采集，含临时密码就地改密后
  // 的新口令；第三步 finish 用它重登（改密会撤销全部旧 token）
  const [draft, setDraft] = useState<SiteDraft>({
    siteName: "",
    username: "",
    password: "",
    announceUrl: "",
  });
  const [ack, setAck] = useState(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [result, setResult] = useState<string>("");
  // 首个邀请码（P0-2.2）：向导完成时后端自动发的一枚，展示 + 一键复制
  const [firstInvite, setFirstInvite] = useState<string | null>(null);

  // 首屏加载：api 迁移未跑完时会连不上，退避重试 + 保留真实错误原因
  const boot = useSetupStatus(t);
  const { status } = boot;
  // 向导完成后就地置位（boot.status 只读，故本地维护一个覆盖值）
  const [doneOverride, setDoneOverride] = useState(false);
  const statusView: Status | null =
    status && doneOverride ? { ...status, done: true } : status;

  useEffect(() => {
    if (status?.done) setStep(4);
  }, [status]);

  async function finish() {
    setBusy(true);
    setError("");
    try {
      // 需先登录拿 token（向导完成端点要求 SITEPACKS_MANAGE）：登录响应
      // Set-Cookie 下发 HttpOnly flux_token，浏览器随后的同源请求自动携带
      await api.post("/api/v1/auth/login", {
        username: draft.username,
        password: draft.password,
      });
      setSessionCookie(true);
      const res = await api.post<{
        purged: [string, number][];
        extras: [string, number][];
        first_invite?: string | null;
      }>("/api/v1/setup", {
        pack,
        site_name: draft.siteName,
        announce_url: draft.announceUrl,
        onboarding_preset: preset,
        games_compliance_ack: ack,
      });
      const purged = res.purged?.map(([k, n]) => `${k}:${n}`).join(" ") ?? "";
      setResult(t("done", "安装完成") + (purged ? `（清理 ${purged}）` : ""));
      setFirstInvite(res.first_invite ?? null);
      setDoneOverride(true);
    } catch (e) {
      setError(e instanceof ApiError ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  }

  const btn = [
    "rounded-md px-4 py-2 text-sm font-medium transition-colors",
    "disabled:opacity-50 bg-[var(--accent)]",
    "text-[var(--accent-contrast)]",
  ].join(" ");

  // 首屏未拿到状态时也要渲染外壳（标题/步骤条/加载态），否则冷启动期
  // 是一个纯白页，站长既看不到标题也看不到「正在重试」，只会以为站点坏了

  return (
    <div className="mx-auto max-w-2xl px-4 py-12">
      <h1 className="text-xl font-semibold">🚀 {t("title", "站点安装向导")}</h1>
      <p className="mt-2 text-sm text-muted">
        {t("subtitle", "四步完成建站：选择站型 → 站点信息 → 新手模板 → 合规确认")}
      </p>

      {/* 步骤条 */}
      <ol className="mt-6 flex items-center gap-2 text-xs">
        {[1, 2, 3, 4].map((n) => (
          <li
            key={n}
            className={`flex items-center gap-1 rounded-full px-3 py-1 ${
              step === n
                ? "bg-[var(--accent)] text-[var(--accent-contrast)]"
                : "bg-[var(--panel)] text-muted"
            }`}
          >
            {n}.{" "}
            {t(
              `step${n}`,
              ["选择站型", "站点信息", "新手模板", "合规确认"][n - 1],
            )}
          </li>
        ))}
      </ol>

      {/* 首屏加载态 / 失败诊断：api 迁移未跑完时会命中（迁移在 bind 之前跑） */}
      <SetupBootStatus boot={boot} t={t} />

      {error && (
        <p className="mt-4 rounded-md border border-danger/40 bg-danger/10
          px-3 py-2 text-sm text-danger">
          {error}
        </p>
      )}

      {step === 1 && statusView && (
        <div className="mt-6">
          <div className="grid gap-3 sm:grid-cols-2">
            {statusView.packs.map((p) => (
              <button
                key={p.code}
                onClick={() => {
                  setPack(p.code);
                  setStep(2);
                }}
                className={`rounded-lg border p-4 text-left transition-colors hover:border-accent ${
                  pack === p.code ? "border-accent" : "border-line"
                }`}
              >
                <div className="font-medium">{p.name}</div>
                <div className="mt-1 text-xs text-muted">
                  {p.description}
                </div>
                <div className="mt-2 font-mono text-[10px] text-muted">
                  {p.code}
                </div>
              </button>
            ))}
          </div>
          {/* G12：站型可显式跳过——产品定位是「通用建站」，站长可能要从零搭；
              也修掉「包列表为空时卡在第一步」的死路 */}
          <div className="mt-4 text-center">
            <button
              className="rounded-md border border-line px-4 py-2 text-sm
                text-muted transition-colors hover:border-accent"
              onClick={() => {
                setPack("");
                setStep(2);
              }}
            >
              {t("skipPack", "跳过：暂不预设站型，用空站起步")}
            </button>
            <p className="mt-2 text-xs text-muted">
              {t(
                "skipPackHint",
                "跳过 = 不应用任何站型包（分类 / 模块保持系统默认）；之后可在后台「站点设定」页的「内容包」里随时导入。",
              )}
            </p>
          </div>
        </div>
      )}

      {step === 2 && (
        <SetupStepSite
          t={t}
          btn={btn}
          status={statusView}
          initial={draft}
          onBack={() => setStep(1)}
          onNext={(d) => {
            setDraft(d);
            setStep(3);
            // 改密已落库（must_reset 清位），但 boot.status 还是旧值——
            // 不重拉的话，用户从③/④退回②时会看到「再设一次新密码」，
            // 而能填的只有刚设过的那个 ⇒ 后端拒「新旧相同」⇒ 死锁。
            // 重拉后 root_temp_password=false ⇒ ②渲染成登录区（口令已预填）。
            boot.reload();
          }}
        />
      )}

      {step === 3 && (
        <div className="mt-6">
          <h2 className="text-base font-semibold">
            {t("presetTitle", "选择新手运营模板（可选）")}
          </h2>
          <p className="mt-1 text-xs text-muted">
            {t(
              "presetHint",
              "两种已被主流引擎验证的新手哲学，一键写入一组运营参数。随时可在后台切换或微调，不影响已有用户数据。",
            )}
          </p>
          <div className="mt-4 grid gap-3 sm:grid-cols-2">
            {(
              [
                ["strict", "presetStrict", "presetStrictDesc"],
                ["lenient", "presetLenient", "presetLenientDesc"],
              ] as const
            ).map(([code, nameKey, descKey]) => (
              <button
                key={code}
                onClick={() => {
                  setPreset(code);
                  setStep(4);
                }}
                className={`rounded-lg border p-4 text-left transition-colors hover:border-accent ${
                  preset === code ? "border-accent" : "border-line"
                }`}
              >
                <div className="font-medium">
                  {t(nameKey, code === "strict" ? "考核淘汰制" : "缓冲宽进制")}
                </div>
                <div className="mt-1 text-xs text-muted">
                  {t(
                    descKey,
                    code === "strict"
                      ? "高压高留存：新人考核自动派发 + H&R 从严 + 低保户降级"
                      : "宽进宽养：初始上传缓冲 + H&R 宽限预警 + 无考核",
                  )}
                </div>
              </button>
            ))}
          </div>
          <div className="mt-4 flex gap-2">
            {/* 此前第三步没有上一步：站名/announce 填错只能刷新重来，
                而此时密码已改 ⇒ 刷新后还得重新登录。补上回退。 */}
            <button className={btn} onClick={() => setStep(2)}>
              {t("prev", "上一步")}
            </button>
            <button
              className="rounded-md border border-line px-4 py-2 text-sm
                text-muted transition-colors hover:border-accent"
              onClick={() => {
                setPreset("");
                setStep(4);
              }}
            >
              {t("presetSkip", "跳过：保持现状，参数手动配置")}
            </button>
          </div>
        </div>
      )}

      {step === 4 && (
        <SetupStepFinish
          t={t}
          btn={btn}
          status={statusView}
          ack={ack}
          setAck={setAck}
          result={result}
          firstInvite={firstInvite}
          busy={busy}
          onFinish={finish}
          onPrev={() => setStep(3)}
        />
      )}

    </div>
  );
}
