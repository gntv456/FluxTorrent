# -*- coding: utf-8 -*-
import re

p = r"D:\FluxTorrent\design\torrent-detail\种子详情页-高保真样图.html"
s = open(p, encoding="utf-8").read()

def fix_a(m):
    tag = m.group(0)
    if "href" not in tag:
        tag = tag.replace("<a ", "<a href=\"#\" ", 1)
    if "onclick" not in tag:
        tag = tag[:-1] + " onclick=\"return demo(this,event)\">"
    return tag

s = re.sub(r"<a\b[^>]*>", fix_a, s)

# 抓轨日志符号 -> SVG 图标
s = s.replace(
    '<span class="ok">\u2713 100</span>',
    '<span class="ok"><svg viewBox="0 0 24 24"><path d="M4 12l5 5L20 6"/></svg>100</span>')
s = s.replace(
    '<span class="bad">\u26a0 98</span>',
    '<span class="bad"><svg viewBox="0 0 24 24"><path d="M12 9v4M12 17h.01"/><path d="M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z"/></svg>98</span>')

open(p, "w", encoding="utf-8").write(s)

# 统计剩余问题
print("a-tags:", len(re.findall(r"<a\b", s)))
print("no-href:", len(re.findall(r"<a\b(?![^>]*href)", s)))
print("no-onclick:", len(re.findall(r"<a\b(?![^>]*onclick)", s)))
print("check-sym:", "\u2713" in s, "\u26a0" in s)
