"use client";

import type { RefObject } from "react";
import { useI18n } from "@/i18n/client";
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
    "min-h-[28px] min-w-[32px] rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-1.5 text-xs font-bold text-ink hover:border-[var(--baozi-orange)]";
  const bbSelect =
    "min-h-[28px] rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] px-1 text-xs text-ink outline-none focus:border-[var(--baozi-orange)]";
  // 色板（NP 颜色面板口径）：色块按钮，点击包住选区
  const BB_COLORS: [string, string][] = [
    ["#111827", "黑"], ["#6b7280", "灰"], ["#ffffff", "白"], ["#e02020", "红"],
    ["#a61b29", "深红"], ["#f59e0b", "橙"], ["#fadb14", "黄"], ["#d4a017", "金"],
    ["#16a34a", "绿"], ["#0d9488", "青绿"], ["#2563eb", "蓝"], ["#4f46e5", "靛"],
    ["#9333ea", "紫"], ["#eb2f96", "粉"], ["#8b4513", "棕"], ["#0ea5e9", "天蓝"],
  ];
  const BB_EMOJIS = ["😄", "😂", "🥰", "😮", "😭", "😅", "😡", "👍", "🙏", "🎉", "🔥", "❤️", "🤔", "💯", "🍺"];
  // 下拉面板通用样式：固定宽度 + 网格（防在窄单元格里竖排成一列）
  const bbPanel =
    "absolute z-10 mt-1 grid w-56 gap-1 rounded-[var(--r-sm)] border border-[var(--baozi-line)] bg-[var(--baozi-paper)] p-2 shadow-[var(--shadow-card)]";
  const bbSwatch =
    "flex h-7 w-full cursor-pointer items-center justify-center gap-1 rounded-[var(--r-sm)] border border-[var(--baozi-line)] text-[11px] text-ink hover:border-[var(--baozi-orange)]";

  return (
    <FormRow label={dict.upload.descr}>
      <div className="flex flex-col gap-1">
        <div className="flex flex-wrap items-center gap-1">
          <details className="relative">
            <summary className={`${bbBtn} inline-flex cursor-pointer list-none items-center justify-center gap-1`} title={dict.upload.bbColor ?? "颜色"}>
              🎨 {dict.upload.bbColor ?? "颜色"}
            </summary>
            <div className={`${bbPanel} grid-cols-8 w-72`}>
              {BB_COLORS.map(([hex, label]) => (
                <button
                  key={hex}
                  type="button"
                  title={label}
                  className={bbSwatch}
                  style={hex === "#ffffff" ? { background: "#fff" } : { background: hex, color: "#fff" }}
                  onClick={() => {
                    bbWrap(`[color=${hex}]`, "[/color]");
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
            title={dict.upload.bbFont ?? "字体"}
          >
            <option value="">{dict.upload.bbFont ?? "字体"}</option>
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
            title={dict.upload.bbSize ?? "字号"}
          >
            <option value="">{dict.upload.bbSize ?? "字号"}</option>
            {[1, 2, 3, 4, 5, 6, 7, 8].map((n) => (
              <option key={n} value={n}>
                {n} 号
              </option>
            ))}
          </select>
          <button type="button" className={`${bbBtn} font-black`} onClick={() => bbWrap("[b]", "[/b]")} title="Bold">B</button>
          <button type="button" className={`${bbBtn} italic`} onClick={() => bbWrap("[i]", "[/i]")} title="Italic">I</button>
          <button type="button" className={`${bbBtn} underline`} onClick={() => bbWrap("[u]", "[/u]")} title="Underline">U</button>
          <button type="button" className={`${bbBtn} line-through`} onClick={() => bbWrap("[s]", "[/s]")} title="Strikethrough">S</button>
          <button type="button" className={bbBtn} onClick={() => bbWrap("[url]", "[/url]", "https://")} title={dict.upload.bbLink ?? "链接"}>🔗</button>
          <button
            type="button"
            className={bbBtn}
            title={dict.upload.bbImg ?? "图片"}
            onClick={() => {
              const u = window.prompt(dict.upload.bbImg ?? "图片 URL");
              if (u && u.trim()) bbInsert(`[img]${u.trim()}[/img]`);
            }}
          >🖼️</button>
          <button type="button" className={bbBtn} onClick={() => bbWrap("[quote]", "[/quote]")} title={dict.upload.bbQuote ?? "引用"}>❝</button>
          <button
            type="button"
            className={bbBtn}
            title={dict.upload.bbCode ?? "代码 / MediaInfo"}
            onClick={() => bbWrap("[code]", "[/code]", "MediaInfo / General / Complete name …")}
          >{"</>"}</button>
          <details className="relative">
            <summary className={`${bbBtn} inline-flex cursor-pointer list-none items-center justify-center`} title={dict.upload.bbEmoji ?? "表情"}>😀</summary>
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
          {dict.upload.descrHint}（BBCode：<code>[b][i][color=][size=][url][img][quote][code]</code> 均受支持）
        </span>
      </div>
    </FormRow>
  );
}
