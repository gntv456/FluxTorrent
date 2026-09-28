# 翻译贡献指南（i18n）

> 想让 FluxTorrent 说出你的语言？本文讲清楚字典结构、两种贡献方式（完整语言 /
> 段级回落）与验收门禁。顺手也解释「术语表」与字典的分工。

## 字典架构（30 秒版）

- `apps/web/i18n/zh-CN.ts` 是**类型源**（`Dict`）——所有语言的键结构以它为准；
- `zh-TW.ts` / `en.ts` 是**完整语言**：`Omit<Dict, …>` + spread 补段（Omit 联合里
  列出「在 export 对象里单独补定义」的段）；
- `ja.ts` 是**段级回落语言**（E10 起）：`DeepPartial<Dict>`，翻到哪段哪段生效，
  其余自动回落 zh-CN（`i18n/merge.ts` 深合并）；
- 后端错误消息三语（`apps/api/src/i18n.rs`），ja 请求暂映射英文；
- 运行时改写：站长「术语表」（site_terms）在 `getDict` 出口做字符串替换——
  你翻译的字典与站长的术语规则叠加生效，互不冲突。

## 方式一：补全段级回落语言（推荐起步）

以 `ja.ts` 为例：挑一个还没翻译的段（如 `forum` / `shop`），从 `zh-CN.ts`
复制整段，翻译值，粘进 `ja.ts` 同名段。**只贴你翻译的键也行**——缺的键自动
回落中文。提交前跑：

```bash
cd apps/web && npx tsc --noEmit        # 键名拼错会红
node ../../scripts/i18n_guard.mjs      # 新增硬编码中文检测（翻译值不算）
```

## 方式二：新增完整语言

1. `i18n/config.ts`：`LOCALES` 加码 + `dateLocale` 加映射 + `siteLangToLocale`
   加换算（NP 口径码如 `jp`）；
2. 新建 `i18n/<locale>.ts`：从 `zh-TW.ts` 起步（结构最接近完整范式），逐段翻译；
3. `i18n/server.ts`：`DICTS` 注册；
4. `components/locale-switcher.tsx` 与 `app/login/login-view.tsx`：语言标签；
5. 门禁：tsc + i18n_guard + 手工点一遍主要页面。

## 纪律（评审会看）

1. **占位符不动**：`{n}` / `{magic}` / `{x}` 是插值变量，翻译时保留原样；
2. **键名不改**：键是契约；要改键先在 zh-CN 改类型，四语同步；
3. **不引入新段**：段落名即功能域，新增段需要先在 zh-CN 定义并给全量语言补位；
4. **行宽 ≤80**（line_limit_guard 会拦），长句用续行；
5. 翻译腔 warning：种子站的行话（做种/辅种/保种/官种）在 PT 社区有惯用译法，
   不确定就在 PR 里标注，评审会帮忙定。

## 已知限制

- 段级回落语言的「未翻译段」对不懂中文的用户仍是中文——翻译覆盖率按
  高曝光段优先（nav/torrents/login/shop 已在 ja 首批）；
- RTL（阿拉伯语/希伯来语）：当前布局未做镜像，接受度评估见
  `_doc/开源生态…策划案` E10 备注，暂不接收 RTL 语言。
