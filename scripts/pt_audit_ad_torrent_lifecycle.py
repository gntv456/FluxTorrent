# -*- coding: utf-8 -*-
"""批七闸门 D：种子生命周期链（发种 → 编辑 → 回收站 → 审核闭环）。

资深 PT 用户视角的一条完整线：
  上传（带标签/维度/封面/MediaInfo/IMDb）
  → 部分编辑（只改正文，其余字段不许被抹）
  → IMDb 手滑填错的反应（发种口与编辑口同一规则）
  → 删除进回收站 → 列表读口 → 恢复回待审
  → 被拒 → 作者收到通知 → 冷却后重新提交
每条否定式断言都配一个正控（同一条通道改得动的东西），免得
「接口没写」和「接口写对了但值本来就那样」混成一片绿。
读口的断言落在**接口回吐**上，不落在数据库列上——库里对、接口不给，
用户看到的仍是空。
"""
import hashlib
import json
import os
import re
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
    return "msg=%s code=%s" % (str(resp.get("message"))[:56],
                               resp.get("code"))


def col(tid, expr="*"):
    return q("SELECT %s FROM torrents WHERE id = %s" % (expr, tid))


# ---------- bencode：与真实客户端一致的 .torrent ----------
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
    # 片数必须与「文件大小 / 片长」相符，否则站点的校验会拒
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


BOUNDARY = "----fluxlife77"


def multipart(fields, content):
    out = b""
    for k, v in fields.items():
        if v is None:
            continue
        out += ('--%s\r\nContent-Disposition: form-data; name="%s"\r\n'
                '\r\n%s\r\n' % (BOUNDARY, k, v)).encode()
    out += ('--%s\r\nContent-Disposition: form-data; name="file"; '
            'filename="life.torrent"\r\n'
            "Content-Type: application/x-bittorrent\r\n\r\n"
            % BOUNDARY).encode()
    out += content + b"\r\n" + ("--%s--\r\n" % BOUNDARY).encode()
    return out


def upload(tok, torrent_name, **fields):
    fields.setdefault("category_id", "1")
    body = multipart(fields, make_torrent(torrent_name))
    req = urllib.request.Request(
        BASE + "/torrents?category_id=%s" % fields["category_id"],
        method="POST", data=body)
    req.add_header("Content-Type",
                   "multipart/form-data; boundary=%s" % BOUNDARY)
    req.add_header("Authorization", "Bearer " + tok)
    try:
        r = urllib.request.urlopen(req, timeout=40)
        return r.status, json.loads(r.read())
    except urllib.error.HTTPError as e:
        raw = e.read()
        try:
            return e.code, json.loads(raw)
        except Exception:
            return e.code, {"raw": raw[:220].decode("utf-8", "replace")}


def new_member(tag):
    """管理员发码 → 注册 → 登录。作者视角（重提/收信）必须用真成员号。"""
    s, r = call("POST", "/admin/invites", {"user_id": 1, "count": 1},
                token=ROOT)
    codes = ((r.get("data") or {}).get("codes")) or []
    if not codes:
        return None, None, "发码失败 http=%s" % s
    code, cid = codes[0]["code"], codes[0]["id"]
    CLEANUP.append("DELETE FROM invites WHERE id = %d" % cid)
    s, r = call("GET", "/auth/captcha")
    cap = r.get("data") or {}
    nums = re.findall(r"-?\d+", str(cap.get("question") or ""))
    if len(nums) < 2:
        return None, None, "验证码题解析失败 http=%s" % s
    user = "ad%s%d" % (tag, int(time.time()) % 100000)
    s, r = call("POST", "/auth/register", {
        "username": user, "email": "%s@ad.local" % user,
        "password": "password123", "invite_code": code,
        "captcha_id": cap.get("captcha_id"),
        "captcha_answer": int(nums[0]) + int(nums[1]),
        "captcha_token": "", "fields": {}})
    if s != 200:
        return None, None, "注册失败 http=%s %s" % (s, r.get("message"))
    uid = (r.get("data") or {}).get("user_id")
    CLEANUP.append("UPDATE users SET status = 2 WHERE id = %d" % uid)
    CLEANUP.append("DELETE FROM messages WHERE receiver_id = %d" % uid)
    for i in range(6):
        s, r = call("POST", "/auth/login",
                    {"username": user, "password": "password123"})
        if s == 200:
            return r["data"]["token"], uid, ""
        time.sleep(3 + i * 3)
    return None, uid, "新成员登录失败 http=%s" % s


def snap(tid):
    """一行看清七件套：名字/短标/正文/IMDb/封面/MediaInfo/状态。"""
    return col(tid, "coalesce(name,'-')||'|'||coalesce(small_descr,'-')||'|'"
                    "||coalesce(replace(descr,E'\n',' '),'-')||'|'"
                    "||coalesce(imdb_id,'-')||'|'"
                    "||coalesce(media_info->>'poster','-')||'|'"
                    "||coalesce(media_info->>'mediainfo','-')||'|'"
                    "||approval_status::text")


def n_tag(tid):
    return int(q("SELECT count(*) FROM tags WHERE torrent_id = %s" % tid) or 0)


def n_sec(tid):
    return int(q("SELECT count(*) FROM torrent_sections "
                 "WHERE torrent_id = %s" % tid) or 0)


def decide(tid, approve, reason=""):
    return call("POST", "/admin/reviews/decide",
                {"torrent_id": tid, "approve": approve, "reason": reason},
                token=ROOT)


def listed(resp, tid):
    d = resp.get("data") or {}
    items = d.get("items") if isinstance(d, dict) else d
    return any(str((x or {}).get("id")) == str(tid) for x in (items or []))


TODAY = int(time.time()) % 1000000
MTOK, MID, WHY = new_member("life")
ok("D0 作者探针号可准备", MTOK is not None, WHY)

print("== D1 发种：六件套一起提交 ==")
TAGS = json.dumps([3, 6])
KIND = q("SELECT kind FROM section_kinds WHERE enabled ORDER BY sort LIMIT 1")
DICT = q("SELECT id FROM section_dict WHERE kind = '%s' ORDER BY sort "
         "LIMIT 1" % (KIND or "media"))
SECTIONS = json.dumps({KIND or "media": int(DICT or 1)})
NAME = "_life_full_%d" % TODAY
st, r = upload(ROOT, NAME, name=NAME, small_descr="生命周期短标",
               descr="上传正文一段话", tags=TAGS, sections=SECTIONS,
               poster="https://img.example/p.jpg",
               imdb="https://www.imdb.com/title/tt1234567/",
               mediainfo="完整 MediaInfo 文本")
TID = ((r.get("data") or {}).get("id")) if st == 200 else None
ok("D1 上传 200（标签/维度/封面/IMDb 链接/MediaInfo）",
   st == 200 and TID is not None, "http=%s %s" % (st, brief(r)))
if not TID:
    sys.exit("D1 失败，后续链路无从测起")
CLEANUP.append("DELETE FROM tags WHERE torrent_id = %d" % TID)
CLEANUP.append("DELETE FROM torrent_sections WHERE torrent_id = %d" % TID)
CLEANUP.append("DELETE FROM torrents WHERE id = %d" % TID)
# 换作者：审核台有「不能审自己的种」闸门，后面的裁决要用站长裁决成员种
exec_sql("UPDATE torrents SET owner_id = %d WHERE id = %d" % (MID or 1, TID))

BASE_ROW = snap(TID)
ok("D1 站长发种免审直过（approval_status=1）",
   BASE_ROW.split("|")[6] == "1", BASE_ROW)
ok("D1 标签 2 条落库", n_tag(TID) == 2, "n=%d" % n_tag(TID))
ok("D1 维度落库", n_sec(TID) == 1, "n=%d" % n_sec(TID))
ok("D1 整条 IMDb 链接被提取成编号（TT1234567）",
   "|TT1234567|" in BASE_ROW, BASE_ROW)
ok("D1 封面进 media_info.poster", "p.jpg" in BASE_ROW, BASE_ROW)
ok("D1 MediaInfo 全文进 media_info", "MediaInfo 文本" in BASE_ROW, BASE_ROW)
s, d = call("GET", "/torrents/%s/aggregate" % TID, token=ROOT)
agg = d.get("data") or {}
det = agg.get("detail") or {}
blob = json.dumps(agg, ensure_ascii=False)
ok("D1 详情页数据源（aggregate）供得到 IMDb 编号",
   "TT1234567" in blob, "http=%s imdb=%r" % (s, det.get("imdb_id")))
ok("D1 aggregate 供得到封面", "p.jpg" in blob, "poster=%r" % det.get("poster"))
ok("D1 aggregate 供得到 MediaInfo 全文",
   "MediaInfo 文本" in str(det.get("mediainfo")), "mediainfo=%r"
   % str(det.get("mediainfo"))[:60])
ok("D1 aggregate 供得到维度名（详情页「规格」区消费的就是它）",
   bool(det.get("sections")), "sections=%r" % (det.get("sections"),))
tg = agg.get("tags") or {}
mine = [str(x) for x in (tg.get("mine") or [])]
dict_names = {str(x.get("id")): x.get("name") for x in (tg.get("dict") or [])}
# `tags.dict` 是全站标签字典（恒含全部标签名），拿它判「供得到」是空判据；
# 真正随种子变的是 `tags.mine`（打标的那几个 id），名称再由 dict 解出。
ok("D1 aggregate 供得到两个标签名（详情页标签条）",
   {"3", "6"} <= set(mine)
   and {dict_names.get(m) for m in mine} >= {"官种", "首发"},
   "mine=%s names=%s" % (mine, [dict_names.get(m) for m in mine]))
s, d = call("GET", "/torrents/%s/tags" % TID, token=ROOT)
ok("D1 标签接口（打标面板）也读得到",
   s == 200 and "官种" in json.dumps(d.get("data") or [], ensure_ascii=False),
   "http=%s %s" % (s, brief(d)))
# 非法 IMDb 在发种口就该报错（与编辑口同一规则）
st, r = upload(ROOT, "_life_bad_%d" % TODAY, name="_life_bad_%d" % TODAY,
               descr="非法 imdb", imdb="abc123")
ok("D1 发种填了非法 IMDb 明确报错（不是静默丢掉）",
   st == 400, "http=%s %s" % (st, brief(r)))
BAD_TID = ((r.get("data") or {}).get("id")) if st == 200 else None
if BAD_TID:
    CLEANUP.append("DELETE FROM torrents WHERE id = %d" % BAD_TID)
# 把状态摆回「待审」：审核台的裁决只认待审种，而站长发种是免审直过的，
# 这里等价于成员发种的实际结果（作者已在前面换成探针成员号）。
exec_sql("UPDATE torrents SET approval_status = 0, approved_at = NULL "
         "WHERE id = %d" % TID)

print("== D2 部分编辑：只改正文，其余六件套不许被抹 ==")
s, r = call("PUT", "/torrents/%s" % TID, {"descr": "改过的正文"}, token=ROOT)
ok("D2 PUT 只带 descr → 200（此前整条 SQL 里混进 // 注释 ⇒ 100% 500）",
   s == 200, "http=%s %s" % (s, brief(r)))
ROW2 = snap(TID)
ok("D2 正文真的改了", "改过的正文" in ROW2, ROW2)
ok("D2 名字/短标未被抹",
   ROW2.split("|")[:2] == BASE_ROW.split("|")[:2],
   "%s -> %s" % (BASE_ROW, ROW2))
ok("D2 IMDb/封面/MediaInfo 未被抹",
   ROW2.split("|")[3:6] == BASE_ROW.split("|")[3:6],
   "%s -> %s" % (BASE_ROW, ROW2))
ok("D2 标签未被抹", n_tag(TID) == 2, "n=%d" % n_tag(TID))
ok("D2 维度未被抹", n_sec(TID) == 1, "n=%d" % n_sec(TID))
ok("D2 状态未被部分编辑动过（成员种仍待审）",
   ROW2.split("|")[6] == "0", ROW2)
s, r = call("PUT", "/torrents/%s" % TID, {"small_descr": "改过的短标"},
            token=ROOT)
ok("D2 正控：同一条 PUT 改得动短标",
   s == 200 and "改过的短标" in snap(TID),
   "http=%s %s" % (s, snap(TID)))

print("== D3 IMDb 编辑口：非法报错 / 合法改写 / 空串清空 ==")
s, r = call("PUT", "/torrents/%s" % TID, {"imdb_id": "abc"}, token=ROOT)
ok("D3 非法 IMDb → 400（此前静默 200 保留旧值）",
   s == 400, "http=%s %s 库里=%s" % (s, brief(r), snap(TID).split("|")[3]))
ok("D3 报错后库里编号未被改动",
   snap(TID).split("|")[3] == "TT1234567", snap(TID))
s, r = call("PUT", "/torrents/%s" % TID, {"imdb_id": "tt7654321"}, token=ROOT)
ok("D3 正控：合法编号能改（大写归一）",
   s == 200 and snap(TID).split("|")[3] == "TT7654321",
   "http=%s %s" % (s, snap(TID)))
s, r = call("PUT", "/torrents/%s" % TID, {"imdb_id": ""}, token=ROOT)
ok("D3 正控：空串能清空成 NULL（不是写入空串）",
   s == 200 and snap(TID).split("|")[3] == "-",
   "http=%s %s" % (s, snap(TID)))
s, r = call("PUT", "/torrents/%s" % TID,
            {"imdb_id": "https://www.imdb.com/title/tt0111167/"}, token=ROOT)
ok("D3 正控：整条链接也能识别",
   s == 200 and snap(TID).split("|")[3] == "TT0111167",
   "http=%s %s" % (s, snap(TID)))
s, r = call("PUT", "/torrents/%s" % TID, {"poster": ""}, token=ROOT)
ok("D3 正控：封面空串 = 清除",
   s == 200 and snap(TID).split("|")[4] == "-", "http=%s %s" % (s, snap(TID)))

print("== D4 回收站：作者删 → 读口 → 站长恢复 ==")
s, r = call("DELETE", "/torrents/%s" % TID, token=MTOK or ROOT)
ok("D4 作者删自己未过审种 → 软删进回收站",
   s == 200 and snap(TID).split("|")[6] == "3",
   "http=%s %s st=%s" % (s, brief(r), snap(TID).split("|")[6]))
# `alive=0`：列表默认还叠了「只回活种」，探针种 0 做种会被它先滤掉，
# 那样「出现在/不出现在列表」两条断言都不判别（分不清是被审核视图
# 挡的还是被存活筛选挡的）。全部视图下才只剩 approval 一个变量。
s, lst = call("GET", "/torrents?limit=100&alive=0", token=ROOT)
ok("D4 回收站种不出现在默认列表（存活筛选放到最宽，只剩 approval 一个变量）",
   not listed(lst, TID), "http=%s" % s)
s, lst = call("GET", "/torrents?approval=2&limit=100&alive=0", token=ROOT)
ok("D4 被拒视图含回收站（原口径维持）", s == 200 and listed(lst, TID),
   "http=%s %s" % (s, brief(lst)))
s, lst = call("GET", "/torrents?approval=3&limit=100&alive=0", token=ROOT)
ok("D4 回收站有独立读口（恢复按钮得有的可点）",
   s == 200 and listed(lst, TID), "http=%s %s" % (s, brief(lst)))
d3r = lst.get("data") or {}
it3r = (d3r.get("items") if isinstance(d3r, dict) else d3r) or []
row3 = next((x for x in it3r if str((x or {}).get("id")) == str(TID)), {})
ok("D4 正控：读出来的行带着 approval_status=3",
   str(row3.get("approval_status")) == "3", "row=%s" % str(row3)[:110])
s, lst = call("GET", "/torrents?approval=3&limit=100&alive=0",
              token=MTOK or ROOT)
d3 = lst.get("data") or {}
it3 = (d3.get("items") if isinstance(d3, dict) else d3) or []
ok("D4 正控：普通成员的 approval=3 被闸门吞掉（不泄露回收站）",
   s == 200 and not any(str((x or {}).get("approval_status")) == "3"
                        for x in it3), "n=%d" % len(it3))
s, lst = call("GET", "/torrents?approval=9&limit=20", token=ROOT)
ok("D4 越界视图参数明确报错（此前答非所问回一批过审种）",
   s == 400, "http=%s %s" % (s, brief(lst)))
s, r = call("POST", "/torrents/%s/restore" % TID, {}, token=MTOK or ROOT)
ok("D4 作者自恢复按权限口挡下（可读的 403，不是 500）",
   s in (200, 403), "http=%s %s" % (s, brief(r)))
if s == 403:
    print("   注：恢复权当前只在 torrent.manage（成员删错了只能求管理）")
    s, r = call("POST", "/torrents/%s/restore" % TID, {}, token=ROOT)
    ok("D4 站长恢复 → 回到待审",
       s == 200 and snap(TID).split("|")[6] == "0",
       "http=%s %s" % (s, brief(r)))
s, r = call("POST", "/torrents/%s/restore" % TID, {}, token=ROOT)
ok("D4 正控：不在回收站的种恢复 404（别报假成功）",
   s == 404, "http=%s %s" % (s, brief(r)))

print("== D5 审核闭环：拒绝 → 作者收信 → 冷却 → 重提 → 过审 ==")
s, r = decide(TID, False, "封面失效，请补正")
ok("D5 站长拒绝待审种 → status=2 且留拒因",
   s == 200 and col(TID, "approval_status") == "2"
   and "封面失效" in col(TID, "coalesce(deny_note,'')"),
   "http=%s %s note=%s" % (s, brief(r), col(TID, "coalesce(deny_note,'-')")))
s, ms = call("GET", "/messages/inbox?limit=30", token=MTOK)
mm = json.dumps(ms.get("data") or [], ensure_ascii=False)
ok("D5 作者在站内信里看到拒因",
   s == 200 and "封面失效" in mm, "http=%s %s" % (s, mm[:150]))
s, r = call("POST", "/torrents/%s/resubmit" % TID, {}, token=ROOT)
ok("D5 非作者重提被拒", s == 403, "http=%s %s" % (s, brief(r)))
s, r = call("POST", "/torrents/%s/resubmit" % TID, {}, token=MTOK)
ok("D5 刚被拒立刻重提有冷却（挡机械刷队列）",
   s == 400 and "冷却" in str(r.get("message")),
   "http=%s %s" % (s, brief(r)))
exec_sql("UPDATE torrents SET mtime = now() - interval '11 minutes' "
         "WHERE id = %d" % TID)
s, r = call("POST", "/torrents/%s/resubmit" % TID, {}, token=MTOK)
ok("D5 冷却期后作者可重提 → 回到待审、拒因清空",
   s == 200 and col(TID, "approval_status||'/'||coalesce(deny_note,'-')")
   == "0/-", "http=%s %s st=%s" % (s, brief(r), col(TID, "approval_status")))
s, r = call("POST", "/torrents/%s/resubmit" % TID, {}, token=MTOK)
ok("D5 正控：待审（非被拒）种重提明确报错",
   s == 400 and "仅被拒" in str(r.get("message")),
   "http=%s %s" % (s, brief(r)))
s, r = call("PUT", "/torrents/%s" % TID, {"descr": "补好封面的正文"},
            token=MTOK)
ok("D5 作者编辑自己的种（编辑口对作者同样开放）",
   s == 200, "http=%s %s" % (s, brief(r)))
s, r = decide(TID, True)
ok("D5 站长过审 → status=1 且写 approved_at",
   s == 200 and col(TID, "approval_status") == "1"
   and col(TID, "approved_at") != "", "http=%s %s" % (s, brief(r)))
s, ms = call("GET", "/messages/inbox?limit=30", token=MTOK)
ok("D5 作者也收到过审通知（标题带「种子过审：」+ 种名）",
   s == 200 and "种子过审：" in json.dumps(ms.get("data") or [],
                                           ensure_ascii=False)
   and NAME in json.dumps(ms.get("data") or [], ensure_ascii=False),
   "http=%s %s" % (s, json.dumps(ms.get("data") or [],
                                  ensure_ascii=False)[:150]))
s, r = decide(TID, True)
ok("D5 正控：已过审的种再裁决报错（不静默改一遍）",
   s == 400 and "不在待审" in str(r.get("message")),
   "http=%s %s" % (s, brief(r)))

print("== D6 详情页聚合缓存：写完必须跟着变 ==")
# 详情页读的是 /aggregate（2.5 缓存）。此前只有编辑/删除/恢复等管理口
# 会失效缓存，打标/收藏/评论这类用户写口全漏——表现是「点完没反应，
# 刷新还是老样子」，要等缓存自然过期。每条都先热一次缓存再改再读。
def agg_tags_mine():
    _, a = call("GET", "/torrents/%s/aggregate" % TID, token=ROOT)
    tg = (a.get("data") or {}).get("tags") or {}
    return [str(x) for x in (tg.get("mine") or [])]


def agg_detail():
    _, a = call("GET", "/torrents/%s/aggregate" % TID, token=ROOT)
    return (a.get("data") or {}).get("detail") or {}


def agg_comments():
    _, a = call("GET", "/torrents/%s/aggregate" % TID, token=ROOT)
    c = (a.get("data") or {}).get("comments")
    if isinstance(c, dict):
        return len(c.get("items") or [])
    return len(c or [])


WARM = agg_tags_mine()
s, r = call("PUT", "/torrents/%s/tags" % TID, {"tag_id": 8, "on": True},
            token=ROOT)
ok("D6 打标后聚合立刻反映（缓存有失效）",
   s == 200 and "8" in agg_tags_mine(),
   "warm=%s now=%s %s" % (WARM, agg_tags_mine(), brief(r)))
call("PUT", "/torrents/%s/tags" % TID, {"tag_id": 8, "on": False}, token=ROOT)
ok("D6 正控：取消标后也立刻反映（不是只在变多时看得见）",
   "8" not in agg_tags_mine(), str(agg_tags_mine()))
BEFORE = agg_detail().get("bookmark_count") or 0
s, r = call("PUT", "/torrents/%s/bookmark" % TID, {"on": True}, token=ROOT)
ok("D6 收藏数写完立刻反映",
   s == 200 and (agg_detail().get("bookmark_count") or 0) == BEFORE + 1,
   "before=%s after=%s http=%s %s" % (BEFORE,
                                      agg_detail().get("bookmark_count"),
                                      s, brief(r)))
call("PUT", "/torrents/%s/bookmark" % TID, {"on": False}, token=ROOT)
CBEF = agg_comments()
s, r = call("POST", "/torrents/%s/comments" % TID,
            {"body": "聚合失效探针评论"}, token=ROOT)
CID_OK = (r.get("data") or {}).get("id")
if CID_OK:
    CLEANUP.append("DELETE FROM comments WHERE id = %s" % CID_OK)
ok("D6 评论写完立刻反映在聚合里",
   s == 200 and CID_OK and agg_comments() == CBEF + 1,
   "before=%s after=%s http=%s %s" % (CBEF, agg_comments(), s, brief(r)))

print("== 收尾 ==")
for stmt in reversed(CLEANUP):
    exec_sql(stmt)
ok("D9 探针种无残留", col(TID, "count(*)") == "0",
   "left=%s" % col(TID, "count(*)"))
summary()
