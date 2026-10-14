"use client";

import type { RefObject } from "react";
import { useI18n } from "@/i18n/client";
import { useConfirm, AskDialog } from "@/components/ui/use-confirm";
import { FormRow, fieldCls } from "@/components/upload-form-parts";

/** 发布表单·简介与 BBCode 工具条（从 upload-form.tsx 按域拆出，300 行门禁）：
 *  NP bbcode 口径工具条（颜色/字体/字号/B/I/U/S/链接/图片/引用/代码/表情）
 *  + 简介 textarea。descr 状态与 descrRef 由主表单传入（submit 读取）。 */

export function UploadDescrBlock({
  descr,
  setDescr,
  descrRef,
}: {
  descr: string;
  setDescr: React.Dispatch<React.SetStateAction<string>>;
  descrRef: RefObject<HTMLTextAreaElement | null>;
}) {
  const { dict } = useI18n();
  // P2 清偿：图片 URL 采集（window.prompt 退役 → ConfirmDialog 输入槽）
  const ask = useConfirm();

  // BBCode 工具条（NP bbcode 口径）：包住选区 / 插入
  function bbWrap(open: string, close: string, ph = "") {
    const el = descrRef.current;
    const s = el?.selectionStart ?? descr.length;
    const e = el?.selectionEnd ?? descr.length;
    const sel = descr.slice(s, e) || ph;
    setDescr(descr.slice(0, s) + open + sel + close + descr.slice(e));
    requestAnimationFrame(() => {
      if (!el) return;
      el.focus();
      el.selectionStart = s + open.length;
      el.selectionEnd = s + open.length + sel.length;
    });
  }
  function bbInsert(txt: string) {
    const el = descrRef.current;
    const s = el?.selectionStart ?? descr.length;
    const e = el?.selectionEnd ?? s;
    setDescr(descr.slice(0, s) + txt + descr.slice(e));
    requestAnimationFrame(() => {
      if (!el) return;
      el.focus();
      el.selectionStart = el.selectionEnd = s + txt.length;
    });
  }
  const bbBtn =
    "min-h-10 min-w-[32px] rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-1.5 text-xs font-bold text-ink hover:border-[var(--baozi-orange)]";
  const bbSelect =
    "min-h-10 rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-1 text-xs text-ink outline-none focus:border-[var(--baozi-orange)]";
  // 色板（NP 颜色面板口径）：色块按钮，点击包住选区。
  // W-36：BBCode 色板是「用户内容词汇」——写进简介的 [color=red] 在
  // 夜间主题也必须是红，不随主题换肤；色值以 CSS 变量集中在 base.css
  // --content-bb-* 层（明暗同值，改色只动令牌层一处）。
  const BB_COLORS: [string, string][] = [
    ["var(--content-bb-ink)", "黑"],
    ["var(--content-bb-gray)", "灰"],
    ["var(--content-bb-white)", "白"],
    ["var(--content-bb-red)", "红"],
    ["var(--content-bb-red-deep)", "深红"],
    ["var(--content-bb-orange)", "橙"],
    ["var(--content-bb-yellow)", "黄"],
    ["var(--content-bb-gold)", "金"],
    ["var(--content-bb-green)", "绿"],
    ["var(--content-bb-teal)", "青绿"],
    ["var(--content-bb-blue)", "蓝"],
    ["var(--content-bb-indigo)", "靛"],
    ["var(--content-bb-purple)", "紫"],
    ["var(--content-bb-pink)", "粉"],
    ["var(--content-bb-brown)", "棕"],
    ["var(--content-bb-sky)", "天蓝"],
  ];
  const BB_EMOJIS = [
    "😄",
    "😂",
    "🥰",
    "😮",
    "😭",
    "😅",
    "😡",
    "👍",
    "🙏",
    "🎉",
    "🔥",
    "❤️",
    "🤔",
    "💯",
    "🍺",
  ];
  // 下拉面板通用样式：固定宽度 + 网格（防在窄单元格里竖排成一列）
  const bbPanel =
    "absolute z-10 mt-1 grid w-56 max-w-[calc(100vw-2.5rem)] gap-1 rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] p-2 shadow-[var(--shadow-card)]";
  const bbSwatch =
    "flex h-7 w-full cursor-pointer items-center justify-center gap-1 rounded-[var(--r-sm)] border border-[var(--baozi-line)] text-[12px] text-ink hover:border-[var(--baozi-orange)]";

  return (
    <FormRow label={dict.upload.descr}>
      <div className="flex flex-col gap-1">
        <div className="flex flex-wrap items-center gap-1">
          <details className="relative">
            <summary
              className={`${bbBtn} inline-flex cursor-pointer list-none items-center justify-center gap-1`}
              title={dict.upload.bbColor}
            >
              🎨 {dict.upload.bbColor}
            </summary>
            <div className={`${bbPanel} grid-cols-8 w-72`}>
              {BB_COLORS.map(([hex, label]) => (
                <button
                  key={hex}
                  type="button"
                  title={label}
                  className={bbSwatch}
                  style={
                    // 白色块需要描边区分底色，其余色块白字可读
                    hex === "var(--content-bb-white)"
                      ? { background: "#fff" }
                      : { background: hex, color: "#fff" }
                  }
                  onClick={() => {
                    bbWrap(`[color=${label}]`, "[/color]");
                    const d = document.activeElement?.closest("details");
                    if (d instanceof HTMLDetailsElement) d.open = false;
                  }}
                >
                  {label}
                </button>
              ))}
            </div>
          </details>
          <select
            value=""
            onChange={(e) => {
              if (e.target.value) bbWrap(`[font=${e.target.value}]`, "[/font]");
            }}
            className={bbSelect}
            title={dict.upload.bbFont}
          >
            <option value="">{dict.upload.bbFont}</option>
            <option value="SimSun">宋体</option>
            <option value="KaiTi">楷体</option>
            <option value="SimHei">黑体</option>
            <option value="serif">Serif</option>
            <option value="monospace">等宽</option>
          </select>
          <select
            value=""
            onChange={(e) => {
              if (e.target.value) bbWrap(`[size=${e.target.value}]`, "[/size]");
            }}
            className={bbSelect}
            title={dict.upload.bbSize}
          >
            <option value="">{dict.upload.bbSize}</option>
            {[1, 2, 3, 4, 5, 6, 7, 8].map((n) => (
              <option key={n} value={n}>
                {n} 号
              </option>
            ))}
          </select>
          <button
            type="button"
            className={`${bbBtn} font-black`}
            onClick={() => bbWrap("[b]", "[/b]")}
            title="Bold"
          >
            B
          </button>
          <button
            type="button"
            className={`${bbBtn} italic`}
            onClick={() => bbWrap("[i]", "[/i]")}
            title="Italic"
          >
            I
          </button>
          <button
            type="button"
            className={`${bbBtn} underline`}
            onClick={() => bbWrap("[u]", "[/u]")}
            title="Underline"
          >
            U
          </button>
          <button
            type="button"
            className={`${bbBtn} line-through`}
            onClick={() => bbWrap("[s]", "[/s]")}
            title="Strikethrough"
          >
            S
          </button>
          <button
            type="button"
            className={bbBtn}
            onClick={() => bbWrap("[url]", "[/url]", "https://")}
            title={dict.upload.bbLink}
          >
            🔗
          </button>
          <button
            type="button"
            className={bbBtn}
            title={dict.upload.bbImg}
            onClick={() =>
              ask.open({
                title: dict.upload.bbImg,
                label: dict.upload.bbImg,
                run: (u) => {
                  if (u.trim()) bbInsert(`[img]${u.trim()}[/img]`);
                },
              })
            }
          >
            🖼️
          </button>
          <button
            type="button"
            className={bbBtn}
            onClick={() => bbWrap("[quote]", "[/quote]")}
            title={dict.upload.bbQuote}
          >
            ❝
          </button>
          <button
            type="button"
            className={bbBtn}
            title={dict.upload.bbCode}
            onClick={() =>
              bbWrap(
                "[code]",
                "[/code]",
                "MediaInfo / General / Complete name …",
              )
            }
          >
            {"</>"}
          </button>
          <details className="relative">
            <summary
              className={`${bbBtn} inline-flex cursor-pointer list-none items-center justify-center`}
              title={dict.upload.bbEmoji}
            >
              😀
            </summary>
            <div className={`${bbPanel} grid-cols-5`}>
              {BB_EMOJIS.map((em) => (
                <button
                  key={em}
                  type="button"
                  className="rounded-[var(--r-sm)] py-1 text-lg hover:bg-[var(--head-b)]"
                  onClick={() => {
                    bbInsert(em);
                    const d = document.activeElement?.closest("details");
                    if (d instanceof HTMLDetailsElement) d.open = false;
                  }}
                >
                  {em}
                </button>
              ))}
            </div>
          </details>
        </div>
        <textarea
          ref={descrRef}
          value={descr}
          onChange={(e) => setDescr(e.target.value)}
          placeholder={dict.upload.descrPlaceholder}
          rows={9}
          maxLength={30000}
          className={fieldCls}
        />
        <span className="text-xs text-sub">
          {dict.upload.descrHint}（BBCode：
          <code>[b][i][color=][size=][url][img][quote][code]</code> 均受支持）
        </span>
      </div>
      <AskDialog {...ask} />
    </FormRow>
  );
}
