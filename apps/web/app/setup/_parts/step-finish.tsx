"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import type { Status } from "./setup-types";

/**
 * 向导第三步：合规确认 → 完成结果（邀请码/落点按钮）→ 开站检查清单。
 * 从 setup/page.tsx 按域拆出（300 行门禁）；状态与完成动作由主组件注入。
 */
export function SetupStepFinish({
  t,
  btn,
  status,
  ack,
  setAck,
  result,
  firstInvite,
  busy,
  onFinish,
  onPrev,
}: {
  t: (k: string, fallback: string) => string;
  btn: string;
  status: Status | null;
  ack: boolean;
  setAck: (v: boolean) => void;
  result: string;
  firstInvite: string | null;
  busy: boolean;
  onFinish: () => void;
  onPrev: () => void;
}) {
  const router = useRouter();
  const [copied, setCopied] = useState(false);
  return (
    <div className="mt-6 space-y-4">
      {status?.done && !result && (
        <p
          className="rounded-md border border-success/40 bg-success/10 px-3
            py-2 text-sm"
        >
          {t("alreadyDone", "本站已完成安装向导（重复完成幂等）。")}
        </p>
      )}
      <label
        className="flex items-start gap-2 rounded-md border border-line p-3
          text-sm"
      >
        <input
          type="checkbox"
          className="mt-0.5"
          checked={ack}
          onChange={(e) => setAck(e.target.checked)}
          disabled={status?.done}
        />
        <span>
          {t(
            "ackText",
            "我确认已了解：本系统内置的娱乐玩法（刮刮乐/猜大小等机会类" +
              "游戏）使用站内虚拟货币，启用前我已自行确认并遵守所在司法" +
              "域关于机会类游戏的法律法规。",
          )}
        </span>
      </label>
      {result && (
        <div
          className="rounded-md border border-success/40 bg-success/10 px-3
            py-3 text-sm"
        >
          <p>✅ {result}</p>
          {/* P0-2.2：后端自动产的首个邀请码——注册死锁的解口，展示+一键复制 */}
          {firstInvite && (
            <div
              className="mt-2 rounded-md border border-line
                bg-[var(--panel)] px-3 py-2"
            >
              <p className="text-xs font-medium">
                {t(
                  "firstInviteTitle",
                  "已生成首个邀请码（本站为邀请制注册）：",
                )}
              </p>
              <div className="mt-1 flex items-center gap-2">
                <code className="break-all font-mono text-xs">
                  {firstInvite}
                </code>
                <button
                  className="rounded border border-line px-2 py-0.5 text-xs"
                  onClick={() => {
                    navigator.clipboard?.writeText(firstInvite);
                    setCopied(true);
                    setTimeout(() => setCopied(false), 1500);
                  }}
                >
                  {copied ? t("copied", "已复制") : t("copy", "复制")}
                </button>
              </div>
              <p className="mt-1 text-xs text-muted">
                {t(
                  "firstInviteHint",
                  "把它交给第一个注册的用户（注册页填入即可）；更多邀请码在后台「邀请管理」发放。",
                )}
              </p>
            </div>
          )}
          {/* 0208 P0：向导终点给落点——别让站长停在原地；
              G13：补「导入内容包」——首装只有站型包一条内容来源，
              不衔接到「站点设定 → 内容包」对话框，站长不知道还能导入分类/主题 */}
          <div className="mt-3 flex flex-wrap gap-2">
            <button
              className={btn}
              onClick={() => router.push("/torrents")}
            >
              {t("goHome", "进入站点")}
            </button>
            <button className={btn} onClick={() => router.push("/admin")}>
              {t("goAdmin", "去管理后台")}
            </button>
            <button className={btn} onClick={() => router.push("/upload")}>
              {t("goUpload", "发布第一颗种子")}
            </button>
            <button
              className={btn}
              onClick={() => router.push("/admin/settings?packs=1")}
            >
              {t("goPacks", "导入内容包")}
            </button>
          </div>
          <p className="mt-2 text-xs text-muted">
            {t(
              "nextSteps",
              "开站前建议检查：后台「站点设定」里的 tracker announce " +
                "地址、邮件 SMTP、注册模式三项；更多分类/主题等内容包" +
                "也可在「站点设定 → 内容包」导入。",
            )}
          </p>
          {/* 冷启动收尾联动：用 IP 直连 :3000 完成向导（无域名/反代）时，
              服务器还在裸奔——给出「收回 3000」的三步指引。域名访问则不显示。 */}
          {typeof window !== "undefined" &&
            (/^\d+\.\d+\.\d+\.\d+$/.test(window.location.hostname) ||
              window.location.hostname === "localhost") && (
              <div
                className="mt-3 rounded-md border border-warn/40 bg-warn/10
                  px-3 py-2 text-left text-xs"
              >
                <p className="font-medium">
                  {t("ipAccessTitle", "你现在是 IP 直连（3000 对外网敞开）")}
                </p>
                <p className="mt-1 text-muted">
                  {t(
                    "ipAccessHint",
                    "正式开站前建议：① 宝塔/1Panel 建站反代到 " +
                      "127.0.0.1:3000 并配 HTTPS 证书；② 后台 Tracker 地址改" +
                      "成 https://你的域名；③ 服务器 docker/.env 删掉 " +
                      "WEB_BIND=0.0.0.0 那行（或重跑 quick-deploy.sh 自动收" +
                      "回）并重启栈，云安全组关掉 3000。",
                  )}
                </p>
              </div>
            )}
        </div>
      )}
      {/* 开站 checklist（0209 P2-12）：装完就有可操作的警示卡。
          0214 链接化：每条警示直达对应设定分组，不再让站长自己找路 */}
      {status?.done && status.checklist && !result && (
        <div
          className="rounded-md border border-warn/40 bg-warn/10 px-3 py-3
            text-sm"
        >
          <p className="font-medium">
            {t("checklistTitle", "开站检查清单")}
          </p>
          <ul className="mt-2 list-disc space-y-1 pl-5 text-xs">
            {status.checklist.announce_local && (
              <li>
                {t(
                  "chkAnnounce",
                  "tracker announce 地址仍是本地回环（127.0.0.1）——用户下载的种子文件将无法做种，",
                )}
                <a
                  className="underline"
                  href="/admin/settings?group=basic"
                  target="_blank"
                  rel="noreferrer"
                >
                  {t("chkGoFix", "去「基础设定 → Tracker 地址」修改")}
                </a>
              </li>
            )}
            {status.checklist.smtp_unset && (
              <li>
                {t(
                  "chkSmtp",
                  "邮件 SMTP 未配置——找回密码 / 邀请函 / 群发都将静默跳过，",
                )}
                <a
                  className="underline"
                  href="/admin/settings?group=smtp"
                  target="_blank"
                  rel="noreferrer"
                >
                  {t("chkGoSmtp", "去配置 SMTP")}
                </a>
              </li>
            )}
            <li>
              {t("chkReg", "注册模式：")}{" "}
              <b>{status.checklist.registration_mode}</b>
              {status.checklist.registration_mode === "invite_only"
                ? t(
                    "chkRegInvite",
                    "（邀请制：需要先在后台「邀请管理」发码）",
                  )
                : t("chkRegOpen", "（开放注册）")}
            </li>
          </ul>
        </div>
      )}
      <div className="flex gap-2">
        <button className={btn} onClick={onPrev} disabled={status?.done}>
          {t("prev", "上一步")}
        </button>
        <button
          className={btn}
          disabled={!ack || busy || status?.done}
          onClick={onFinish}
        >
          {busy ? t("working", "执行中…") : t("finish", "完成安装")}
        </button>
      </div>
    </div>
  );
}
