# -*- coding: utf-8 -*-
"""批八闸门 E：建站后台写侧 → 前台可见层（站长改了到底变不变）。

这是本项目的第一卖点（通用建站 / 自定义维度），所以要验的是**闭环**：
  新建分类 → 发种表单能选到 → 发得出种 → 列表筛得到
  分类「免审」开关 → 该分类下发种真的直接过审
  新建维度（多选）→ 表单出现该字段 → 上传带值 → 详情页规格区看得到
  维度设为必填 → 不带就发不出去
  维度停用 → 表单不再出现，但存量种的已填值不该凭空消失
  标签字典新增 → 打标面板选得到 → 详情页标签条渲染 → 删除后的处置
每条否定式断言都配正控；destructive 边界只要求「可读的反应 + 不 500」，
把实际值打出来，避免把「我猜的口径」当成站点的口径。
"""
import hashlib
import json
import os
import subprocess
import sys
import time
import urllib.error
import urllib.request

sys.stdout.reconfigure(encoding="utf-8")
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from pt_audit_lib import BASE, call, login, ok, summary  # noqa: E402

ROOT = login()
CLEANUP = []
SUF = str(int(time.time()))[-6:]


def q(sql_text):
    p = subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-tAc", sql_text],
        capture_output=True, text=True, encoding="utf-8")
    return p.stdout.strip()


def exec_sql(sql_text):
    subprocess.run(
        ["docker", "exec", "flux-postgres", "psql", "-U", "flux", "-q",
         "-d", "fluxtorrent", "-c", sql_text],
        capture_output=True, text=True, encoding="utf-8")


def brief(resp):
    return "msg=%s code=%s" % (str(resp.get("message"))[:60],
                               resp.get("code"))


def items_of(resp):
    d = resp.get("data") or []
    return d.get("items") if isinstance(d, dict) else d


def benc(obj):
    if isinstance(obj, int):
        return b"i%de" % obj
    if isinstance(obj, bytes):
        return b"%d:%s" % (len(obj), obj)
    if isinstance(obj, str):
        return benc(obj.encode())
    if isinstance(obj, list):
        return b"l" + b"".join(benc(x) for x in obj) + b"e"
    if isinstance(obj, dict):
        out = b"d"
        for k in sorted(obj.keys()):
            out += benc(k) + benc(obj[k])
        return out + b"e"
    raise TypeError(type(obj))


def make_torrent(name, total=4096, piece=16384):
    count = max(1, (total + piece - 1) // piece)
    info = {
        b"name": name.encode(),
        b"piece length": piece,
        b"pieces": hashlib.sha1(b"piece").digest() * count,
        b"private": 1,
        b"length": total,
    }
    return benc({b"info": info,
                 b"announce": b"http://127.0.0.1:7070/announce/x"})


BOUNDARY = "----fluxsite99"


def multipart(fields, content):
    out = b""
    for k, v in fields.items():
        if v is None:
            continue
        out += ('--%s\r\nContent-Disposition: form-data; name="%s"\r\n'
                '\r\n%s\r\n' % (BOUNDARY, k, v)).encode()
    out += ('--%s\r\nContent-Disposition: form-data; name="file"; '
            'filename="site.torrent"\r\n'
            "Content-Type: application/x-bittorrent\r\n\r\n"
            % BOUNDARY).encode()
    out += content + b"\r\n" + ("--%s--\r\n" % BOUNDARY).encode()
    return out


def upload(cid, torrent_name, **fields):
    body = multipart({"name": torrent_name,
                      "category_id": str(cid),
                      "descr": "建站闭环探针正文", **fields},
                     make_torrent(torrent_name))
    req = urllib.request.Request(
        BASE + "/torrents?category_id=%s" % cid, method="POST", data=body)
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=%s" % BOUNDARY)
    req.add_header("Authorization", "Bearer " + ROOT)
    try:
        r = urllib.request.urlopen(req, timeout=40)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, {"raw": raw[:200].decode("utf-8", "replace")}


def drop_torrent(tid):
    exec_sql("DELETE FROM tags WHERE torrent_id = %d; "
             "DELETE FROM torrent_sections WHERE torrent_id = %d; "
             "DELETE FROM torrents WHERE id = %d" % (tid, tid, tid))


WEB = os.environ.get("FLUX_WEB_BASE", "http://127.0.0.1:3000")


def page(tid, tok):
    """SSR 出的详情页 HTML（带会话 cookie）。

    配置面的最终判据在这一层：接口供得到而页面渲染不出，站长看到的仍是
    「改了没反应」。web 容器不在时这里会回 HTTPxxx，断言如实红。
    """
    req = urllib.request.Request("%s/torrent/%s" % (WEB, tid))
    req.add_header("Cookie", "flux.session=1; flux_token=%s" % tok)
    try:
        body = urllib.request.urlopen(req, timeout=60).read()
        return body.decode("utf-8", "replace")
    except urllib.error.HTTPError as e:
        return "HTTP%s" % e.code
    except Exception as e:  # 端口不通/超时也要如实报，不让页面断言静默消失
        return "ERR:%s" % e


def kinds_of(resp):
    """/section-dict 形态：{kinds: [元数据…], <kind>: [选项行…], …}"""
    d = resp.get("data") or {}
    return d.get("kinds") if isinstance(d, dict) else (d or [])


def opts_of(resp, kind):
    d = resp.get("data") or {}
    return (d.get(kind) or []) if isinstance(d, dict) else []


def tag_rows_of(resp):
    """/tags-dict 自 0160 起回 {tags: [[id, name, kind], …]}（元组行，
    web 侧 torrents 页就是按元组解析的，不是缺陷）。"""
    d = resp.get("data") or {}
    rows = d.get("tags") if isinstance(d, dict) else d
    out = []
    for r in rows or []:
        out.append({"id": r[0], "name": r[1], "kind": r[2]}
                   if isinstance(r, list) else r)
    return out


print("== E1 新建分类：前台选得到、发得出、筛得到 ==")
CAT_NAME = "审计剧%d" % int(SUF)
s, r = call("POST", "/admin/categories", {"name": CAT_NAME, "sort": 999,
                                          "parent_id": 0}, token=ROOT)
CID = (r.get("data") or {}).get("id")
ok("E1 新建分类 200", s in (200, 201) and CID, "http=%s %s" % (s, brief(r)))
if not CID:
    sys.exit("E1 失败，后续闭环无从测起")
CLEANUP.append("DELETE FROM categories WHERE id = %d" % CID)
# 前台类目源 = /site-profile（发种页与导航都吃它），不是 /categories
s, r = call("GET", "/site-profile", token=ROOT)
cats = ((r.get("data") or {}).get("categories")) or []
hit = next((c for c in (cats or []) if c.get("id") == CID), None)
ok("E1 分类出现在公开类目接口（发种表单的选项源）", hit is not None,
   "http=%s n=%s" % (s, len(cats or [])))
ok("E1 正控：改名能改（同一通道活着，不是「本来就在那」）",
   call("PUT", "/admin/categories/%s" % CID, {"name": CAT_NAME + "x"},
        token=ROOT)[0] == 200
   and q("SELECT name FROM categories WHERE id = %d" % CID)
   == CAT_NAME + "x", q("SELECT name FROM categories WHERE id = %d" % CID))
NAME1 = "_site_e1_%s" % SUF
st, r = upload(CID, NAME1, name=NAME1, small_descr="分类闭环种")
T1 = (r.get("data") or {}).get("id")
ok("E1 新分类下能发种", st == 200 and T1, "http=%s %s" % (st, brief(r)))
s, r = call("GET", "/torrents?category_id=%s&limit=100&alive=0" % CID,
            token=ROOT)
ok("E1 列表按新分类筛得到这颗种",
   s == 200 and any(str((x or {}).get("id")) == str(T1) for x in items_of(r)),
   "http=%s %s" % (s, brief(r)))

def new_uploader(tag):
    """后台建号 → 降到 class 1 → 改初始密码 → 登录。

    为什么不能用站长发种：站长 class 99 ≥ 免审等级阈值（缺省 92），
    而阈值设定只认 0..=99（999 会被 parse 门挡回落回缺省），
    站长发种永远免审 ⇒ 分类开关是不是被读都看不出来。class 1 的作者
    既不够免审等级也没有审批特权，分类开关才是唯一变量。
    """
    user = "ae%s%d" % (tag, int(time.time()) % 100000)
    pw = "Gate#123456"
    s, r = call("POST", "/admin/adduser",
                {"username": user, "password": pw,
                 "email": "%s@ae.local" % user}, token=ROOT)
    if s != 200:
        return None, "adduser http=%s %s" % (s, brief(r))
    uid = q("SELECT id FROM users WHERE username = '%s'" % user)
    CLEANUP.append("UPDATE users SET status = 2 WHERE id = %s" % uid)
    exec_sql("UPDATE users SET class_id = 1 WHERE id = %s" % uid)
    t0 = login(user, pw)
    s, r = call("POST", "/me/password/change",
                {"old_password": pw, "new_password": pw + "x"}, token=t0)
    if s != 200:
        return None, "改初始密码失败 http=%s %s" % (s, brief(r))
    return login(user, pw + "x"), ""


print("== E2 分类「免审」开关：发种真的直接过审 ==")
UTOK, UWHY = new_uploader("up")
ok("E2 普通作者探针号可准备（class 1，不够免审等级）", UTOK is not None,
   UWHY)


def upload_as(tok, cid, name):
    fields = {"name": name, "small_descr": "开关探针",
              "descr": "建站闭环探针正文，长度足够过常规校验",
              "category_id": str(cid)}
    body = multipart(fields, make_torrent(name))
    req = urllib.request.Request(
        BASE + "/torrents?category_id=%s" % cid, method="POST", data=body)
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=%s" % BOUNDARY)
    req.add_header("Authorization", "Bearer " + tok)
    try:
        rr = urllib.request.urlopen(req, timeout=40)
        return rr.status, json.loads(rr.read())
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, {"raw": raw[:200].decode("utf-8", "replace")}


def status_of(tid):
    return q("SELECT approval_status FROM torrents WHERE id = %s" % tid)


UT = UTOK or ROOT
s, r = call("PUT", "/admin/categories/%s/flags" % CID,
            {"auto_approve": False}, token=ROOT)
st, r = upload_as(UT, CID, "_site_e2_%s" % SUF)
T2 = (r.get("data") or {}).get("id")
ok("E2 正控：开关关着时普通作者发种落待审（0）",
   st == 200 and T2 and status_of(T2) == "0",
   "http=%s tid=%s st=%s 状态=%s" % (st, T2, brief(r), status_of(T2)))
s, r = call("PUT", "/admin/categories/%s/flags" % CID,
            {"auto_approve": True}, token=ROOT)
ok("E2 打开分类免审 200", s == 200, "http=%s %s" % (s, brief(r)))
ok("E2 开关落库", q("SELECT auto_approve::text FROM categories WHERE id = %d"
                    % CID) == "true",
   q("SELECT auto_approve::text FROM categories WHERE id = %d" % CID))
st, r = upload_as(UT, CID, "_site_e3_%s" % SUF)
T3 = (r.get("data") or {}).get("id")
ok("E2 免审分类下普通作者发种 → 直接过审（开关有消费者）",
   st == 200 and T3 and status_of(T3) == "1",
   "http=%s tid=%s st=%s 状态=%s" % (st, T3, brief(r), status_of(T3)))

print("== E3 新建多选维度：表单出现 → 上传带值 → 详情页渲染 ==")
KIND = "ae_audit_%s" % SUF
s, r = call("POST", "/admin/section-kinds",
            {"kind": KIND, "label": "审计维度", "field_type": "multiselect",
             "multiple": True, "enabled": True, "sort": 900}, token=ROOT)
ok("E3 新建维度 200", s == 200, "http=%s %s" % (s, brief(r)))
CLEANUP.append("DELETE FROM section_dict WHERE kind = '%s'" % KIND)
CLEANUP.append("DELETE FROM section_kinds WHERE kind = '%s'" % KIND)
s, r = call("GET", "/section-dict?category_id=%s" % CID, token=ROOT)
kinds = kinds_of(r) or []
krow = next((k for k in kinds if k.get("kind") == KIND), None)
ok("E3 维度出现在发种表单的字段源（/section-dict）", krow is not None,
   "http=%s n=%s %s" % (s, len(kinds), brief(r)))
ok("E3 正控：字段类型带着走（前台按它决定控件形态）",
   str((krow or {}).get("field_type")) == "multiselect",
   "row=%s" % str(krow)[:120])
D1 = (call("POST", "/admin/section-dict", {"kind": KIND, "name": "选项甲",
                                           "sort": 1}, token=ROOT)[1]
      .get("data") or {}).get("id")
D2 = (call("POST", "/admin/section-dict", {"kind": KIND, "name": "选项乙",
                                           "sort": 2}, token=ROOT)[1]
      .get("data") or {}).get("id")
ok("E3 维度选项可新增（字典两条）", D1 and D2, "d1=%s d2=%s" % (D1, D2))
s, r = call("GET", "/section-dict?category_id=%s" % CID, token=ROOT)
krow = next((k for k in (kinds_of(r) or []) if k.get("kind") == KIND), {})
opts = opts_of(r, KIND)
ok("E3 选项出现在表单源里", len(opts) == 2, "opts=%s" % str(opts)[:120])
SEC = json.dumps({KIND: {"dict_ids": [D1, D2]}})
NAME4 = "_site_e4_%s" % SUF
st, r = upload(CID, NAME4, name=NAME4, small_descr="带维度种",
               sections=SEC)
T4 = (r.get("data") or {}).get("id")
ok("E3 上传带新维度的值 → 200", st == 200 and T4,
   "http=%s %s" % (st, brief(r)))
s, agg = call("GET", "/torrents/%s/aggregate" % T4, token=ROOT)
det = (agg.get("data") or {}).get("detail") or {}
sblob = json.dumps(det.get("sections") or {}, ensure_ascii=False)
ok("E3 详情页规格区供得到该维度的两个值",
   "选项甲" in sblob and "选项乙" in sblob, "http=%s sec=%s" % (s, sblob[:150])
   )
s, r = call("GET", "/torrents?sec_%s=%s&limit=100&alive=0" % (KIND, D1),
            token=ROOT)
ok("E3 按新维度筛选命中这颗种（筛选器认识新维度）",
   s == 200 and any(str((x or {}).get("id")) == str(T4) for x in items_of(r)),
   "http=%s %s" % (s, brief(r)))
s, r = call("GET", "/torrents?sec_%s=999999&limit=100&alive=0" % KIND,
            token=ROOT)
ok("E3 正控：不存在的选项筛不到它（判据不是恒真）",
   s == 200 and not any(str((x or {}).get("id")) == str(T4)
                        for x in items_of(r)), "http=%s" % s)

print("== E4 维度必填：不带就发不出去 ==")
s, r = call("PUT", "/admin/section-kinds/%s" % KIND,
            {"kind": KIND, "label": "审计维度", "field_type": "multiselect",
             "multiple": True, "enabled": True, "required": True},
            token=ROOT)
ok("E4 维度设为必填 200", s == 200, "http=%s %s" % (s, brief(r)))
NAME5 = "_site_e5_%s" % SUF
st, r = upload(CID, NAME5, name=NAME5, small_descr="漏必填维度")
T5 = (r.get("data") or {}).get("id")
ok("E4 不带必填维度发种被拒（必填真的生效）",
   st == 400 and "审计维度" in str(r.get("message")),
   "http=%s %s" % (st, brief(r)))
if T5:
    drop_torrent(T5)
st, r = upload(CID, "_site_e6_%s" % SUF, name="_site_e6_%s" % SUF,
               small_descr="补齐必填", sections=json.dumps(
                   {KIND: {"dict_ids": [D1]}}))
T6 = (r.get("data") or {}).get("id")
ok("E4 正控：补上必填维度后发得出（不是无条件拒）",
   st == 200 and T6, "http=%s %s" % (st, brief(r)))

print("== E5 维度停用：表单不再出现，存量值不许消失 ==")
s, r = call("PUT", "/admin/section-kinds/%s" % KIND,
            {"kind": KIND, "label": "审计维度", "field_type": "multiselect",
             "multiple": True, "enabled": False, "required": False},
            token=ROOT)
ok("E5 停用维度 200", s == 200, "http=%s %s" % (s, brief(r)))
s, r = call("GET", "/section-dict?category_id=%s" % CID, token=ROOT)
ok("E5 停用后表单源里没有它",
   not any(k.get("kind") == KIND for k in (kinds_of(r) or [])),
   "http=%s" % s)
s, agg = call("GET", "/torrents/%s/aggregate" % T4, token=ROOT)
det = (agg.get("data") or {}).get("detail") or {}
ok("E5 正控：存量种的已填值仍渲染（停用≠删数据）",
   "选项甲" in json.dumps(det.get("sections") or {}, ensure_ascii=False),
   "http=%s sec=%s" % (s, str(det.get("sections"))[:120]))

print("== E6 标签字典：新增可选 → 打标可渲染 → 删除有处置 ==")
TAG_NAME = "审计标%s" % SUF
s, r = call("POST", "/admin/tags-dict", {"name": TAG_NAME,
                                         "scope": "torrent"}, token=ROOT)
TD = (r.get("data") or {}).get("id")
ok("E6 新增标签字典项 200", s == 200 and TD, "http=%s %s" % (s, brief(r)))
if TD:
    CLEANUP.append("DELETE FROM tags WHERE tag_id = %d" % TD)
    CLEANUP.append("DELETE FROM tag_dict WHERE id = %d" % TD)
s, r = call("GET", "/tags-dict", token=ROOT)
ok("E6 标签出现在打标面板的字典源",
   s == 200 and any(str(x.get("id")) == str(TD) for x in tag_rows_of(r)),
   "http=%s %s" % (s, brief(r)))
s, r = call("PUT", "/torrents/%s/tags" % T4,
            {"tag_id": TD, "on": True}, token=ROOT)
ok("E6 给种打上这个新标签", s == 200, "http=%s %s" % (s, brief(r)))
s, agg = call("GET", "/torrents/%s/aggregate" % T4, token=ROOT)
tg = (agg.get("data") or {}).get("tags") or {}
# dict 是全站标签字典（恒含所有标签名），判「打标有没有效」只能看 mine
ok("E6 详情页标签条渲染得出它",
   str(TD) in [str(x) for x in (tg.get("mine") or [])],
   "http=%s mine=%s" % (s, tg.get("mine")))
HTML = page(T4, ROOT)
ok("E6 渲染层：SSR 出的 HTML 里有这个新标签名",
   TAG_NAME in HTML, "%s… len=%s" % (HTML[:70], len(HTML)))
ok("E6 渲染层：新维度的选项名也在 HTML 里（规格区）",
   "选项甲" in HTML, "len=%s" % len(HTML))
s, r = call("DELETE", "/admin/tags-dict/%s" % TD, token=ROOT)
ok("E6 删除仍被引用的标签：有可读反应（不许 500）",
   s != 500 and s != 0, "http=%s %s 库里 tag_dict=%s tags=%s"
   % (s, brief(r), q("SELECT count(*) FROM tag_dict WHERE id = %s" % TD),
      q("SELECT count(*) FROM tags WHERE tag_id = %s" % TD)))
gone = q("SELECT count(*) FROM tag_dict WHERE id = %s" % TD)
if gone == "0":
    ok("E6 若删除放行，则种上的引用一并清掉（不留悬空行）",
       q("SELECT count(*) FROM tags WHERE tag_id = %s" % TD) == "0",
       "悬空=%s" % q("SELECT count(*) FROM tags WHERE tag_id = %s" % TD))
    # 反向正控：删掉后 HTML 里就该没有它了。有前面「里面有」对照，
    # 这一条才是判别的（同一条渲染链的两个方向各验一次）。
    ok("E6 渲染层反向：删掉后 SSR HTML 不再含该标签名",
       TAG_NAME not in page(T4, ROOT), "")

print("== E7 后台批量动作：置顶徽标要立刻跟着变 ==")


def agg_field(tid, key):
    _, a = call("GET", "/torrents/%s/aggregate" % tid, token=ROOT)
    return ((a.get("data") or {}).get("torrent") or {}).get(key)


# 先热一次缓存：批量口此前只推进列表代际，详情聚合缓存原封不动
WARM_STICKY = agg_field(T4, "sticky")
s, r = call("POST", "/admin/torrents/batch",
            {"action": "sticky", "ids": [T4], "pos_state": 1}, token=ROOT)
ok("E7 批量置顶落库",
   s == 200 and q("SELECT pos_state FROM torrents WHERE id = %d" % T4) == "1",
   "http=%s %s pos=%s" % (s, brief(r),
                          q("SELECT pos_state FROM torrents WHERE id = %d"
                            % T4)))
ok("E7 聚合立刻反映置顶（详情页的置顶徽标消费点）",
   agg_field(T4, "sticky") is True,
   "warm=%s now=%s" % (WARM_STICKY, agg_field(T4, "sticky")))
call("POST", "/admin/torrents/batch",
     {"action": "sticky", "ids": [T4], "pos_state": 0}, token=ROOT)
ok("E7 正控：取消置顶后聚合也立刻回落（不是只在变真时看得见）",
   agg_field(T4, "sticky") is False, "now=%s" % agg_field(T4, "sticky"))

print("== E8 收尾：删分类的处置 ==")
s, r = call("DELETE", "/admin/categories/%s" % CID, token=ROOT)
ok("E8 分类下有种时删除有可读反应（不许 500）",
   s != 500 and s != 0, "http=%s %s 余种=%s"
   % (s, brief(r), q("SELECT count(*) FROM torrents WHERE category_id = %d"
                     % CID)))
for tid in [t for t in (T1, T2, T3, T4, T6) if t]:
    drop_torrent(tid)
for stmt in reversed(CLEANUP):
    if stmt == "drop":
        continue
    exec_sql(stmt)
LEFT = q("SELECT count(*) FROM torrents WHERE category_id = %d" % CID)
ok("E9 探针种与探针分类无残留", LEFT == "0", "left=%s" % LEFT)
summary()
