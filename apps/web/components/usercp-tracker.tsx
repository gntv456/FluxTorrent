"use client";

import { useEffect, useState } from "react";
import { api } from "@/lib/api-client";
import { useI18n } from "@/i18n/client";
import { Row } from "@/components/usercp-row";
import { BrowseSettingsRow } from "@/components/usercp-tracker-browse";
import type { UserSettings } from "@/components/usercp";

/** 网站设定面板（从 usercp.tsx 按域拆出，300 行门禁）：
 *  默认分类 / 样式与语言 / 详情页开关等；
 *  种子列表浏览参数拆至 usercp-tracker-browse.tsx。
 *
 *  分类名单来自 /site-profile（categories 表真数据，站型跟随、站长可改）。
 *  2026-11-14 硬编码清偿：原 TRACKER_CATS 写死旧站 cat401-410 编号，
 *  与现库 categories（id 1-10）id 空间对不上，勾选从未生效过。 */

interface ProfileCat {
  id: number;
  name: string;
}

export function TrackerTab({
  s,
  patch,
}: {
  s: UserSettings;
  patch: (p: Partial<UserSettings>) => void;
}) {
  const { dict, currency } = useI18n();
  const t = dict.usercp.tracker;
  const [profileCats, setProfileCats] = useState<ProfileCat[]>([]);
  useEffect(() => {
    api
      .get<{ categories?: ProfileCat[] }>("/api/v1/site-profile")
      .then((r) => setProfileCats(r.categories ?? []))
      .catch(() => setProfileCats([]));
  }, []);
  // 勾选状态 = browsecat 里**仍存在于现分类集**的 id：旧站的 401-410
  // 存量（id 空间对不上）与站长已删的分类自动失效，不会存进下一次保存
  const validIds = new Set(profileCats.map((c) => String(c.id)));
  const cats = (s.browsecat ?? "")
    .split(",")
    .filter((c) => validIds.has(c));
  const toggleCat = (id: string, on: boolean) => {
    const next = on
      ? [...new Set([...cats, id])]
      : cats.filter((c) => c !== id);
    patch({ browsecat: next.join(",") });
  };
  return (
    <div className="brand-wide-table-scroll">
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
                      {profileCats.map((c) => (
                        <label key={c.id} className="uc-cat-item">
                          <input
                            type="checkbox"
                            checked={cats.includes(String(c.id))}
                            onChange={(e) =>
                              toggleCat(String(c.id), e.target.checked)
                            }
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
          </select>
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
          </select>
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
