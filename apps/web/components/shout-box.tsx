"use client";

import { useCallback, useEffect, useRef, useState } from "react";
import { api, ApiError } from "@/lib/api-client";
import { isStaffClass } from "@/lib/domain/user-class";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import { useI18n } from "@/i18n/client";
import { dateLocale } from "@/i18n/config";
import { Icon } from "@/components/icons";
import { ShoutEmojiBar } from "@/components/shout-emoji";
import { ShoutCommands, type BotCommand } from "@/components/shout-commands";
import { useBotReplies } from "@/components/shout-bot";
import {
  ShoutRow,
  ShoutBotRow,
  type Shout,
} from "@/components/shout-row";

/** 聊天盒（shoutbox.php 复刻）：最近 50 条 + 发言框（1-300 字），15s 自动刷新
 *  0078：消息以 / 开头时额外调用 GET /shoutbox/bot/exec?cmd= 并把回话插入聊天流
 *
 *  2026-11-02：加 active 开关——手机端首页把聊天收进抽屉，抽屉外那份
 *  （CSS 隐去）不该再每 15 秒轮询一次；抽屉里那份是独立实例，照常轮询。
 *
 *  2026-11-15：NP 惯例四件接线——发言冷却倒计时（后端 10s rate_window
 *  可见化）/ 行内删除（本人 2 分钟内+职员）/ 快捷表情 / /me 动作。 */
export function ShoutBox({ active = true }: { active?: boolean } = {}) {
  const { dict, locale } = useI18n();
  const t = dict.shoutbox;
  const tb = dict.shoutbot;
  const [items, setItems] = useState<Shout[] | null>(null);
  const [text, setText] = useState("");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const bot = useBotReplies();
  // 清空（0406 纯净化配套）：仅职员可见；danger 确认防手滑。
  // me.id：本人 2 分钟内撤回自己的发言（后端 shoutbox_delete 口径）
  const [meClass, setMeClass] = useState<number | null>(null);
  const [meId, setMeId] = useState<number | null>(null);
  const ask = useConfirm();
  useEffect(() => {
    api
      .get<{ id?: number; class_id?: number }>("/api/v1/me")
      .then((m) => {
        setMeClass(m.class_id ?? null);
        setMeId(m.id ?? null);
      })
      .catch(() => {});
  }, []);
  const [commands, setCommands] = useState<BotCommand[]>([]);
  const timer = useRef<ReturnType<typeof setInterval> | null>(null);
  // 可见刷新倒计时（秒级）：与轮询同一节拍，到 0 触发 load 并重置为 REFRESH_SECS
  const REFRESH_SECS = 15;
  const [countdown, setCountdown] = useState(REFRESH_SECS);
  // 发言冷却（后端 rate_window 10s 同拍）：发送成功即起倒计时，
  // 归零前发送钮禁用——把 429 变成可见的「等 N 秒」，不再靠报错体感
  const COOLDOWN_SECS = 10;
  const [cooldown, setCooldown] = useState(0);

  // P-314：拉取失败此前 setItems([]) ⇒ 留言板显示「暂无留言」，把故障演成空板；
  // 会员会以为自己的留言被清了。failed 只用来换那一句文案。
  const [failed, setFailed] = useState(false);
  const load = useCallback(async () => {
    setFailed(false);
    try {
      setItems(await api.get<Shout[]>("/api/v1/shoutbox"));
    } catch {
      setItems([]);
      setFailed(true);
    }
  }, []);
  useEffect(() => {
    if (!active) return;
    load();
    setCountdown(REFRESH_SECS);
    // 单节拍驱动：每秒递减倒计时，归 0 即刷新并重置。倒计时与轮询天然同步，
    // 避免两个独立 timer 各跑各的造成「数字到 0 了却没刷」的错位。
    timer.current = setInterval(() => {
      setCountdown((n) => {
        if (n <= 1) {
          load();
          return REFRESH_SECS;
        }
        return n - 1;
      });
    }, 1000);
    return () => {
      if (timer.current) clearInterval(timer.current);
    };
  }, [load, active]);

  // 命令列表（占位提示 + 快捷按钮）；失败静默
  useEffect(() => {
    api
      .get<{ commands: BotCommand[] }>("/api/v1/shoutbox/bot")
      .then((r) => setCommands(r.commands ?? []))
      .catch(() => {});
  }, []);

  // 发言冷却递减（独立于刷新轮询；active=false 的隐藏实例同样在跑——
  // 冷却是「距上次发言」的全局约束，与面板可见性无关）
  useEffect(() => {
    if (cooldown <= 0) return;
    const h = setTimeout(() => setCooldown((n) => n - 1), 1000);
    return () => clearTimeout(h);
  }, [cooldown]);

  const botHint = commands.length ? tb.commandsHint : "";

  async function send(rawArg?: string) {
    const raw = (rawArg ?? text).trim();
    if (!raw || busy) return;
    if (cooldown > 0) {
      setMsg(t.cooldown.replace("{n}", String(cooldown)));
      return;
    }
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/shoutbox", { message: raw });
      if (rawArg !== undefined) setText("");
      setCooldown(COOLDOWN_SECS);
      // /命令：额外触发机器人回话，展示在本会话聊天流。
      // /me 动作（带参数）不是机器人命令，跳过。
      if (raw.startsWith("/") && !raw.startsWith("/me ")) {
        try {
          await bot.exec(raw);
        } catch (e) {
          setMsg(bot.isApiErr(e) ? e.message : tb.replyFailed);
        }
      }
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function delOne(id: number) {
    if (busy) return;
    setBusy(true);
    setMsg(null);
    try {
      await api.post("/api/v1/shoutbox/delete", { id });
      setMsg(t.deleted);
      load();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  async function clearAll() {
    if (busy) return;
    setBusy(true);
    setMsg(null);
    try {
      const r = await api.post<{ cleared: number }>("/api/v1/shoutbox/clear");
      setMsg(t.clearDone.replace("{n}", String(r.cleared)));
      setItems([]);
      bot.reset();
    } catch (e) {
      setMsg(e instanceof ApiError ? e.message : dict.common.networkError);
    } finally {
      setBusy(false);
    }
  }

  const timeStr = (iso: string) =>
    new Date(iso).toLocaleTimeString(dateLocale(locale), {
      hour: "2-digit",
      minute: "2-digit",
    });

  const staff = isStaffClass(meClass ?? undefined);

  return (
    <section className="brand-panel shoutbox">
      <header className="brand-panel__head">
        <h2>
          <Icon
            name="messages"
            size={16}
            className="mr-1 inline align-[-3px] text-[var(--sky)]"
          />{" "}
          {t.title}
        </h2>
        <small className="flex items-center gap-2">
          {t.refreshIn
            ? t.refreshIn.replace("{n}", String(countdown))
            : t.note}
          {staff && (
            <button
              type="button"
              className={
                "text-xs text-[var(--danger-text)] underline " +
                "disabled:opacity-50"
              }
              disabled={busy || (items ?? []).length === 0}
              onClick={() =>
                ask.open({
                  tone: "danger",
                  title: t.clearAsk,
                  run: () => void clearAll(),
                })
              }
            >
              {t.clear}
            </button>
          )}
        </small>
      </header>
      <AskDialog {...ask} />
      <ul className="shoutbox__list">
        {(items ?? [])
          .slice()
          .reverse()
          .map((s) => (
            <ShoutRow
              key={s.id}
              s={s}
              meId={meId}
              staff={staff}
              busy={busy}
              onDel={(id) => void delOne(id)}
              timeStr={timeStr}
            />
          ))}
        {bot.replies.map((r) => (
          <ShoutBotRow
            key={`bot-${r.key}`}
            r={r}
            botName={tb.botName}
            timeStr={timeStr}
          />
        ))}
        {items !== null && items.length === 0 && bot.replies.length === 0 && (
          <li className="shoutbox__empty">
            {failed ? dict.common.loadFailed : t.empty}
          </li>
        )}
      </ul>
      {msg && <p className="shoutbox__msg">{msg}</p>}
      <form
        className="shoutbox__form"
        onSubmit={(e) => {
          e.preventDefault();
          void send();
        }}
      >
        {/* 2026-11-16：命令候选收编为「/」弹出钮（原按钮墙常驻占版面），
            与表情钮并排在输入行左端 */}
        {commands.length > 0 && (
          <ShoutCommands
            commands={commands}
            busy={busy}
            onRun={(cmd) => void send(cmd)}
            onFill={(cmd) => setText(cmd)}
          />
        )}
        <ShoutEmojiBar onPick={(emoji) => setText((v) => v + emoji)} />
        <input
          value={text}
          onChange={(e) => setText(e.target.value)}
          maxLength={300}
          placeholder={botHint || t.mePlaceholder}
          aria-label={t.title}
        />
        <button
          type="submit"
          className="brand-button"
          disabled={busy || cooldown > 0 || !text.trim()}
          title={cooldownHint(cooldown, t.cooldown)}
        >
          {cooldown > 0
            ? t.cooldownShort.replace("{n}", String(cooldown))
            : t.send}
        </button>
      </form>
    </section>
  );
}

/** 冷却中发送钮的悬浮提示（抽函数守行宽） */
function cooldownHint(secs: number, tpl: string) {
  return secs > 0 ? tpl.replace("{n}", String(secs)) : undefined;
}
