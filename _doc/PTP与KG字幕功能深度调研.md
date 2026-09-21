# PTP 与 KG 字幕功能深度调研

> 调研日期：2026-09-21　对象：PassThePopcorn（PTP）、Karagarga（KG）
> 难度说明：两家都是闭源 + invite-only，全站登录墙（KG 的 robots.txt 只放行 `/index.php` 和 `/login.php`），
> 拿不到内部页面与源码。**因此本报告严格区分「确凿」与「未证实」，未证实项一律标出，不做脑补。**

---

## 0. 结论速览

| | PTP | KG |
|---|---|---|
| 形态 | **没有字幕文件功能**，只有「内嵌字幕语言」元数据 | **有字幕产出**（上传 + 悬赏），形态是社区协作而非 CRUD |
| 独立字幕区 | ❌ 无（无 `/subtitles` 路由、无 `subtitles.php`） | ❓ 未证实（无法取证，倾向于挂在种子/论坛而非独立列表页） |
| 字幕实体 | 只是种子元数据里的语言标签，页面上表现为**国旗图标** | 有真实字幕文件（Wiki 明说"translates and **uploads** subtitles"） |
| 杀手锏 | 无 | **pots**：用 ratio credit 悬赏求字幕，译者翻译上传后拿走这笔 credit |
| 对 FluxTorrent 的价值 | 低（反例：证明"影视站也可以不做字幕区"） | 高（**pots 是 NP/U3D 都没有的社会化机制**） |

**对我上一轮报告的修正**：上一轮写「Gazelle 系没有独立字幕区」——对 **PTP 成立**（它确实没有字幕区，只有元数据旗帜），
但**不该把 KG 混进"Gazelle 系"**：KG 是自研闭源 Custom 架构，不是 Gazelle 分支，而且它有真实的字幕产出与独特的悬赏机制。

---

## 1. PTP：字幕只是元数据，不是文件

### 1.1 确凿证据

**证据 A：种子详情页只有语言旗帜，没有字幕文件**

GreasyFork 上有给 PTP 用的第三方脚本 *Show favorite subtitles*（`@include passthepopcorn.me/torrents.php?id=`），
它的工作方式是在 `#torrent-table` 里遍历每一行 `tr.group_torrent_header` 的下一行，检测
`img[alt="English"]` / `img[alt="French"]` 这类**国旗图片**（图源 `static/common/flags/usa.gif`），按用户偏好语言高亮。

→ 直接证明：PTP 种子页显示的是「这一种子内嵌了哪些语言的字幕」，形式是**一排国旗图标**，不是可下载的字幕文件列表，也没有下载按钮。
（脚本页 URL：https://greasyfork.org/en/scripts/415888-show-favorite-subtitles ；本环境直接抓取失败，证据来自检索命中的脚本说明与代码摘要）

**证据 B：官方 JSON API 不把字幕当实体**

autobrr 的 PTP API 客户端 `pkg/ptp/ptp.go` 里 Torrent 结构只有 Id / Quality / Source / Container / Codec / GoldenPopcorn 等字段，**没有 subtitle 字段**。
（佐证而非铁证：autobrr 只映射它需要的子集）

**证据 C：唯一查到的官方字幕规则是 remux 规则（2020-06-12 公告）**

> "Including English or original language subtitles from the primary source."

即：remux 发种必须带上片源自带的英文或原语言字幕。这是**对压制内容的要求**，不是字幕上传功能。
（转载：https://clwind.com.cn/read.php?page=e&tid=180906）

### 1.2 逐项回答

| 问题 | 结论 | 置信度 |
|---|---|---|
| 有没有独立字幕区（/subtitles） | 无 | 高（多轮检索无命中，脚本证据反证页面只有旗帜） |
| 能不能上传字幕文件 | 无（因此也没有 note / verified / 审核 / 举报） | 高 |
| 详情页字幕区块长什么样 | 非独立面板，是每行种子下的语言国旗 `img[alt="English"]`，悬停看语言名；无下载按钮 | 高 |
| 发种表单有没有 Subtitles 语言多选 | 推断有（由旗帜输出反推），**未拿到源码/截图** | 中（推断） |
| 字幕请求 | 无专属机制，走通用 Requests + Bonus 悬赏 + 论坛 | 中 |
| 字幕规则（禁机翻/须同步/命名） | 只证实 remux 那条；其余未证实 | — |

### 1.3 为什么 PTP 不做字幕区

PTP 是**英语站**，片源以英语片为主，字幕需求天然低；它的质量控制压在"压制规范"上（remux 必须带原语言字幕），而不是"社区补字幕"。
**这是个有用的反例：影视站不是非做字幕区不可**——做不做取决于站型与语种结构。FluxTorrent 的 `site_type` 参数化思路正好可以承接这种差异（影视站开字幕模块，音乐/教育站关掉）。

---

## 2. KG：把字幕做成社区悬赏（pots）

### 2.1 确凿证据

**证据 A：Wikipedia 原文（最硬的一条）**

> "Beyond sharing media, members have also been known to create subtitles for films in languages not previously made available, such as English subtitles for a number of films by Iranian director Sohrab Shahid-Saless.
> **Subtitles may be requested by contributing ratio credit toward "pots", which is then given to any user who translates and uploads subtitles for the corresponding film.**"

（en.wikipedia.org/wiki/Karagarga，已通过 wikiless 镜像二次核实原文）

这条信息量极大，拆开是四件事：
1. **字幕是社区自产的**（为原本没有字幕的影片做字幕，如给伊朗导演 Sohrab Shahid-Saless 的片子做英文字幕）
2. **有"求字幕"入口**
3. **求字幕的方式是"凑 pots"**——多人往一个 pot 里贡献 ratio credit
4. **结算方式**：谁翻译并上传了对应影片的字幕，谁拿走这笔 credit

→ 这是一个**众筹悬赏 + 认领交付**的闭环，用站点最硬的通货（ratio credit，做种额度）结算，而不是用魔力值之类的软通货。

**证据 B：字幕署名文化（TorrentFreak 2023-07-06）**

TCM 播出 1970 年西班牙影片《El Jardín de las Delicias》时，字幕 credits 里写着：

> "Subtitles: Supersoft and Scalisto **for KG**"

说明 KG 字幕有**译者署名规范**（多人协作可并列署名 + 标注出片站），而且质量高到会被商业渠道直接拿走。
（https://torrentfreak.com/turner-classic-movies-airs-a-film-with-pirated-subtitles-230706）

**证据 C：站点形态**

- 真站域名 **karagarga.in**（karagarga.net 现在是 SEO 垃圾站，别采信）
- 传统 PHP 站：`/index.php`、`/login.php`、`/confirmationemail.php`
- robots.txt 只 allow `/index.php` 和 `/login.php`，其余全 Disallow → 无法用爬虫探测内部路径
- 登录页文案："If you want the love, you have to log in."
- 被 PT 目录站归类为 **Custom**（自研），非 Gazelle/TBDev/XBTIT；多轮 GitHub 检索**没有源码泄露或 fork**

### 2.2 逐项回答

| 问题 | 结论 | 置信度 |
|---|---|---|
| 用什么 codebase | 自研闭源 Custom（AlternativeTo 标 Proprietary + "a unique GUI, a large number of original features"）；无源码泄露 | 高 |
| 有没有独立字幕区 | **未证实**。无 `subtitles.php` 类路径的公开证据；pots 更可能挂在论坛/请求区 | 低（证据空白） |
| 上传字段/格式白名单/大小限制/修订版 | **未证实**（无 UI、无规则页、无源码） | 低 |
| 字幕↔种子关联方式（按种子挂载 / 独立字幕库 / 打包进种子） | **未证实** | 低 |
| 字幕请求机制 | ✅ **pots**（ratio credit 众筹悬赏） | 高（Wiki 原文） |
| 字幕组署名 | ✅ "Subtitles: A and B for KG" | 高（TF 报道） |
| 认领/校对/时间轴协作流程 | **未证实**（无文档描述） | 低 |
| UI 截图 / 列表列与按钮 | **未证实**（闭站 + Wayback 在本环境抓取失败） | 低 |

### 2.3 证据边界（重要）

公开资料只能确凿证明两件事：**KG 有很强的字幕社区产出**，以及**它用 pots 做字幕悬赏**。
UI 级、字段级的细节（字幕区在哪、上传表单长什么样、格式白名单）**拿不到**，任何声称知道这些细节的说法都不可信。
所以借鉴 KG 时，能借鉴的是**机制（pots + 署名）**，不是界面。

---

## 3. 五方字幕能力对比

| 维度 | NexusPHP | UNIT3D | **PTP** | **KG** | FluxTorrent |
|---|---|---|---|---|---|
| 独立字幕区 | ✅ subs.php | ✅ /subtitles | ❌ | ❓ 未证实 | ✅ /subtitles |
| 字幕文件实体 | ✅ | ✅ | ❌（只有语言标签） | ✅（有上传动作） | ✅（attachments） |
| 绑定种子 | ✅（按 torrent 分目录） | ✅ 强绑定 | — | ❓ | ⚠️ 可空 |
| 种子页字幕块 | ❌ | ✅ | ⚠️ 只有国旗图标 | ❓ | ❌ |
| 格式白名单 | ✅ | ✅ | — | ❓ | ❌ |
| 审核 / verified | ❌ | ✅ | — | ❓ | ❌ |
| 举报奖惩 | ✅ +50/−100 魔力 | ❌ | — | ❓ | ⚠️ 有按钮无闭环 |
| **字幕悬赏（众筹）** | ❌ | ❌ | ❌（走通用 Requests） | ✅ **pots（ratio credit）** | ❌（求种有 bounty，求字幕无） |
| **译者/字幕组署名** | ❌ | ❌（只有 anon） | ❌ | ✅ "for KG" 署名 | ❌ |
| 成就激励 | ❌ | ✅ 13 档 | ❌ | ❓（有 Bounty Hunter 等级，关联未证实） | ❌ |

---

## 4. 对 FluxTorrent 的启发

### 4.1 KG 的 pots 值得抄，而且我们基础设施已经现成

KG 的 pots = **多人凑钱 → 悬赏 → 有人交付 → 拿走全部悬赏**。对比 FluxTorrent 现状：

| 已有 | 位置 | 能不能复用到字幕 |
|---|---|---|
| 求种悬赏 `requests.bounty` + `fulfilled_torrent_id` | `0001_init.sql:324-333` | ✅ 可扩成「求字幕」：把 `bounty` 落到字幕而非种子 |
| 论坛悬赏发放（CAS 防并发双采 + 幂等键 `forum-bounty-pay:{topic_id}` + `earn_spark`） | `community_http/bounty.rs:20-95` | ✅ 发放链路可直接抄，把幂等键换成 `subtitle-bounty-pay:{req_id}` |
| 字幕表 + 上传 + 下载 + 举报 | `content_http/subtitles.rs`、`subtitle-board.tsx` | ✅ 交付端已有 |
| 勋章 / 任务体系 | `medals`、`tasks` | ✅ 可挂「字幕译者」成就 |

**最小落地方案**（不新建体系，只加一张表 + 一个接口）：

```sql
CREATE TABLE subtitle_requests (
  id BIGSERIAL PRIMARY KEY,
  user_id BIGINT NOT NULL REFERENCES users(id),
  torrent_id BIGINT REFERENCES torrents(id),   -- 为哪个片子求字幕
  lang TEXT NOT NULL,                          -- 求什么语言
  descr TEXT,
  bounty BIGINT NOT NULL DEFAULT 0,            -- 悬赏池（魔力）
  contributors JSONB NOT NULL DEFAULT '[]',    -- 凑钱人 [{user_id, spark}] ← pots 的众筹部分
  status SMALLINT NOT NULL DEFAULT 0,          -- 0 进行中 / 1 已交付 / 2 已撤销
  fulfilled_subtitle_id BIGINT REFERENCES subtitles(id),
  created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
```

流程：**发起求字幕 → 众人往 bounty 池里加魔力（这就是 pots）→ 译者上传字幕并认领 → 站方/发起人确认 → 整池魔力一次性发给译者**（抄 `bounty.rs` 的 CAS + 幂等键）。

### 4.2 译者署名字段

KG 的 "Subtitles: Supersoft and Scalisto for KG" 说明**署名是最好的激励**（比魔力更持久）。
建议给 `subtitles` 加：

```sql
ALTER TABLE subtitles
  ADD COLUMN IF NOT EXISTS author_name TEXT,     -- 译者/字幕组署名（可多人）
  ADD COLUMN IF NOT EXISTS source TEXT,          -- 字幕来源：原创翻译/校对/转抄
  ADD COLUMN IF NOT EXISTS anon BOOLEAN NOT NULL DEFAULT false;
```

列表与种子页显示署名；下载的字幕文件名带上署名（如 `xxx.chs.[译者名].srt`）。

### 4.3 PTP 给的反向启发

PTP 证明「影视站不做字幕文件区也能活得好」，因为它的片源结构不需要。
→ FluxTorrent 是**通用站**（默认 general，可切 movie/music/education…），字幕模块应该跟着 `site_type` / `module_subtitles` 走：
- 影视/动漫站：字幕区 + 种子页挂载 + pots 悬赏（全开）
- 音乐/教育/软件站：关掉（跟 NP 站一样，教育站的字幕区常年是死板块）

---

## 5. 证据清单

| 结论 | 证据 | 强度 |
|---|---|---|
| KG 有 pots 悬赏机制 | Wikipedia / Karagarga 原文（wikiless 镜像二次核实） | 确凿 |
| KG 有字幕产出与上传 | 同上（"translates and uploads subtitles"） | 确凿 |
| KG 字幕署名文化 | TorrentFreak 2023-07-06（"Subtitles: Supersoft and Scalisto for KG"） | 确凿 |
| KG 自研闭源、无源码泄露 | AlternativeTo（Proprietary / unique GUI）+ 多轮 GitHub 检索无果 | 确凿 |
| KG 真站是 karagarga.in、PHP 站、robots 全封 | 实抓 karagarga.in/index.php 与 /robots.txt | 确凿 |
| PTP 种子页只有语言国旗 | GreasyFork 脚本 415888（检索命中，本环境直连失败） | 较强 |
| PTP API 无字幕字段 | autobrr `pkg/ptp/ptp.go` | 佐证 |
| PTP remux 须带原语言/英文字幕 | clwind 转载 PTP 2020-06-12 公告 | 确凿 |
| PTP 无独立字幕区 | 多轮检索无命中 + 上述反证 | 较强（推定） |
| KG 字幕区 UI / 上传字段 / 格式白名单 | **无公开证据** | 空白，未证实 |
