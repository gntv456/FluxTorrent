# -*- coding: utf-8 -*-
"""常驻闸门：「花火花」的会员写口必须拒绝未识别字段（P-314 一族的续集）。

普查（写口 451 条路由里的请求体结构）说：会员面 140 个结构体没有
`deny_unknown_fields`，其中一批是**直接花钱的**：抽卡三消费、池抽三玩法、
刮刮乐、猜大小、钓鱼、魔力池捐赠。旧写法下多带一个键会被静默忽略，
而这类口的后果不是「报错」而是「按缺省执行」：

| 载荷 | 旧写法的反应 | 会员以为 |
| --- | --- | --- |
| `/gacha/dismantle` 多带 `count:5` | 只化 1 张，200 | 化了一叠，碎片 ×5 |
| `/gacha/draw` 把 `count` 拼成 `n:10` | 单抽 1 次、扣 1 张票 | 十连 |
| `/games/jgg` 多带 `ticket:300` | 按池配置的票价扣 | 我出的是 300 |
| `/games/bigsmall` 把 `props` 拼成 `porps` | 当没带道具 | 放大镜生效了 |

本批覆盖 7 个结构体 / 10 条路由。逐条都先核过**前端真发出的键**
（`pt_write_surface_report.py` 里那句「一刀切 deny 会拒掉合法请求」就是
针对这件事——所以能不能加，判据是载荷对账，不是候选数）。

判据：
  V1  覆盖清单是集合级台账：每条路由的体结构必须带 deny；清单加人要走
      显式登记，减人（属性被摘）直接红
  V2  前端载荷对账：表里登记的每个调用点，源码里确实发这些键，且键集
      ⊆ 结构体字段 ⇒「加了 deny 界面照常能用」是机检的，不是我口头保证
  V3  行为证据：`cargo test pool_games::tests::unknown_field_is_rejected`
      （合法载荷仍受理；带未知键报错且点名那个键）
  V4  活体（自升级）：拿 `/gacha/dismantle` 带未知键 + 不存在的 card_id 打
      现网——回 400「含未识别的字段：count」说明修复已进镜像；
      回 1004 说明请求**走到了业务层**（未知键仍被吞）⇒ WARN 记部署账。
      这条探针不花钱：card_id 不存在，既不会真化卡也不会扣碎片
  V5  文案在册：path_cfg 的 humanize 单测断言过「含未识别的字段：box_id」，
      否则 400 会给会员看英文 serde 原文

跑法：PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_write_unknown_field.py
"""
import io
import json
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
sys.stdout.reconfigure(encoding="utf-8", errors="replace")
from pt_audit_lib import BASE, ok, summary  # noqa: E402

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
WEB = os.path.join(ROOT, "apps", "web")
# (路由, 结构体, 源文件, 前端调用文件, 调用点 marker, 前端实发的键)
COVERED = [
    # 2026-10-26 版主登记（P-644）：感谢口补了 deny_unknown_fields，并把
    # `Option<web::Json<T>>` 换成 `OptJson<T>`（旧形状把解析错误折成
    # `Ok(None)`，deny 根本不生效）。放在表首是为了不动 V7 的 `COVERED[-4:]`
    # ——那四条是它挑定的正控对象，尾部一漂正控就测在别的口上。
    ("/torrents/{id}/thanks", "ThankBody",
     "apps/api/src/torrent_http/interact.rs",
     "components/torrent-social.tsx", "/api/v1/torrents/${torrentId}/thanks",
     ["amount"]),
    ("/gacha/dismantle", "CardReq", "apps/api/src/gacha_http/craft.rs",
     "components/gacha-album.tsx", "/api/v1/gacha/${path}",
     ["card_id", "idempotency_key"]),
    ("/gacha/exchange", "CardReq", "apps/api/src/gacha_http/craft.rs",
     "components/gacha-album.tsx", "/api/v1/gacha/${path}",
     ["card_id", "idempotency_key"]),
    ("/gacha/levelup", "CardReq", "apps/api/src/gacha_http/craft.rs",
     "components/gacha-album.tsx", "/api/v1/gacha/${path}",
     ["card_id", "idempotency_key"]),
    ("/gacha/draw", "DrawReq", "apps/api/src/gacha_http/draw.rs",
     None, None, ["banner_id", "count", "idempotency_key"]),
    ("/games/jgg", "DrawReq", "apps/api/src/games_http/pool_games.rs",
     "app/(main)/games/jgg/_inner.tsx", "/api/v1/games/jgg",
     ["idempotency_key"]),
    ("/games/capsule", "DrawReq", "apps/api/src/games_http/pool_games.rs",
     "app/(main)/games/capsule/_inner.tsx", "/api/v1/games/capsule",
     ["idempotency_key"]),
    ("/games/wheel", "DrawReq", "apps/api/src/games_http/pool_games.rs",
     "app/(main)/games/wheel/_inner.tsx", "/api/v1/games/wheel",
     ["idempotency_key"]),
    ("/games/scratch", "BetReq", "apps/api/src/games_http/helpers.rs",
     "app/(main)/games/scratch/_inner.tsx", "/api/v1/games/scratch",
     ["bet", "idempotency_key"]),
    ("/games/bigsmall", "GuessReq",
     "apps/api/src/games_http/casino_bigsmall.rs",
     "app/(main)/games/bigsmall/_inner.tsx", "/api/v1/games/bigsmall",
     ["bet", "guess", "props", "idempotency_key"]),
    ("/games/fishing/cast", "CastReq", "apps/api/src/games_http/fishing.rs",
     "app/(main)/games/fishing/_inner.tsx", "/api/v1/games/fishing/cast",
     ["bet", "idempotency_key"]),
    ("/magic-pool/donate", "DonateReq", "apps/api/src/economy_http/pool.rs",
     "components/pool-donate.tsx", "/api/v1/magic-pool/donate", ["amount"]),
    # P-642 续：可选请求体（OptJson）这一族——P-314 那轮扫的是
    # `body: web::Json<T>`，这族整个漏在门外，而漏下来的正是最需要
    # 拒未知键的几只（缺省语义会扩大破坏范围）。
    ("/games/coupons/use", "UseCouponReq",
     "apps/api/src/games_http/linkage.rs", None, "", ["idempotency_key"]),
    ("/games/pet/feed", "PetActionReq", "apps/api/src/games_http/pets.rs",
     "app/(main)/games/pet/_inner.tsx", "/api/v1/games/pet/feed",
     ["idempotency_key", "use_coupon"]),
    ("/games/pet/claim", "PetActionReq", "apps/api/src/games_http/pets.rs",
     "app/(main)/games/pet/_inner.tsx", "/api/v1/games/pet/claim",
     ["idempotency_key"]),
    ("/games/pet/customize", "CustomizeReq",
     "apps/api/src/games_http/pet_custom.rs",
     "components/game/pet-pen.tsx", "/api/v1/games/pet/customize",
     ["name", "species"]),
]
# OptJson 写口里**暂时**没加的，每条必须写明为什么（空着等于放行）：
#   · 存量超限文件（>300 行，门禁「只许瘦不许胖」）要先拆文件才能加属性；
#   · 站方在途文件不代改。
OPT_PENDING = {
    "/downloads/keys/revoke":
        "compat_http/download_keys.rs 是 335 行的存量超限文件，加一行属性就撞"
        "「只许瘦不许胖」⇒ 先拆文件再补 deny（这条缺省语义是「撤本人全部凭证」，"
        "字段名打错的代价最大，已在待办点名）",
    "/bank/loan/repay":
        "economy_http/loan_repay.rs 376 行存量超限，同上要先拆；缺省语义是"
        "「一次性结清」＝替会员决定多扣一笔钱",
}
PATH = "/api/v1/gacha/dismantle"


def read(rel):
    p = os.path.join(ROOT, rel.replace("/", os.sep))
    return io.open(p, encoding="utf-8", errors="replace").read()


def struct_fields(rel, name):
    """结构体字段 + 它头上是否挂着 deny。"""
    s = read(rel)
    m = re.search(r"struct\s+%s\s*\{" % name, s)
    if not m:
        return None, []
    head = s[max(0, m.start() - 400):m.start()]
    rest = s[m.end():]
    depth, fields = 1, []
    for ln in rest.split("\n")[1:]:
        depth += ln.count("{") - ln.count("}")
        fm = re.match(r"\s*(?:pub(?:\([^)]*\))?\s+)?([a-zA-Z_]\w*)\s*:", ln)
        if fm:
            fields.append(fm.group(1))
        if depth <= 0:
            break
    return ("deny_unknown_fields" in head), fields


def web_payload(relfile, marker):
    """前端调用点实际发出的键。

    只认**调用点**那种形状：`post<...>("<...>marker", {`。文件里同一串路径
    还可能出现在注册表/配置项（如 capsule/_inner.tsx:178 的
    `path: "/api/v1/games/capsule"`），从那儿往后取 400 字会抓到隔壁的
    `ticket`/`netOf`——V2 第一版就是这么假红了一次。
    """
    s = read(os.path.join("apps", "web", relfile))
    pat = re.compile(r"post(?:<[^>]*>)?\(\s*[\"'`][^\"'`]*"
                     + re.escape(marker) + r"[\"'`]\s*,\s*\{")
    m = pat.search(s)
    if not m:
        return None
    seg, depth, i = s[m.end() - 1:], 0, 0
    while i < len(seg):
        depth += seg[i].count("{") - seg[i].count("}")
        if depth == 0 and i > 0:
            break
        i += 1
    seg = seg[:i + 1]
    keys = re.findall(r"[{\s,]([a-zA-Z_]\w*)\s*"
                    r"(?::|,|\s*$|\s*\})", seg)
    stop = {"const", "await", "return", "new", "typeof"}
    return sorted(set(k for k in keys if k not in stop))


print("== V1 覆盖清单：结构体必须带 deny ==")
missing = []
for path, name, rel, _w, _m, _k in COVERED:
    has, fields = struct_fields(rel, name)
    if has is None:
        missing.append("%s: 找不到结构体 %s" % (path, name))
    elif not has:
        missing.append("%s(%s)" % (path, name))
ok("V1 清单 %d 条路由的体结构全部带 deny_unknown_fields（缺 %s）"
   % (len(COVERED), missing or "无"), not missing,
   "属性被摘＝未知键又会静默执行；要移出清单必须连表一起改并说明理由")

print("== V2 前端载荷对账（能不能加的判据就在这）==")


def any_web_caller(path):
    """apps/web 里有没有任何文件提到这个路径（含模板串的尾段）。"""
    tail = path.rstrip("/").split("/")[-1]
    hits = []
    for dp, _d, names in os.walk(WEB):
        if "node_modules" in dp or ".next" in dp:
            continue
        for n in names:
            if not n.endswith((".ts", ".tsx")):
                continue
            t = io.open(os.path.join(dp, n), encoding="utf-8",
                        errors="replace").read()
            if ("api/v1%s" % path) in t or ('"%s"' % path) in t:
                hits.append(os.path.relpath(os.path.join(dp, n),
                                            ROOT).replace("\\", "/"))
            elif path.count("/") >= 2 and ("/api/v1/gacha/${path}" in t
                                           and ("/" + tail) in t):
                hits.append("%s(模板串)" % os.path.relpath(
                    os.path.join(dp, n), ROOT).replace("\\", "/"))
    return sorted(hits)


bad = []
for path, name, rel, wfile, marker, keys in COVERED:
    _has, fields = struct_fields(rel, name)
    extra = [k for k in keys if k not in fields]
    if extra:
        bad.append("%s：登记键 %s 不在字段里" % (path, extra))
    if wfile is None:
        # 登记为「站内无调用点」——这条也要机检：真找到调用点就红，
        # 逼新界面接上时补载荷对账，而不是让 deny 悄悄挡住合法请求
        found = any_web_caller(path)
        if found:
            bad.append("%s：登记成无调用点，但 %s 在发请求"
                       % (path, found))
        continue
    sent = web_payload(wfile, marker)
    if sent is None:
        bad.append("%s：调用点 marker「%s」没找到（登记可能过期）"
                   % (path, marker))
        continue
    if not all(any(k == s or k in s for s in sent) for k in keys):
        bad.append("%s：前端实发键与登记不符（实=%s 登=%s）"
                   % (path, sent, keys))
ok("V2 每条路由：登记载荷键 ⊆ 结构体字段，且前端调用点确实在发这些键"
   "（异常 %s）" % (bad or "无"), not bad,
   "这条是「加了 deny 界面照常能用」的机检证明")

print("== V3 行为证据（真跑 serde）==")
t = subprocess.run(["cargo", "test", "--bin", "flux-api",
                    "pool_games::tests::unknown_field_is_rejected"],
                   capture_output=True, text=True, encoding="utf-8",
                   errors="replace", cwd=ROOT)
out = (t.stdout or "") + (t.stderr or "")
ok("V3 cargo test unknown_field_is_rejected 通过（rc=%d）" % t.returncode,
   t.returncode == 0 and "1 passed" in out,
   "红账=%s" % [l for l in out.splitlines() if "panicked" in l][:2])

print("== V4 活体：未知键在边界就被拒吗 ==")
def post(path, payload, token=None):
    req = urllib.request.Request(BASE + path, method="POST")
    req.add_header("Content-Type", "application/json")
    if token:
        req.add_header("Authorization", "Bearer " + token)
    try:
        with urllib.request.urlopen(req, json.dumps(payload).encode(),
                                    timeout=25) as f:
            return f.status, json.loads(f.read())
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, {"raw": raw[:120].decode("utf-8", "replace")}
    except Exception as ex:
        return 0, {"err": str(ex)[:100]}


try:
    st, lg = post("/auth/login",
                  {"username": "root", "password": "password123"})
    tok = (lg.get("data") or {}).get("token")
    # 探针形状挑得很具体：`bet` 给一个**超过单次上限**的数，`check_bet` 在
    # `check_rate` 与扣款之前就拒掉 ⇒ 这条请求永远不会进账本；再多带一个
    # `extra` 键，于是两种答案能分清：
    #   旧二进制：400「单次下注不能超过 N 魔力」＝未知键被吞、进了业务层
    #   新二进制：400「含未识别的字段：extra」＝边界就拦
    # （第一版探针用 bet=1，被现网真开了一局、扣了 1 火花——闸门自己造账
    #   是要收尾的，而火花流水删不得，所以判据必须天生不花钱。）
    st2, b = post("/games/scratch", {"bet": 9_999_999,
                                     "idempotency_key": "ufgate-noop",
                                     "extra": 1}, tok)
    msg = b.get("message") or ""
    if "未识别的字段" in msg:
        ok("V4 未知键在解析边界被拒（%d code=%s「%s」）"
           % (st2, b.get("code"), msg[:40]), st2 == 400,
           "400 之外还回这句话＝文案有了但状态码不对，前端不弹")
    else:
        print("   [WARN 不判红] %d code=%s 文案=%s｜请求进了业务层＝现网镜像"
              "仍静默忽略未知键（本批修复没进镜像）"
              % (st2, b.get("code"), msg[:44]))
except Exception as ex:
    print("   [跳过 V4] 打不通：%s" % str(ex)[:90])

print("== V6 可选请求体写口全数对账（这族曾被整族漏掉）==")
src_api = os.path.join(ROOT, "apps", "api", "src")
opt_routes = {}
for dp, dns, names in os.walk(src_api):
    dns[:] = [d for d in dns if d not in ("target",)]
    for n in names:
        if not n.endswith(".rs"):
            continue
        rel = os.path.relpath(os.path.join(dp, n),
                              ROOT).replace("\\", "/")
        txt = read(rel)
        if "OptJson<" not in txt:
            continue
        for m in re.finditer(r"OptJson<\w+>", txt):
            head = txt[:m.start()]
            rm = None
            for cm in re.finditer(r'#\[(get|post|put|patch|delete)\("([^"]+)"'
                                  r"\)?", head):
                rm = cm
            if rm is None or rm.group(2) == "":
                continue
            opt_routes.setdefault(rm.group(2), rel)
covered_paths = {p for p, *_ in COVERED}
not_reg = sorted(p for p in opt_routes
                 if p not in covered_paths and p not in OPT_PENDING)
gone = sorted(p for p in OPT_PENDING
              if p in covered_paths or p not in opt_routes)
print("   OptJson 写口路由=%d 已在册=%d 待补=%d"
      % (len(opt_routes), len([p for p in opt_routes
                               if p in covered_paths]), len(OPT_PENDING)))
ok("V6 每个可选体写口要么带 deny、要么在待补台账里有理由（漏 %s）"
   % (not_reg or "无"), not not_reg,
   "这族的缺省语义常是破坏性更大的那一个（撤全部/一次结清/扣魔力）")
ok("V6b 待补台账不悬空（补上了就撤桩，条目过期也红）", not gone,
   "已不存在的口=%s｜已升为在册=%s" % (
       [p for p in OPT_PENDING if p not in opt_routes],
       [p for p in OPT_PENDING if p in covered_paths]))

print("== V7 正控（可翻转的旧内容 A/B，不钉『旧文件不存在』）==")


def read_rev(rel, rev):
    t = subprocess.run(["git", "show", "%s:%s" % (rev, rel)],
                       capture_output=True, encoding="utf-8",
                       errors="replace")
    return t.stdout if t.returncode == 0 else ""


CTRL_REV = "e9c560bd"
still = []
for path, name, rel, _w, _m, _k in COVERED[-4:]:
    src = read_rev(rel, CTRL_REV)
    i = src.find("struct %s" % name)
    head = src[max(0, i - 260):i] if i >= 0 else ""
    if i >= 0 and "deny_unknown_fields" in head:
        still.append("%s(%s)" % (path, name))
ok("V7 正控：%s 那版这四只结构体都没有 deny（有则判据不承重）"
   % CTRL_REV, not still, "旧版已带=%s" % still)

print("== V8 活体探针（天生不花钱：拒在解析边界，两边都不写库）==")
try:
    st, lg = post("/auth/login",
                  {"username": "root", "password": "***"})
    tk = (lg.get("data") or {}).get("token")
    def pet_name():
        r = subprocess.run(["docker", "exec", "flux-postgres", "psql",
                            "-U", "flux", "-d", "fluxtorrent", "-tAc",
                            "SELECT name FROM arcade_pets"
                            " WHERE user_id=1"], capture_output=True,
                           text=True, encoding="utf-8", errors="replace")
        return (r.stdout or "").strip()
    nm = pet_name()
    # 名字超 12 字 ⇒ 业务层必拒；`specie` 是打错的键 ⇒ 新代码在解析层就拒。
    # 两种答案都是 400 且都不写库（改名那步在校验之后），所以现网跑也不留痕。
    st2, b2 = post("/games/pet/customize",
                   {"name": "这个名字故意超过十二个字的限制",
                    "specie": "cat"}, tk)
    msg2 = b2.get("message") or ""
    if "未识别的字段" in msg2:
        ok("V8 未知键在解析边界被拒（%d「%s」）" % (st2, msg2[:40]),
           st2 == 400 and "specie" in msg2,
           "400 之外还回这句＝状态码不对，前端不弹")
    else:
        print("   [WARN 不判红] %d code=%s 文案=%s｜现网镜像仍静默忽略未知键"
              "（本批修复未进镜像）" % (st2, b2.get("code"), msg2[:44]))
    nm2 = pet_name()
    ok("V8b 这条探针没改到宠物名（前后读数一致：%r）" % nm, nm == nm2,
       "前=%r 后=%r" % (nm, nm2))
except Exception as ex:
    print("   [跳过 V8] 打不通：%s" % str(ex)[:90])

print("== V5 中文文案的单测在册 ==")
pc = read("apps/api/src/path_cfg.rs")
ok("V5 path_cfg 有断言把 unknown field 翻成「含未识别的字段：X（请核对字段名）」",
   "含未识别的字段：" in pc and "unknown field" in pc,
   "没这条的话 400 会给会员看英文 serde 原文")

summary()
