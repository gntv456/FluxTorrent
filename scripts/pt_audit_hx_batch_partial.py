# -*- coding: utf-8 -*-
"""闸门 HX：批量收种的「部分成功」必须出声（批二百零四，钉桩 + 活体读数）。

现网实测（2026-10-20，root @ :8080，两枚种都 price=0 且 root 本人所有，零动账）：
    POST /torrents/batch-download {"ids":[49417]}
      -> 404 {"code":1004,"message":"资源不存在"}
         而 49417 明明存在且已过审（approval_status=1，只是没有原始 .torrent
         存档）——单种口对同一枚种给的是 400
         「种子文件缺失（原始 .torrent 未存档），请联系管理组补档」。
    POST /torrents/batch-download {"ids":[49417,40]}
      -> 200 application/zip，zip 里只有 1 条（40）
         服务端唯一的信号藏在 Content-Disposition 的文件名 fluxtorrent-1items.zip
         里；前端不用它，自己按**请求数**起名（torrent-batch.tsx:108
         `fluxtorrent-${ids.length}items.zip`），也没有显式 skipped 通道
         ⇒ 选 2 拿 1、界面写「2items」，成员完全不知道少了一件、更不知道为什么。

代码层三条在册缺陷（apps/api/src/torrent_http/batch.rs）：
  ① 第 77 行 `charge_for_download` 排在第 84 行 `build_torrent_bytes` **之前**，
     而循环里没有任何退款/冲账路径 ⇒ 「付费种 + 存档丢失」= 先扣魔力、先写
     「已购」流水、文件拿不到，且重试因已购跳过扣费段。P-162 那条铁律
     （资格闸门必须在扣费前）在这里被「发货能力」这一维绕过去了。
  ② 第 96-99 行一个都没成功时回 `DomainError::NotFound(ids[0])`：把「存在的种」
     报成「资源不存在」，与单种口刻意区分两种 404 的口径（download.rs 评审
     P1-4）直接矛盾。
  ③ 第 75 行 `skipped` 除声明与自增外没有读者（死变量）⇒ 部分失败没有通道。

为什么先钉不先修：本仓 api bin 当前被站长在途的 auto_review* 文件卡住编译，
Rust 侧改动这轮拿不到「编译 + 现网行为」两级证据。按仓里惯例先钉桩 + 记读数，
改的人把在册断言翻成硬判据即可（探测器在「正确形状」上必须不报在册，见 HX6）。

判据：
  HX1 在册①：循环里 charge_for_download 仍排在 build_torrent_bytes 之前；
  HX2 在册②：空结果分支仍报 NotFound；
  HX3 在册③：那个 skipped 计数器除声明/自增/push 外无读者；
  HX4 在册：只选缺存档种 ⇒ 实测仍是 1004「资源不存在」（修好即转红）；
  HX5 在册：混选少发一件 ⇒ 200 且除文件名数字外无显式 skipped 通道，
      而前端按请求数自己起名（真加了 X-Flux-Skipped 之类即转红）；
  HX6 承重：把「先发货后扣费 + 计数器有读者」的正确形状喂给 HX1/HX3 探测器，
      必须都不报在册（证明探测器不恒真）；
  HX7 台账：现网「过审 + 收费 + 无存档」种数（>0 ⇒ ①已是真金白银的漏）。

用法：`PYTHONIOENCODING=utf-8 python -u scripts/pt_audit_hx_batch_partial.py`
      HX_LIVE=0 跳过 HX4/HX5（夹具两枚都 price=0，不动钱）。
"""
import io
import os
import re
import subprocess
import sys
import urllib.error
import urllib.request
import zipfile

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
sys.path.insert(0, HERE)
from pt_audit_lib import BASE, login, ok, summary  # noqa: E402

LIVE = os.environ.get("HX_LIVE", "1") != "0"
BATCH = "apps/api/src/torrent_http/batch.rs"

# HX6 正控：正确形状（先发货 → 再扣费 → 扣费失败丢掉已取字节 → 计数器有读者）
GOOD_SHAPE = """
    let mut skipped_ids: Vec<i64> = Vec::new();
    for id in &ids {
        let bytes = match build_torrent_bytes(&state, auth.id, *id).await {
            Ok(b) => b,
            Err(_) => { skipped_ids.push(*id); continue; }
        };
        if torrents::charge_for_download(&state.repo.db, auth.id, *id)
            .await
            .is_err()
        {
            skipped_ids.push(*id);
            continue;
        }
        files.push((name_of(id), bytes));
    }
    if files.is_empty() {
        return Err(DomainError::Validation(format!(
            "{} 个种子都没能取到", skipped_ids.len())));
    }
    resp.insert_header(("X-Flux-Skipped", skipped_ids.len().to_string()));
"""


def rd(rel):
    return io.open(os.path.join(ROOT, rel), encoding="utf-8",
                   errors="replace").read()


def code_only(ts):
    """剥 // 与 /// 注释：在册判据只认真码，别被我自己写的说明喂胖。"""
    return "\n".join(ln for ln in ts.split("\n")
                     if not ln.strip().startswith("//"))


def loop_body(src):
    """取 `for id in &ids {` 那一段（到配对闭括号）。"""
    i = src.index("for id in &ids {")
    d, k = 0, i
    while k < len(src):
        if src[k] == "{":
            d += 1
        elif src[k] == "}":
            d -= 1
            if d == 0:
                return src[i:k + 1]
        k += 1
    return src[i:]


def charges_before_ship(loop):
    """①在册：charge 的位置早于 build 的位置。"""
    a = loop.find("charge_for_download")
    b = loop.find("build_torrent_bytes")
    return a >= 0 and b >= 0 and a < b


def dead_counters(src):
    """返回「只有声明 + 自增/push，没有任何读者」的累加器名清单。

    注意不能只取第一条 `let mut X = ...;`：batch.rs 里 `ids` 也是
    `let mut ids: Vec<i64> = Vec::new();` 形态，它有真读者，混进来就会把
    判据读成「找不到死计数器」（本轮 HX3 的第一次红就是这么来的）。"""
    dead = []
    for m in re.finditer(
            r"let mut (\w+)\s*(?::\s*[^=;]+)?=\s*(?:0;|Vec::new\(\);)", src):
        name = m.group(1)
        readers = []
        for mm in re.finditer(r"\b%s\b" % re.escape(name), src):
            line = src[src.rfind("\n", 0, mm.start()) + 1:
                       src.find("\n", mm.start())].strip()
            if line.startswith("let mut " + name):
                continue
            if re.fullmatch(r"%s (\+= 1|\.push\([^;]*\));"
                            % re.escape(name), line):
                continue
            readers.append(line)
        if not readers:
            dead.append(name)
    return dead


def empty_case_404(block):
    """②在册：一个都没成功时报的是 NotFound。"""
    i = block.find("if files.is_empty()")
    return i >= 0 and "NotFound" in block[i:i + 200]


batch = code_only(rd(BATCH))
loop = loop_body(batch)
ok("HX1 在册：批量口仍在扣费之后才发货（改好即红，届时升成硬判据）",
   charges_before_ship(loop),
   "charge@%s < build@%s；循环内无退款路径"
   % (loop.find("charge_for_download"), loop.find("build_torrent_bytes")))
dead = dead_counters(batch)
# 本条写这份闸门的过程中变了形：起初 batch.rs:75 的 skipped 确实是死变量，
# 站长同日在第 109 行接上了 `X-Batch-Skipped` 头 ⇒ 断言从「在册（无读者）」
# 改成「必须有读者」的硬判据（谁再把它摘掉就红）。教训：在册类断言要按
# 「有没有任一计数通道」写，别钉自己脑补的键名，也别在长回合里假设文件不动。
ok("HX3 计数通道：batch.rs 必须把失败件数报给调用方（X-Batch-Skipped）",
   re.search(r'X-Batch-Skipped", skipped\.to_string', batch) is not None,
   "无读者的累加器=%s；界面还须真读这个头（torrent-batch.tsx 现在仍按请求数"
   "自己起名）" % (dead,))
ok("HX2 在册：空结果仍报 NotFound（把存在的种说成不存在）",
   empty_case_404(batch),
   "单种口对同一枚种给的是 400「种子文件缺失…请联系管理组补档」，两路口径相反")

good = code_only(GOOD_SHAPE)
good_loop = loop_body(good)
good_dead = dead_counters(good)
ok("HX6 承重：正确形状不被 HX1/HX3 探测器误报",
   not charges_before_ship(good_loop) and "skipped_ids" not in good_dead,
   "先发货后扣费=%s；被误判成死变量的=%s"
   % (not charges_before_ship(good_loop), good_dead))

if LIVE:
    def psql(q):
        r = subprocess.run(
            ["docker", "exec", "-i", "flux-postgres", "psql", "-U", "flux",
             "-d", "fluxtorrent", "-q", "-tA"], input=q,
            capture_output=True, text=True, encoding="utf-8", errors="replace")
        err = (r.stderr or "")
        if r.returncode != 0 or "ERROR" in err:
            raise RuntimeError("psql: " + err[:300])
        return (r.stdout or "").strip()

    NO_RAW = "SELECT 1 FROM torrent_files f WHERE f.torrent_id = t.id"
    no_raw_id = int(psql("SELECT min(t.id) FROM torrents t WHERE "
                         "t.approval_status = 1 AND NOT EXISTS (%s)" % NO_RAW))
    raw_id = int(psql("SELECT min(t.id) FROM torrents t WHERE "
                      "t.approval_status = 1 AND EXISTS (%s) "
                      "AND t.price = 0" % NO_RAW))
    paid_gap = int(psql("SELECT count(*) FROM torrents t WHERE "
                        "t.approval_status = 1 AND t.price > 0 AND "
                        "NOT EXISTS (%s)" % NO_RAW) or 0)
    ok("HX7 台账：现网「过审+收费+无存档」种数（>0 ⇒ HX1 已是真金白银的漏）",
       paid_gap == 0,
       "读数=%s；=0 时漏只在「内容包导入/存档被清」后触发（np_import.rs:138 "
       "直接落 approval_status=1 且无 raw，已在册）" % paid_gap)

    tok = login("root", "password123")

    def post(ids):
        # 用 Bearer 而不是 Cookie：csrf_mw 的判据是「非安全方法 + 带了非空会话
        # cookie」，用 cookie 打会被 2003 无权限拒——那是仪器错不是产品行为
        req = urllib.request.Request(
            "%s/torrents/batch-download" % BASE,
            data=('{"ids":[%s]}' % ",".join(str(x) for x in ids)).encode(),
            headers={"Content-Type": "application/json",
                     "Authorization": "Bearer " + tok})
        try:
            with urllib.request.urlopen(req) as r:
                return r.status, r.headers, r.read()
        except urllib.error.HTTPError as e:
            return e.code, e.headers, e.read()

    st, _h, body = post([no_raw_id])
    txt = body.decode("utf-8", "replace")
    ok("HX4 在册：只选缺存档种，批量口回的是撒谎的 1004 而不是可行动中文",
       st == 404 and '"code":1004' in txt,
       "id=%s http=%s body=%s（改好后此条应转红，届时升成"
       "「必须 4xx 且 message 含『存档』」的硬判据）" % (no_raw_id, st, txt[:80]))
    st2, h2, zip_body = post([no_raw_id, raw_id])
    try:
        names = zipfile.ZipFile(io.BytesIO(zip_body)).namelist()
    except Exception as e:  # noqa: BLE001
        names = ["<非 zip: %s>" % type(e).__name__]
    cd = h2.get("content-disposition") or ""
    got = h2.get("x-batch-skipped")
    ok("HX5 硬判据：混选少发一件时容器必须带 X-Batch-Skipped 且数=选择数-条目数",
       st2 == 200 and len(names) == 1 and got == "1",
       "选 2 得 %s 条 %s，头=%s；现网已带头 ⇒ 剩下的洞在界面：torrent-batch"
       ".tsx:108 按**请求数**自己起名 fluxtorrent-2items.zip、也不读这个头，"
       "成员仍不知道少了一件（前端收口时把本条扩成「界面必须显示件数差额」）"
       % (len(names), names, got))
else:
    print("[HX4/HX5] HX_LIVE=0 跳过活体两条")

summary()
