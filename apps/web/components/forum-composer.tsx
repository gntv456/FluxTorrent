"use client";

import { useEffect, useState } from "react";
import { useRouter } from "next/navigation";
import { api, ApiError } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { TagChip, TagChipData } from "@/components/forum-bits";
import { VideoInsertRow } from "@/components/forum-video-insert";
import { Modal } from "@/components/modal";

/** 论坛发主题组件群（从 forum-composer.tsx 按域拆出）：
 *  发帖选标签 TagPicker + 发主题 TopicComposer（悬赏/投票/抽奖三型，
 *  0124/0125/0126）。视频插入行在 forum-video-insert.tsx；回帖/管理操作
 *  见 forum-post-actions.tsx。 */

/** 可发帖版块精简项（GET /forums 过滤 can_create 后再用分区分组） */
type CreatableBoard = {
  id: number;
  name: string;
  can_create?: boolean;
  category_id?: number | null;
  category_name?: string | null;
};

/** 版块按分区 optgroup：有分区名的进组，其余落「未分组」；
 *  分组按出现顺序稳定（/forums 本身按 sort 返回）。 */
function boardGroups(boards: CreatableBoard[], ungrouped: string) {
  const groups: { key: string; name: string; items: CreatableBoard[] }[] = [];
  const byKey = new Map<string, (typeof groups)[number]>();
  for (const b of boards) {
    const key =
      b.category_id != null && b.category_name
        ? `cat-${b.category_id}`
        : "cat-none";
    let g = byKey.get(key);
    if (!g) {
      g = { key, name: b.category_name ?? ungrouped, items: [] };
      byKey.set(key, g);
      groups.push(g);
    }
    g.items.push(b);
  }
  return groups;
}

/** 发主题选标签：字典自取（公开接口 /forums/tags，含 tag_dict 样式列），最多选 5 个 */
function TagPicker({
  selected,
  onToggle,
}: {
  selected: number[];
  onToggle: (id: number) => void;
}) {
  const { dict } = useI18n();
  const [dictRows, setDictRows] = useState<TagChipData[]>([]);

  useEffect(() => {
    let alive = true;
    api
      .get<TagChipData[]>("/api/v1/forums/tags")
      .then((r) => {
        if (alive) setDictRows(r ?? []);
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, []);

  if (dictRows.length === 0) return null;
  const t = dict.forums;
  const full = selected.length >= 5;

  return (
    <div className="flex flex-col gap-1">
      <span className="text-sm text-sub">{t.tagPick}</span>
      <div className="flex flex-wrap gap-1.5">
        {dictRows.map((d) => {
          const on = selected.includes(d.id);
          return (
            <button
              key={d.id}
              type="button"
              onClick={() => onToggle(d.id)}
              disabled={!on && full}
              aria-pressed={on}
              className={`rounded-full transition disabled:cursor-not-allowed disabled:opacity-40 ${
                on
                  ? "outline outline-2 outline-offset-1 outline-sky"
                  : "opacity-70 hover:opacity-100"
              }`}
              title={on ? t.tagOn : t.tagOff}
            >
              <TagChip tag={d} />
            </button>
          );
        })}
      </div>
    </div>
  );
}

/** 发主题弹窗。forumId 传入 = 版块页入口（锁定该版块）；
 *  不传 = 全局入口（如论坛首页「发布新帖」），弹窗内先选版块。
 *  对接 POST /forums/topics */
export function TopicComposer({ forumId }: { forumId?: number }) {
  const { dict } = useI18n();
  const router = useRouter();
  const [open, setOpen] = useState(false);
  const [picked, setPicked] = useState<number | null>(forumId ?? null);
  // 全局入口：懒拉可发帖版块（can_create = 读/回/发三档全过 或 版主）；
  // null = 版块页入口（无需选择器）
  const [boards, setBoards] = useState<CreatableBoard[] | null>(
    forumId != null ? null : [],
  );
  const [title, setTitle] = useState("");
  const [body, setBody] = useState("");
  const [tags, setTags] = useState<number[]>([]);
  // 悬赏（0124）：选了金额即 bounty 类型；金额空 = 普通帖
  const [bounty, setBounty] = useState("");
  // 投票（0125）：填了 ≥2 个选项即 poll 类型
  const [pollOpts, setPollOpts] = useState(["", ""]);
  // 抽奖（0126）：填了名额+奖金即 lottery 类型
  const [lotWinners, setLotWinners] = useState("");
  const [lotPrize, setLotPrize] = useState("");
  const [lotTicket, setLotTicket] = useState("0");
  const [lotHours, setLotHours] = useState("24");
  const [msg, setMsg] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  // 全局入口：首次打开弹窗时拉一次可发帖版块（/forums 已带 can_create）
  useEffect(() => {
    if (forumId != null || !open || boards === null || boards.length > 0)
      return;
    let alive = true;
    api
      .get<{ forums: CreatableBoard[] }>("/api/v1/forums")
      .then((r) => {
        if (alive) setBoards((r?.forums ?? []).filter((f) => f.can_create));
      })
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [open, forumId, boards]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setMsg(null);
    const bountyNum = Math.floor(Number(bounty));
    const opts = pollOpts.map((s) => s.trim()).filter(Boolean);
    const lw = Math.floor(Number(lotWinners));
    const lp = Math.floor(Number(lotPrize));
    const lt = Math.floor(Number(lotTicket) || 0);
    const lh = Math.floor(Number(lotHours) || 24);
    const isLottery =
      Number.isFinite(lw) && lw > 0 && Number.isFinite(lp) && lp > 0;
    try {
      const r = await api.post<{ topic_id: number }>("/api/v1/forums/topics", {
        forum_id: picked,
        title: title.trim(),
        body: body.trim(),
        tags,
        ...(Number.isFinite(bountyNum) && bountyNum > 0
          ? { topic_type: "bounty", bounty_spark: bountyNum }
          : {}),
        ...(opts.length >= 2 ? { topic_type: "poll", poll_options: opts } : {}),
        ...(isLottery
          ? {
              topic_type: "lottery",
              lottery_winners: lw,
              lottery_prize: lp,
              lottery_ticket: lt,
              lottery_hours: lh,
            }
          : {}),
      });
      router.push(`/forums/topic/${r.topic_id}`);
    } catch (err) {
      // 后端校验消息（如敏感词提示「内容包含敏感词…」）比字典的通用 1002 文案更有用，优先透传
      setMsg(
        err instanceof ApiError && err.message
          ? err.message
          : dict.common.networkError,
      );
      setBusy(false);
    }
  }

  if (!open) {
    return (
      <button
        type="button"
        onClick={() => setOpen(true)}
        className={
          forumId != null
            ? "min-h-[44px] self-start rounded-full bg-coral px-5 text-sm font-bold text-white active:scale-[0.97]"
            : "baozi-button"
        }
      >
        {dict.forums.newTopic}
      </button>
    );
  }

  return (
    <Modal
      open={open}
      onClose={() => setOpen(false)}
      title={dict.forums.newTopic}
      size="lg"
    >
      <form onSubmit={submit} className="flex flex-col gap-3">
        {boards !== null && (
          <label className="flex flex-col gap-1">
            <span className="text-sm text-sub">{dict.forums2.pickBoard}</span>
            <select
              value={picked ?? ""}
              onChange={(e) => setPicked(Number(e.target.value))}
              required
              className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
            >
              <option value="" disabled>
                {dict.forums2.pickBoardPh}
              </option>
              {boards.length === 0 && (
                <option value="">{dict.forums2.noBoard}</option>
              )}
              {boardGroups(boards, dict.forums.uncategorized).map((g) => (
                <optgroup key={g.key} label={g.name}>
                  {g.items.map((b) => (
                    <option key={b.id} value={b.id}>
                      {b.name}
                    </option>
                  ))}
                </optgroup>
              ))}
            </select>
          </label>
        )}
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.forums.topicTitle}</span>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            required
            maxLength={120}
            className="min-h-[44px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
          />
        </label>
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.forums.topicBody}</span>
          <textarea
            value={body}
            onChange={(e) => setBody(e.target.value)}
            required
            rows={5}
            maxLength={5000}
            className="min-h-[100px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 py-2 text-sm outline-none focus:border-sky"
          />
        </label>
        <TagPicker
          selected={tags}
          onToggle={(id) =>
            setTags((ts) =>
              ts.includes(id) ? ts.filter((x) => x !== id) : [...ts, id],
            )
          }
        />
        {/* 悬赏（0124）：填金额即悬赏帖，冻结立扣；留空 = 普通帖 */}
        <label className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.forums.bountyLabel}</span>
          <input
            type="number"
            min={0}
            max={1000000}
            value={bounty}
            onChange={(e) => setBounty(e.target.value)}
            placeholder={dict.forums.bountyPh}
            className="min-h-[44px] w-44 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
          />
        </label>
        {/* 投票（0125）：≥2 个非空选项即投票帖；选项发帖后定死 */}
        <div className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.forums.pollLabel}</span>
          <div className="flex flex-col gap-1">
            {pollOpts.map((v, i) => (
              <input
                key={i}
                value={v}
                onChange={(e) =>
                  setPollOpts((os) =>
                    os.map((x, j) => (j === i ? e.target.value : x)),
                  )
                }
                maxLength={60}
                placeholder={`${dict.forums.pollOption} ${i + 1}`}
                className="min-h-[38px] rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
              />
            ))}
            {pollOpts.length < 10 && (
              <button
                type="button"
                onClick={() => setPollOpts((os) => [...os, ""])}
                className="self-start rounded-full border border-line px-3 py-1 text-xs font-bold text-sub hover:border-sky hover:text-sky"
              >
                + {dict.forums.pollAddOption}
              </button>
            )}
          </div>
        </div>
        {/* 抽奖（0126）：填名额+奖金即抽奖帖（奖金池发布时冻结）；票价 0=免费参与 */}
        <div className="flex flex-col gap-1">
          <span className="text-sm text-sub">{dict.forums.lotLabel}</span>
          <div className="flex flex-wrap gap-2">
            <input
              type="number"
              min={1}
              max={100}
              value={lotWinners}
              onChange={(e) => setLotWinners(e.target.value)}
              placeholder={dict.forums.lotWinners}
              className="min-h-[44px] w-28 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
            />
            <input
              type="number"
              min={1}
              max={100000}
              value={lotPrize}
              onChange={(e) => setLotPrize(e.target.value)}
              placeholder={dict.forums.lotPrize}
              className="min-h-[44px] w-32 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
            />
            <input
              type="number"
              min={0}
              max={10000}
              value={lotTicket}
              onChange={(e) => setLotTicket(e.target.value)}
              placeholder={dict.forums.lotTicket}
              className="min-h-[44px] w-32 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
            />
            <input
              type="number"
              min={1}
              max={720}
              value={lotHours}
              onChange={(e) => setLotHours(e.target.value)}
              placeholder={dict.forums.lotHours}
              className="min-h-[44px] w-28 rounded-[var(--r-sm)] border border-line bg-cloud px-3 text-sm outline-none focus:border-sky"
            />
          </div>
        </div>
        {/* 视频内嵌（0189/0190）：白名单链接或本地上传，插入 !video() 独立行 */}
        <VideoInsertRow
          onInsert={(line) => setBody((b) => `${b}${b ? "\n" : ""}${line}\n`)}
        />
        {msg && (
          <p role="alert" className="text-sm text-danger">
            {msg}
          </p>
        )}
        <div className="flex gap-2">
          <button
            type="submit"
            disabled={busy}
            className="min-h-[44px] rounded-full bg-sky px-5 text-sm font-bold text-white active:scale-[0.97] disabled:opacity-50"
          >
            {busy ? dict.forums.posting : dict.forums.submitTopic}
          </button>
          <button
            type="button"
            onClick={() => setOpen(false)}
            className="min-h-[44px] rounded-full border border-line px-5 text-sm text-sub active:scale-[0.97]"
          >
            {dict.forums.cancel}
          </button>
        </div>
      </form>
    </Modal>
  );
}
