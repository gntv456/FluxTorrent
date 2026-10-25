/** 账户健康页文案（zh-CN / zh-TW / en / ja 四语自包含）。
 *
 *  为什么不并进 zh-CN.ts 那四本主字典：那几本正被站长在途修改（本轮只读）。
 *  本页只服务 /account-health 与两处入口，单独成文件即可；
 *  主字典合并时整段搬进 `accountHealth` 键即可，消费方不用改。
 */
import type { Locale } from "./config";

export interface AppealCopy {
  label: string;
  placeholder: string;
  submit: string;
  busy: string;
  ok: string;
  tooShort: string;
}

const zhCN = {
  title: "账户健康",
  sub: "风险原因、可核验的新手进度与站点公开策略",
  backToCenter: "返回控制面板",
  loginRequired: "请先登录后查看账户健康",
  loginHint: "登录后可查看本人风险分数、证据原因、申诉入口与新手进度。",
  loginCta: "去登录",
  riskTitle: "风险与证据",
  riskSub: "仅展示本人数据；判定权重与阈值不公开",
  scoreLabel: "风险分",
  band: {
    none: "正常",
    low: "低",
    watch: "观察",
    high: "高",
    critical: "严重",
  } as Record<string, string>,
  ruleset: "规则集 v{ruleset}",
  evaluated: "最近评估",
  decay: "衰减半衰期 {days} 天",
  reasonsTitle: "当前信号",
  reasonsEmpty: "没有未结的风险信号",
  reasonsEmptyDesc:
    "保持做种与下载行为正常即可；分数会随时间与新证据更新。",
  reasons: {
    ghost_seed: "幽灵做种（无实际数据上传）",
    speed_over_ceiling: "速度超过物理上限",
    counter_reset: "流量计数器重置",
    uncorroborated_upload: "上传量无对应下载佐证",
    xreport_over: "举报数据与日志不符",
    near_cap_rhythm: "接近限额的规律性流量",
    self_deal: "自我交易",
    torrent_audit: "种子内容审计异常",
    xreport_absurd: "异常举报",
    download_under_report: "下载量少报",
    bit_thief: "客户端作弊特征",
    login_ip_spread: "登录 IP 分散异常",
    client_blocked: "客户端被站点规则拦截",
    other: "其他风控信号",
  } as Record<string, string>,
  hits: "{n} 次",
  torrents: "相关种子",
  lastSeen: "最近",
  caseTitle: "进行中的案件",
  caseId: "案件 #{id}",
  caseStatus: { "0": "待处理", "2": "处理中" } as Record<string, string>,
  openedAt: "立案时间",
  noCase: "当前没有进行中的作弊案件。",
  latestTitle: "最近一次申诉",
  appealStatus: {
    open: "处理中",
    accepted: "已受理",
    rejected: "已驳回",
  } as Record<string, string>,
  outcome: {
    upheld: "维持原判",
    mitigated: "减轻处理",
    overturned: "撤销处理",
  } as Record<string, string>,
  appeal: {
    label: "申诉说明",
    placeholder:
      "请描述具体情况（例如：客户端版本、做种时间、可核对的日志）",
    submit: "提交申诉",
    busy: "提交中…",
    ok: "申诉已提交，处理结果会显示在下方。",
    tooShort: "申诉内容至少 10 个字符。",
  } as AppealCopy,
  appealPending: "该案件已有未结申诉，请等待处理结果。",
  appealLimit: "未结申诉已达上限（{open}/{limit}），请等待处理。",
  onboardTitle: "新手清单",
  onboardSub: "每一步都由站内真实记录判定，不需要手动勾选",
  onboardProgress: "已完成 {done}/{total}",
  onboardAllDone: "全部完成",
  stepDone: "已完成",
  stepTodo: "去完成",
  steps: {
    email: {
      label: "验证邮箱",
      desc: "在个人资料页完成邮箱验证",
    },
    profile: {
      label: "完善个人资料",
      desc: "添加头像或个人简介（任一即可）",
    },
    first_download: {
      label: "完成首次下载",
      desc: "从种子列表下载并完成任意一个种子",
    },
    first_seed: {
      label: "开始做种",
      desc: "让任意种子产生做种时长或上传记录",
    },
    passkey: {
      label: "注册通行密钥",
      desc: "用系统生物识别或安全密钥登录",
    },
    twofa: {
      label: "开启两步验证",
      desc: "在安全设置中绑定 TOTP 应用",
    },
    rss: {
      label: "创建 RSS 令牌",
      desc: "在获取 RSS 页面生成个人令牌",
    },
  } as Record<string, { label: string; desc: string }>,
  policyTitle: "站点公开策略",
  policySub:
    "生效值的规范化快照；H&R 时长/期限为站点默认，单种政策可覆盖",
  policyVersion: "策略版本 {version}",
  policyUpdated: "更新于 {date}",
  policyHr: "H&R 站点默认：做种 {hours} 小时 / {days} 天",
  policyPrewarn: "到期前 {hours} 小时预警",
  policyLimit: "未结违规达 {n} 次时暂停下载",
  policyWarn: "警告期 {base} 天；反复违规最多顺延 {max} 天",
  policyWarnNoCap: "警告期 {base} 天；反复违规顺延不封顶",
  policyWarnOff: "警告信已关闭",
  policyDecay: "风险分衰减半衰期 {days} 天",
  policyHrLink: "查看我的 H&R 记录",
};

export type AccountHealthCopy = typeof zhCN;

const zhTW: AccountHealthCopy = {
  title: "帳戶健康",
  sub: "風險原因、可核驗的新手進度與站點公開策略",
  backToCenter: "返回控制面板",
  loginRequired: "請先登入後查看帳戶健康",
  loginHint: "登入後可查看本人風險分數、證據原因、申訴入口與新手進度。",
  loginCta: "去登入",
  riskTitle: "風險與證據",
  riskSub: "僅顯示本人資料；判定權重與閾值不公開",
  scoreLabel: "風險分",
  band: {
    none: "正常",
    low: "低",
    watch: "觀察",
    high: "高",
    critical: "嚴重",
  },
  ruleset: "規則集 v{ruleset}",
  evaluated: "最近評估",
  decay: "衰減半衰期 {days} 天",
  reasonsTitle: "目前訊號",
  reasonsEmpty: "沒有未結的風險訊號",
  reasonsEmptyDesc:
    "保持做種與下載行為正常即可；分數會隨時間與新證據更新。",
  reasons: {
    ghost_seed: "幽靈做種（無實際上傳資料）",
    speed_over_ceiling: "速度超過物理上限",
    counter_reset: "流量計數器重設",
    uncorroborated_upload: "上傳量無對應下載佐證",
    xreport_over: "檢舉數據與日誌不符",
    near_cap_rhythm: "接近限額的規律性流量",
    self_deal: "自我交易",
    torrent_audit: "種子內容稽核異常",
    xreport_absurd: "異常檢舉",
    download_under_report: "下載量少報",
    bit_thief: "客戶端作弊特徵",
    login_ip_spread: "登入 IP 分散異常",
    client_blocked: "客戶端被站點規則攔截",
    other: "其他風控訊號",
  },
  hits: "{n} 次",
  torrents: "相關種子",
  lastSeen: "最近",
  caseTitle: "進行中的案件",
  caseId: "案件 #{id}",
  caseStatus: { "0": "待處理", "2": "處理中" },
  openedAt: "立案時間",
  noCase: "目前沒有進行中的作弊案件。",
  latestTitle: "最近一次申訴",
  appealStatus: {
    open: "處理中",
    accepted: "已受理",
    rejected: "已駁回",
  },
  outcome: {
    upheld: "維持原判",
    mitigated: "減輕處理",
    overturned: "撤銷處理",
  },
  appeal: {
    label: "申訴說明",
    placeholder:
      "請描述具體情況（例如：客戶端版本、做種時間、可核對的日誌）",
    submit: "提交申訴",
    busy: "提交中…",
    ok: "申訴已提交，處理結果會顯示在下方。",
    tooShort: "申訴內容至少 10 個字元。",
  },
  appealPending: "該案件已有未結申訴，請等待處理結果。",
  appealLimit: "未結申訴已達上限（{open}/{limit}），請等待處理。",
  onboardTitle: "新手清單",
  onboardSub: "每一步都由站內真實記錄判定，不需要手動勾選",
  onboardProgress: "已完成 {done}/{total}",
  onboardAllDone: "全部完成",
  stepDone: "已完成",
  stepTodo: "去完成",
  steps: {
    email: {
      label: "驗證信箱",
      desc: "在個人資料頁完成信箱驗證",
    },
    profile: {
      label: "完善個人資料",
      desc: "新增頭像或個人簡介（任一即可）",
    },
    first_download: {
      label: "完成首次下載",
      desc: "從種子列表下載並完成任一種子",
    },
    first_seed: {
      label: "開始做種",
      desc: "讓任一種子產生做種時長或上傳記錄",
    },
    passkey: {
      label: "註冊通行密鑰",
      desc: "用系統生物辨識或安全金鑰登入",
    },
    twofa: {
      label: "開啟兩步驗證",
      desc: "在安全設定中綁定 TOTP 應用",
    },
    rss: {
      label: "建立 RSS 權杖",
      desc: "在取得 RSS 頁面產生個人權杖",
    },
  },
  policyTitle: "站點公開策略",
  policySub:
    "生效值的規範化快照；H&R 時長/期限為站點預設，單種政策可覆寫",
  policyVersion: "策略版本 {version}",
  policyUpdated: "更新於 {date}",
  policyHr: "H&R 站點預設：做種 {hours} 小時 / {days} 天",
  policyPrewarn: "到期前 {hours} 小時預警",
  policyLimit: "未結違規達 {n} 次時暫停下載",
  policyWarn: "警告期 {base} 天；反覆違規最多順延 {max} 天",
  policyWarnNoCap: "警告期 {base} 天；反覆違規順延不封頂",
  policyWarnOff: "警告信已關閉",
  policyDecay: "風險分衰減半衰期 {days} 天",
  policyHrLink: "查看我的 H&R 記錄",
};

const en: AccountHealthCopy = {
  title: "Account health",
  sub: "Risk reasons, verifiable onboarding progress and published policies",
  backToCenter: "Back to control panel",
  loginRequired: "Sign in to view your account health",
  loginHint:
    "Sign in to see your risk score, evidence, appeal entry and onboarding progress.",
  loginCta: "Sign in",
  riskTitle: "Risk & evidence",
  riskSub: "Only your own data is shown; weights and thresholds stay private",
  scoreLabel: "Risk score",
  band: {
    none: "Normal",
    low: "Low",
    watch: "Watch",
    high: "High",
    critical: "Critical",
  },
  ruleset: "Ruleset v{ruleset}",
  evaluated: "Last evaluated",
  decay: "Decay half-life {days} days",
  reasonsTitle: "Active signals",
  reasonsEmpty: "No open risk signals",
  reasonsEmptyDesc:
    "Keep seeding and downloading normally; the score updates over time and with new evidence.",
  reasons: {
    ghost_seed: "Ghost seeding (no data actually uploaded)",
    speed_over_ceiling: "Speed above the physical ceiling",
    counter_reset: "Traffic counter reset",
    uncorroborated_upload: "Upload without corroborating download",
    xreport_over: "Report inconsistent with logs",
    near_cap_rhythm: "Near-cap rhythmic traffic",
    self_deal: "Self-dealing",
    torrent_audit: "Torrent content audit anomaly",
    xreport_absurd: "Absurd report",
    download_under_report: "Download under-reporting",
    bit_thief: "Bit-thief client behaviour",
    login_ip_spread: "Unusual login IP spread",
    client_blocked: "Client blocked by site rules",
    other: "Other risk signal",
  },
  hits: "{n} hits",
  torrents: "Related torrents",
  lastSeen: "Last",
  caseTitle: "Open case",
  caseId: "Case #{id}",
  caseStatus: { "0": "Pending", "2": "Under review" },
  openedAt: "Opened",
  noCase: "There is no open cheat case right now.",
  latestTitle: "Latest appeal",
  appealStatus: {
    open: "Under review",
    accepted: "Accepted",
    rejected: "Rejected",
  },
  outcome: {
    upheld: "Upheld",
    mitigated: "Mitigated",
    overturned: "Overturned",
  },
  appeal: {
    label: "Appeal statement",
    placeholder:
      "Describe the situation (client version, seeding time, logs you can check)",
    submit: "Submit appeal",
    busy: "Submitting…",
    ok: "Your appeal has been submitted. The result will appear below.",
    tooShort: "An appeal must be at least 10 characters.",
  },
  appealPending:
    "This case already has an open appeal. Please wait for the result.",
  appealLimit:
    "You have reached the open-appeal limit ({open}/{limit}). Please wait.",
  onboardTitle: "Onboarding checklist",
  onboardSub:
    "Every step is verified against real site records; nothing is self-reported",
  onboardProgress: "{done}/{total} complete",
  onboardAllDone: "All done",
  stepDone: "Done",
  stepTodo: "To do",
  steps: {
    email: {
      label: "Verify email",
      desc: "Complete email verification on your profile page",
    },
    profile: {
      label: "Complete your profile",
      desc: "Add an avatar or a bio (either one)",
    },
    first_download: {
      label: "Download your first torrent",
      desc: "Download and complete any torrent from the list",
    },
    first_seed: {
      label: "Start seeding",
      desc: "Generate seeding time or upload on any torrent",
    },
    passkey: {
      label: "Register a passkey",
      desc: "Sign in with biometrics or a security key",
    },
    twofa: {
      label: "Enable two-factor authentication",
      desc: "Bind a TOTP app in security settings",
    },
    rss: {
      label: "Create an RSS token",
      desc: "Generate a personal token on the Get RSS page",
    },
  },
  policyTitle: "Published site policies",
  policySub:
    "A normalized snapshot of effective values; H&R hours/days are site defaults and may be overridden per torrent",
  policyVersion: "Policy version {version}",
  policyUpdated: "Updated {date}",
  policyHr: "H&R site default: seed {hours} hours / {days} days",
  policyPrewarn: "Pre-warning {hours} hours before the deadline",
  policyLimit: "Download suspended at {n} unresolved H&R violations",
  policyWarn:
    "Warning period {base} days; repeat violations shift it by up to " +
    "{max} days",
  policyWarnNoCap:
    "Warning period {base} days; repeat violations shift it without cap",
  policyWarnOff: "Warning notices disabled",
  policyDecay: "Risk decay half-life {days} days",
  policyHrLink: "View my H&R records",
};

const ja: AccountHealthCopy = {
  title: "アカウントの健全性",
  sub: "リスク理由、検証可能なオンボーディング進捗、公開ポリシー",
  backToCenter: "コントロールパネルに戻る",
  loginRequired: "アカウントの健全性を見るにはログインしてください",
  loginHint:
    "ログインすると、リスクスコア、根拠、異議申し立て、オンボーディング進捗を確認できます。",
  loginCta: "ログイン",
  riskTitle: "リスクと根拠",
  riskSub:
    "ご本人のデータのみ表示します。判定の重みとしきい値は公開しません",
  scoreLabel: "リスクスコア",
  band: {
    none: "正常",
    low: "低",
    watch: "要注意",
    high: "高",
    critical: "重大",
  },
  ruleset: "ルールセット v{ruleset}",
  evaluated: "最終評価",
  decay: "減衰半減期 {days} 日",
  reasonsTitle: "現在のシグナル",
  reasonsEmpty: "未解決のリスクシグナルはありません",
  reasonsEmptyDesc:
    "シードとダウンロードを正常に続けてください。スコアは時間と新しい根拠に応じて更新されます。",
  reasons: {
    ghost_seed: "ゴーストシード（実際のアップロードなし）",
    speed_over_ceiling: "物理的上限を超える速度",
    counter_reset: "通信カウンタのリセット",
    uncorroborated_upload: "ダウンロード裏付けのないアップロード",
    xreport_over: "通報とログの不一致",
    near_cap_rhythm: "上限付近の規則的な通信",
    self_deal: "自己取引",
    torrent_audit: "トレント内容監査の異常",
    xreport_absurd: "異常な通報",
    download_under_report: "ダウンロード量の過少申告",
    bit_thief: "クライアント不正の兆候",
    login_ip_spread: "ログイン IP の異常分散",
    client_blocked: "クライアントがサイト規則でブロック",
    other: "その他のシグナル",
  },
  hits: "{n} 回",
  torrents: "関連トレント",
  lastSeen: "最新",
  caseTitle: "進行中の案件",
  caseId: "案件 #{id}",
  caseStatus: { "0": "対応待ち", "2": "対応中" },
  openedAt: "起案日時",
  noCase: "現在、進行中の不正案件はありません。",
  latestTitle: "最新の申し立て",
  appealStatus: {
    open: "対応中",
    accepted: "受理済み",
    rejected: "却下",
  },
  outcome: {
    upheld: "原判定維持",
    mitigated: "処分軽減",
    overturned: "処分取消",
  },
  appeal: {
    label: "申し立ての内容",
    placeholder:
      "状況を記載してください（クライアントのバージョン、シード時間、確認できるログなど）",
    submit: "申し立てを送信",
    busy: "送信中…",
    ok: "申し立てを送信しました。結果は下に表示されます。",
    tooShort: "申し立ては 10 文字以上で入力してください。",
  },
  appealPending:
    "この案件には未解決の申し立てがあります。結果をお待ちください。",
  appealLimit:
    "未解決の申し立てが上限に達しました（{open}/{limit}）。お待ちください。",
  onboardTitle: "オンボーディングチェックリスト",
  onboardSub:
    "各ステップはサイトの実際の記録で判定されます。手動チェックはありません",
  onboardProgress: "{done}/{total} 完了",
  onboardAllDone: "すべて完了",
  stepDone: "完了",
  stepTodo: "未完了",
  steps: {
    email: {
      label: "メール認証",
      desc: "プロフィールページでメール認証を完了",
    },
    profile: {
      label: "プロフィールを完成",
      desc: "アバターまたは自己紹介を追加（どちらか一方）",
    },
    first_download: {
      label: "初回ダウンロード",
      desc: "一覧からトレントをダウンロードして完了する",
    },
    first_seed: {
      label: "シードを開始",
      desc: "いずれかのトレントでシード時間またはアップロードを記録",
    },
    passkey: {
      label: "パスキーを登録",
      desc: "生体認証またはセキュリティキーでログイン",
    },
    twofa: {
      label: "二段階認証を有効化",
      desc: "セキュリティ設定で TOTP アプリを登録",
    },
    rss: {
      label: "RSS トークンを作成",
      desc: "RSS 取得ページで個人トークンを生成",
    },
  },
  policyTitle: "公開サイトポリシー",
  policySub:
    "有効値の正規化スナップショット。H&R の時間・日数はサイト既定で、トレント別に上書き可能",
  policyVersion: "ポリシーバージョン {version}",
  policyUpdated: "{date} 更新",
  policyHr: "H&R サイト既定：シード {hours} 時間 / {days} 日",
  policyPrewarn: "期限の {hours} 時間前に事前警告",
  policyLimit: "未解決の H&R 違反が {n} 件でダウンロード停止",
  policyWarn: "警告期間 {base} 日。繰り返し違反で最大 {max} 日延長",
  policyWarnNoCap:
    "警告期間 {base} 日。繰り返し違反で延長上限なし",
  policyWarnOff: "警告通知は無効です",
  policyDecay: "リスク減衰の半減期 {days} 日",
  policyHrLink: "自分の H&R 記録を見る",
};

export const accountHealthCopy: Record<Locale, AccountHealthCopy> = {
  "zh-CN": zhCN,
  "zh-TW": zhTW,
  en,
  ja,
};
