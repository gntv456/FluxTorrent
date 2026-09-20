"use client";

import { useI18n } from "@/i18n/client";
import { Row } from "@/components/usercp-row";
import type { UserSettings } from "@/components/usercp";

/** 网站设定·种子列表浏览参数（从 usercp.tsx 的 TrackerTab 拆出，
 *  300 行门禁）：每页条数 / 悬浮提示 / 特殊标题附加 / 操作图标 /
 *  评论悬浮。 */

export function BrowseSettingsRow({
  s,
  patch,
}: {
  s: UserSettings;
  patch: (p: Partial<UserSettings>) => void;
}) {
  const { dict, currency } = useI18n();
  const t = dict.usercp.tracker;
  return (
    <Row head={t.torrentPage}>
      <div className="uc-browse-settings">
        <p className="uc-browse-warning">
          <b>{dict.usercp.warning}</b>
          {t.browseWarn}
        </p>
        <div className="uc-browse-grid">
          <section>
            <h3>{t.perPageTitle}</h3>
            <div>
              {t.perPagePrefix}
              <input
                type="text"
                size={5}
                className="uc-num-input"
                value={s.torrents_per_page}
                onChange={(e) =>
                  patch({ torrents_per_page: Number(e.target.value) || 0 })
                }
              />
              {t.perPageSuffix}
            </div>
          </section>
          <section>
            <h3>{t.tooltipTitle}</h3>
            <label>
              <input
                type="radio"
                name="tooltip"
                checked={s.tooltip === "minorimdb"}
                onChange={() => patch({ tooltip: "minorimdb" })}
              />
              {t.tooltipMinor}
            </label>
            <label>
              <input
                type="radio"
                name="tooltip"
                checked={s.tooltip === "medianimdb"}
                onChange={() => patch({ tooltip: "medianimdb" })}
              />
              {t.tooltipMedian}
            </label>
            <label>
              <input
                type="radio"
                name="tooltip"
                checked={s.tooltip === "off"}
                onChange={() => patch({ tooltip: "off" })}
              />
              {t.tooltipOff}
            </label>
          </section>
          <section>
            <h3>{t.specialTitle}</h3>
            <label>
              <input
                type="checkbox"
                checked={s.append_sticky}
                onChange={(e) => patch({ append_sticky: e.target.checked })}
              />
              {t.specialSticky}
            </label>
            <label>
              <input
                type="checkbox"
                checked={s.append_new}
                onChange={(e) => patch({ append_new: e.target.checked })}
              />
              {t.specialNew}
            </label>
            <div className="uc-option-line">
              <span>{t.specialPromo}：</span>
              <label>
                <input
                  type="radio"
                  name="append_promotion"
                  checked={s.append_promotion === "highlight"}
                  onChange={() => patch({ append_promotion: "highlight" })}
                />
                {t.promoHighlight}
              </label>
              <label>
                <input
                  type="radio"
                  name="append_promotion"
                  checked={s.append_promotion === "word"}
                  onChange={() => patch({ append_promotion: "word" })}
                />
                {t.promoWord}
              </label>
              <label>
                <input
                  type="radio"
                  name="append_promotion"
                  checked={s.append_promotion === "icon"}
                  onChange={() => patch({ append_promotion: "icon" })}
                />
                {t.promoIcon}
              </label>
              <label>
                <input
                  type="radio"
                  name="append_promotion"
                  checked={s.append_promotion === "off"}
                  onChange={() => patch({ append_promotion: "off" })}
                />
                {t.promoOff}
              </label>
            </div>
            <label>
              <input
                type="checkbox"
                checked={s.append_picked}
                onChange={(e) => patch({ append_picked: e.target.checked })}
              />
              {t.specialPicked}
            </label>
          </section>
          <section>
            <h3>{t.titleTitle}</h3>
            <label>
              <input
                type="checkbox"
                checked={s.small_descr}
                onChange={(e) => patch({ small_descr: e.target.checked })}
              />
              {t.showSubtitle}
            </label>
          </section>
          <section>
            <h3>{t.actionIconsTitle}</h3>
            <label>
              <input
                type="checkbox"
                checked={s.dl_icon}
                onChange={(e) => patch({ dl_icon: e.target.checked })}
              />
              {t.dlIcon}
            </label>
            <label>
              <input
                type="checkbox"
                checked={s.bm_icon}
                onChange={(e) => patch({ bm_icon: e.target.checked })}
              />
              {t.bmIcon}
            </label>
          </section>
          <section>
            <h3>{t.commentsTitle}</h3>
            <div className="uc-option-line">
              <label>
                <input
                  type="checkbox"
                  checked={s.show_com_num}
                  onChange={(e) => patch({ show_com_num: e.target.checked })}
                />
                {t.showComNum}
              </label>
              <select
                value={s.show_last_com}
                onChange={(e) => patch({ show_last_com: e.target.value })}
              >
                <option value="yes">{t.and}</option>
                <option value="no">{t.butNot}</option>
              </select>
              <span>{t.hoverLastCom}</span>
            </div>
          </section>
        </div>
      </div>
    </Row>
  );
}
