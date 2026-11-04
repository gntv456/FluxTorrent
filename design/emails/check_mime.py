# -*- coding: utf-8 -*-
"""解析 _smtp_capture/*.eml：核对 MIME 结构、part 顺序、编码、头注入、绝对链接、转义。

输出：
  design/_shots/email-mime-report.json   逐封结构摘要 + 汇总
  design/emails/_smtp_capture/html/*.html 抽出的 HTML part（供 Chrome 渲染）
"""
import email
import email.header
import json
import os
import re
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
CAP = os.path.join(HERE, "_smtp_capture")
OUT = os.path.join(os.path.dirname(HERE), "_shots")
HTML_DIR = os.path.join(CAP, "html")
BASE = "https://flux.example.site"

NAMES = [
    "invite", "password-reset", "email-verify", "review-approved",
    "review-rejected", "application-approved", "application-rejected",
    "appeal-accepted", "appeal-rejected", "account-status", "hr-warning",
]
ORDER = []
for tag in ("zh-cn", "zh-tw", "en"):
    for evil in (False, True):
        for n in NAMES:
            ORDER.append((tag, evil, n))
        if evil:
            ORDER.append((tag, True, "generic-notice"))


def dec_header(v):
    return str(email.header.make_header(email.header.decode_header(v or "")))


def main():
    os.makedirs(OUT, exist_ok=True)
    os.makedirs(HTML_DIR, exist_ok=True)
    files = sorted((f for f in os.listdir(CAP) if f.endswith(".eml")),
                   key=lambda s: int(re.sub(r"\D", "", s) or 0))
    rows = []
    for i, fn in enumerate(files):
        tag, evil, name = ORDER[i] if i < len(ORDER) else ("?", False, "?")
        with open(os.path.join(CAP, fn), "rb") as f:
            raw = f.read()
        msg = email.message_from_bytes(raw)
        raw_head = raw.split(b"\r\n\r\n", 1)[0].decode("utf-8", "replace")
        ctype = msg.get("Content-Type", "")
        parts = list(msg.walk())[1:]  # 去掉 multipart 容器
        p_types = [p.get_content_type() for p in parts]
        ctes = [p.get("Content-Transfer-Encoding", "") for p in parts]
        charset = [p.get_content_charset() for p in parts]
        text = html = ""
        for p in parts:
            payload = p.get_payload(decode=True) or b""
            body = payload.decode("utf-8", "replace")
            if p.get_content_type() == "text/plain":
                text = body
            elif p.get_content_type() == "text/html":
                html = body
        links = re.findall(r'href="([^"]+)"', html)
        rel = [u for u in links if not u.startswith("http")]
        text_urls = re.findall(r'https?://[^\s<>"\']+', text)
        bad_text_urls = re.findall(r'(?m)^\s*/(?:[^\s]*)', text)
        escaped = ("&lt;script&gt;" in html and "&amp;" in html
                   and "<script>" not in html)
        case_escaped = ("<script>" in text) if evil else True
        inj = bool(re.search(r"(?im)^(bcc|cc|x-):", raw_head)) or \
            ("\n" in dec_header(msg.get("Subject", "")) or
             "\r" in dec_header(msg.get("Subject", "")))
        from_hdr = dec_header(msg.get("From", ""))
        bad_decode = "\ufffd" in text + html
        row = {
            "file": fn, "lang": tag, "evil": evil, "template": name,
            "subject": dec_header(msg.get("Subject", "")),
            "from": from_hdr,
            "content_type": ctype.split(";")[0].strip(),
            "part_order": p_types,
            "cte": ctes,
            "charset": charset,
            "lengths": {"text": len(text), "html": len(html)},
            "links": len(links),
            "relative_links": len(rel),
            "text_urls": len(text_urls),
            "relative_text_urls": len(bad_text_urls),
            "escaped_in_html": escaped,
            "raw_preserved_in_text": case_escaped,
            "header_injection": inj,
            "replacement_char": bad_decode,
            "ok": (ctype.startswith("multipart/alternative")
                   and p_types == ["text/plain", "text/html"]
                   and all(c in ("quoted-printable", "base64", "7bit", "8bit")
                           for c in ctes)
                   and not rel and not bad_text_urls and not inj
                   and not bad_decode
                   and (escaped or not evil
                        or name in ("password-reset",))),
        }
        rows.append(row)
        if tag == "zh-cn" or (name in ("invite", "review-rejected") and evil):
            stem = "%s-%s-%s" % (tag, "evil" if evil else "clean", name)
            with open(os.path.join(HTML_DIR, stem + ".html"), "w",
                      encoding="utf-8", newline="\n") as f:
                f.write(html)
            with open(os.path.join(HTML_DIR, stem + ".txt"), "w",
                      encoding="utf-8", newline="\n") as f:
                f.write(text)

    summary = {
        "messages": len(rows),
        "all_ok": all(r["ok"] for r in rows),
        "multipart_alternative": sum(
            r["content_type"] == "multipart/alternative" for r in rows),
        "text_first": sum(r["part_order"] == ["text/plain", "text/html"]
                          for r in rows),
        "cte_values": sorted({c for r in rows for c in r["cte"]}),
        "charsets": sorted({c for r in rows for c in r["charset"]}),
        "relative_links_total": sum(r["relative_links"] for r in rows),
        "header_injection_total": sum(r["header_injection"] for r in rows),
        "escaped_evil_total": sum(r["escaped_in_html"] for r in rows if r["evil"]),
        "evil_total": sum(r["evil"] for r in rows),
        "absolute_link_examples": [u for r in rows[:1] for u in []] or
        re.findall(r'href="([^"]+)"',
                   open(os.path.join(CAP, files[0]), "r", encoding="utf-8",
                        errors="replace").read())[:3],
    }
    with open(os.path.join(OUT, "email-mime-report.json"), "w",
              encoding="utf-8", newline="\n") as f:
        json.dump({"summary": summary, "messages": rows}, f,
                  ensure_ascii=False, indent=2)
    print(json.dumps(summary, ensure_ascii=False, indent=2))
    bad = [r["file"] for r in rows if not r["ok"]]
    print("FAILED:", bad[:5] if bad else "none")
    return 0 if summary["all_ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
