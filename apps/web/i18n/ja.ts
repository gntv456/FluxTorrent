/** 日本語辞書（E10：段級回落式言語）
 *
 *  完全翻訳ではなく「高露出セグメントのみ翻訳 + 残りは簡体字中国語へフォールバック」。
 *  方式: DeepPartial<Dict> 型で部分定義し、getDict 側で zh-CN に深くマージする
 *  （i18n/merge.ts 参照）。翻訳カバレッジ拡大はコミュニティ PR 歓迎——
 *  セグメントを丸ごとコピーして訳せば、そのセグメントから完全版になる。
 */

import type { Dict } from "./zh-CN";

export const ja: DeepPartial<Dict> = {
  nav: {
    home: "ホーム",
    library: "ライブラリ",
    official: "公式",
    forums: "フォーラム",
    messages: "メッセージ",
    textbooks: "教科書",
    medals: "勲章",
    top: "ランキング",
    upload: "アップロード",
    subtitles: "字幕",
    friends: "フレンド",
    requests: "リクエスト",
    offers: "候補",
    jixiao: "実績",
    preserve: "保種エリア",
    endangered: "絶滅危惧",
    teams: "保種チーム",
    classes: "等級",
    resurrections: "復活タスク",
    more: "その他",
    discover: "発見",
    spark: "{magic}エコノミー",
    growth: "成長と栄誉",
    fun: "エンタメ",
    tabbarMessages: "メッセージ",
    gachaDisclosure: "ガチャ公開",
  },
  common: {
    brand: "FluxTorrent",
    globalSearchPh: "Torrent / 投稿を検索…",
    gsTorrents: "Torrent",
    gsTopics: "フォーラム投稿",
    gsEmpty: "該当なし",
    gsMore: "すべての結果を見る →",
    publish: "アップロード",
    login: "ログイン",
    nextPage: "次へ",
    prevPage: "前へ",
    totalItems: "合計 {n} 件",
    pageX: "ページ {x}",
    pleaseLogin: "ログインしてください",
    loading: "読み込み中…",
    pageLoading: "ページを開いています…",
    retry: "再試行",
    loadFailed: "読み込みに失敗しました",
    save: "保存",
    cancel: "キャンセル",
    yes: "はい",
    no: "いいえ",
  },
  meta: {
    titleSuffix: "次世代 PT サイト",
    description:
      "FluxTorrent — 自由にカスタマイズできる汎用 PT サイト構築システム。",
  },
  torrents: {
    title: "トレント",
    download: "ダウンロード",
    newTag: "新着",
    alive: "生存",
    colTitle: "タイトル",
    colSize: "サイズ",
    colSeeders: "シーダー",
    colLeechers: "リーチャー",
    colCompleted: "完了",
    colComments: "コメント",
    colActions: "操作",
    colType: "種類",
    officialTag: "公式",
    total: "合計 {n} 件",
    statusPending: "審査中",
    statusRejected: "却下",
  },
  torrents2: {
    ratingLegend: "評価",
    ratingHint: "この評価以上のTorrentのみ表示（Douban/IMDb、例: 7.5）",
  },
  torrent: {
    size: "サイズ",
    numFiles: "ファイル数",
    category: "カテゴリ",
    infoHash: "Info Hash",
    sticky: "ピン留め",
    anonymous: "匿名",
    descrTitle: "説明",
    filesTitle: "ファイル",
  },
  promotion: {
    free: "無料",
    x2: "2倍",
    x2free: "2倍無料",
    half: "半額",
    x2half: "2倍半額",
    p30: "30%",
  },
  login: {
    loginTitle: "ログイン",
  },


  pwa: {
    installHint:
      "このサイトをデスクトップにインストールして、ネイティブアプリのように使えます：",
    installBtn: "インストール",
    iosHint: "iOS：共有 → ホーム画面に追加",
    dismiss: "閉じる",
  },
};

/** 深い部分型（段レベルのフォールバックに必要な最小定義） */
export type DeepPartial<T> = {
  [K in keyof T]?: T[K] extends object ? DeepPartial<T[K]> : T[K];
};
