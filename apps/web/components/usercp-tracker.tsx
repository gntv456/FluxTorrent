"use client";

import { useI18n } from "@/i18n/client";
import { Row } from "@/components/usercp-row";
import { BrowseSettingsRow } from "@/components/usercp-tracker-browse";
import type { UserSettings } from "@/components/usercp";

/** 网站设定面板（从 usercp.tsx 按域拆出，300 行门禁）：
 *  默认分类 / 样式与语言 / 详情页开关等；
 *  种子列表浏览参数拆至 usercp-tracker-browse.tsx。 */

const TRACKER_CATS = [
  { id: "401", name: "电影" },
  { id: "402", name: "剧集" },
  { id: "403", name: "综艺" },
  { id: "404", name: "纪录片" },
  { id: "405", name: "动漫" },
  { id: "406", name: "音乐视频" },
  { id: "407", name: "体育运动" },
  { id: "408", name: "高品质音频" },
  { id: "410", name: "短剧" },
  { id: "409", name: "其他" },
];

export function TrackerTab({
  s,
  patch,
}: {
  s: UserSettings;
  patch: (p: Partial<UserSettings>) => void;
}) {
  const { dict, currency } = useI18n();
  const t = dict.usercp.tracker;
  const cats = (s.browsecat ?? "").split(",").filter(Boolean);
  const toggleCat = (id: string, on: boolean) => {
    const next = on
      ? [...new Set([...cats, id])]
      : cats.filter((c) => c !== id);
    patch({ browsecat: next.join(",") });
  };
  return (
    <div className="baozi-wide-table-scroll">
    <table className="nexus-table nexus-form">
      <tbody>
        <Row head={t.defaultCat}>
          <div className="uc-cat-groups">
            <section className="uc-cat-card">
              <table>
                <caption>
                  <b>{t.catSection}</b>
                </caption>
                <tbody>
                  <tr>
                    <td className="bottom" colSpan={5}>
                      {TRACKER_CATS.map((c) => (
                        <label key={c.id} className="uc-cat-item">
                          <input
                            type="checkbox"
                            checked={cats.includes(c.id)}
                            onChange={(e) => toggleCat(c.id, e.target.checked)}
                          />
                          {c.name}
                        </label>
                      ))}
                    </td>
                  </tr>
                </tbody>
              </table>
            </section>
            <section className="uc-cat-card">
              <table>
                <caption>
                  <b>{t.extraSection}</b>
                </caption>
                <tbody>
                  <tr>
                    <td className="bottom">
                      <b>{t.showDead}</b>
                      <br />
                      <select
                        value={String(s.incl_dead)}
                        onChange={(e) =>
                          patch({ incl_dead: Number(e.target.value) })
                        }
                      >
                        <option value="0">{t.deadBoth}</option>
                        <option value="1">{t.deadAlive}</option>
                        <option value="2">{t.deadDead}</option>
                      </select>
                    </td>
                    <td className="bottom">
                      <b>{t.showPromo}</b>
                      <br />
                      <select
                        value={String(s.sp_state)}
                        onChange={(e) =>
                          patch({ sp_state: Number(e.target.value) })
                        }
                      >
                        <option value="0">{t.promoAll}</option>
                        <option value="1">{t.promoNormal}</option>
                        <option value="2">{dict.promotion.free}</option>
                        <option value="3">2X</option>
                        <option value="4">2X{dict.promotion.free}</option>
                        <option value="5">{dict.promotion.half}</option>
                        <option value="6">2X {dict.promotion.half}</option>
                        <option value="7">{dict.promotion.p30}</option>
                      </select>
                    </td>
                    <td className="bottom">
                      <b>{t.showBookmark}</b>
                      <br />
                      <select
                        value={String(s.incl_bookmarked)}
                        onChange={(e) =>
                          patch({ incl_bookmarked: Number(e.target.value) })
                        }
                      >
                        <option value="0">{t.bmAll}</option>
                        <option value="1">{t.bmOnly}</option>
                        <option value="2">{t.bmNone}</option>
                      </select>
                    </td>
                  </tr>
                </tbody>
              </table>
            </section>
          </div>
        </Row>
        <Row head={t.stylesheet}>
          <select
            value={s.stylesheet}
            onChange={(e) => patch({ stylesheet: e.target.value })}
          >
            <option value="FluxTorrent">FluxTorrent</option>
          </select>{" "}
          <span className="uc-note">
            {t.stylesheetMore}
            <a>{dict.usercp.clickHere}</a>
          </span>
        </Row>
        <Row head={t.fontsize}>
          <select
            value={s.fontsize}
            onChange={(e) => patch({ fontsize: e.target.value })}
          >
            <option value="small">{t.small}</option>
            <option value="medium">{t.medium}</option>
            <option value="large">{t.large}</option>
          </select>
        </Row>
        <Row head={t.language}>
          <select
            value={s.site_language}
            onChange={(e) => patch({ site_language: e.target.value })}
          >
            <option value="en">English</option>
            <option value="chs">简体中文</option>
            <option value="cht">繁體中文</option>
          </select>{" "}
          <span className="uc-note">
            {t.languageMore}
            <a>{dict.usercp.helpTranslate}</a>
          </span>
        </Row>
        <Row head={t.pmPerPage}>
          {t.pmPerPagePrefix}
          <input
            type="text"
            size={5}
            className="uc-num-input"
            value={s.pm_per_page}
            onChange={(e) =>
              patch({ pm_per_page: Number(e.target.value) || 0 })
            }
          />
          {t.pmPerPageSuffix}
        </Row>
        <Row head={t.detailPage}>
          <label>
            <input
              type="checkbox"
              checked={s.show_description}
              onChange={(e) => patch({ show_description: e.target.checked })}
            />
            {t.showDescr}
          </label>
          <br />
          <label>
            <input
              type="checkbox"
              checked={s.show_imdb}
              onChange={(e) => patch({ show_imdb: e.target.checked })}
            />
            {t.showImdb}
          </label>
        </Row>
        <Row head={t.discussion}>
          <label>
            <input
              type="checkbox"
              checked={s.show_comment}
              onChange={(e) => patch({ show_comment: e.target.checked })}
            />
            {t.showComments}
          </label>
        </Row>
        <Row head={t.showAd}>
          <label>
            <input type="checkbox" checked disabled />
            {t.wantAds}
          </label>
          <br />
          <b className="uc-uploader">发布员</b>
          {t.adNote1}
          <br />
          <b className="uc-poweruser">Power User</b>
          {t.adNote2}
          <a>{dict.usercp.clickHere}</a>
        </Row>
        <Row head={t.timeType}>
          <label>
            <input
              type="radio"
              name="time_type"
              checked={s.time_type === "timeadded"}
              onChange={() => patch({ time_type: "timeadded" })}
            />
            {t.timeAdded}
          </label>
          <label>
            <input
              type="radio"
              name="time_type"
              checked={s.time_type === "timealive"}
              onChange={() => patch({ time_type: "timealive" })}
            />
            {t.timeAlive}
          </label>
        </Row>
        <BrowseSettingsRow s={s} patch={patch} />
      </tbody>
    </table>
    </div>
  );
}
