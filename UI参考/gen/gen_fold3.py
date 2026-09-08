# -*- coding: utf-8 -*-
"""生成 三折叠 全展开(1000x760) 60页 + 半展开(720x900) 12页 + 折叠态(340x720) 10页 + 分屏 8页"""
import sys
sys.path.insert(0, '/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen')
from data import *

n = [146]
def next_id():
    n[0] += 1
    return f"e{n[0]}"

def f3_shell(title, content, label, dark=False):
    aid = next_id()
    sb = '<div class="f3-statusbar"><span>9:41</span><span>📶 🔋 100%</span></div>'
    if dark:
        sb = '<div class="f3-statusbar" style="background:#0F172A;color:rgba(255,255,255,.6)"><span>9:41</span><span>📶 🔋 100%</span></div>'
    tb = f'<div class="f3-topbar"{" style=background:#1F2A44" if dark else ""}><span class="f3-back">‹</span><b{" style=color:#fff" if dark else ""}>{title}</b><div class="f3-search" style="visibility:hidden">🔍</div><span class="f3-filter">•••</span></div>'
    return f'''    <dc-artboard id="{aid}-f3p" label="{label}" width="1000" height="760">
      <div class="fold3-screen">
        {sb}
        {tb}
        <div class="f3-content" style="padding:16px">{content}</div>
      </div>
    </dc-artboard>
'''

def f3_grid_item(icon, name, sub, color, big=False):
    return f'<div style="background:linear-gradient(135deg,{color},#ffffff22);border-radius:12px;padding:{16 if big else 12}px;text-align:center;background:#fff;box-shadow:0 2px 8px rgba(0,0,0,.04)"><span style="font-size:{26 if big else 22}px">{icon}</span><b style="display:block;font-size:{13 if big else 12}px;margin-top:5px">{name}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">{sub}</p></div>'

def f3_tor(t, show_tags=True):
    def _tags():
        parts = [f'<span style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:1px 6px;font-size:9px">{x}</span>' for x in t[6].split("/")]
        return '<div style="display:flex;gap:5px;margin-top:6px">' + "".join(parts) + '</div>'
    tags = _tags() if show_tags else '' ''
    return f'''<div style="background:#fff;border-radius:11px;padding:11px 12px;display:flex;gap:10px;align-items:center">
      <span style="width:36px;height:36px;border-radius:9px;background:{t[2]};color:#fff;display:flex;align-items:center;justify-content:center;font-size:13px;flex-shrink:0">{t[1]}</span>
      <div style="flex:1;min-width:0"><b style="font-size:12px;display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{t[0]}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">{t[3]} · {t[4]} · {t[5]}做种 · {t[6].split("/")[0]}</p>{tags}</div>
      <div style="text-align:right;flex-shrink:0"><b style="font-size:12px;color:#2FA8FF">{t[5]}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">做种</p></div>
    </div>'''

def chip(t, on=False):
    return f'<span style="background:{"#2FA8FF" if on else "#F0F4F8"};color:{"#fff" if on else "#667"};border-radius:14px;padding:5px 13px;font-size:11px">{t}</span>'

out = []
L = out.append

# ===== 全展开 60页 =====
# 1 首页
L(f3_shell("首页", f'''
          <div style="display:grid;grid-template-columns:1.6fr 1fr;gap:14px">
            <div>
              <div style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);border-radius:16px;padding:20px;color:#fff;margin-bottom:12px">
                <h2 style="margin:0;font-size:20px">好学者如春苗，日有所长 🌱</h2>
                <p style="font-size:12px;opacity:.9;margin:6px 0 0">19,434 个优质学习资源 · 4,645 位同学一起学习</p>
                <div style="display:flex;gap:9px;margin-top:12px"><span style="background:rgba(255,255,255,.22);border-radius:10px;padding:7px 16px;font-size:12px">浏览资源</span><span style="background:rgba(255,255,255,.22);border-radius:10px;padding:7px 16px;font-size:12px">发布资源</span></div>
              </div>
              <div style="display:grid;grid-template-columns:repeat(5,1fr);gap:9px;margin-bottom:12px">
                <div style="background:#fff;border-radius:11px;padding:11px;text-align:center"><b style="font-size:15px">108.97T</b><p style="color:#99a;font-size:9px;margin:1px 0 0">上传</p></div>
                <div style="background:#fff;border-radius:11px;padding:11px;text-align:center"><b style="font-size:15px">2.83T</b><p style="color:#99a;font-size:9px;margin:1px 0 0">下载</p></div>
                <div style="background:#fff;border-radius:11px;padding:11px;text-align:center"><b style="font-size:15px">38.5</b><p style="color:#99a;font-size:9px;margin:1px 0 0">分享率</p></div>
                <div style="background:#fff;border-radius:11px;padding:11px;text-align:center"><b style="font-size:15px">85.8M</b><p style="color:#99a;font-size:9px;margin:1px 0 0">火花</p></div>
                <div style="background:#fff;border-radius:11px;padding:11px;text-align:center"><b style="font-size:15px">14,247</b><p style="color:#99a;font-size:9px;margin:1px 0 0">做种中</p></div>
              </div>
              <div style="background:#fff;border-radius:12px;padding:14px"><div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:9px"><b style="font-size:13px">🔥 最新资源</b><span style="color:#99a;font-size:10px">查看全部 ›</span></div><div style="display:grid;gap:7px">{"".join(f3_tor(t) for t in TORRENTS[:3])}</div></div>
            </div>
            <div style="display:grid;gap:12px;align-content:start">
              <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📢 公告</b><div style="display:grid;gap:7px;margin-top:8px">{"".join(f'<div style="border:1px solid #EEF2F7;border-radius:9px;padding:8px 10px;font-size:11px"><b>{nm}</b><p style="color:#99a;font-size:9px;margin:3px 0 0">{t} · {ds}</p></div>' for nm,_,t,ds in NEWS[:3])}</div></div>
              <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📋 我的数据</b><div style="display:grid;grid-template-columns:1fr 1fr;gap:7px;margin-top:8px">{"".join(f'<div style="background:#F0F7FF;border-radius:9px;padding:8px;text-align:center;font-size:10px"><b>{v}</b><p style="color:#99a;font-size:8px;margin:1px 0 0">{nm}</p></div>' for _,nm,v in MYMENU[:6])}</div></div>
            </div>
          </div>
''', "E147 · 三折叠 首页工作台"))

# 2 浏览 ×4
for ci, cn in enumerate(["全部","小学","初中","高中"]):
    L(f3_shell("种子浏览", f'''
          <div style="display:flex;gap:8px;margin-bottom:12px;flex-wrap:wrap">{"".join(chip(c, on=(c==cn)) for c,_ in [("全部",""),("学前教育",""),("小学",""),("初中",""),("职高",""),("高中",""),("教育影音",""),("纪录片","")])}</div>
          <div style="display:grid;grid-template-columns:repeat(2,1fr);gap:10px">{"".join(f3_tor(t) for t in TORRENTS[ci*4:(ci+1)*4])}</div>
          <div style="padding:14px;text-align:center;color:#99a;font-size:12px">第 1 / 486 页 · 共 19,434 个种子 · 排序 最新</div>
''', f"E14{8+ci} · 三折叠 浏览-{cn}"))

# 3 详情 ×6
for ti, t in enumerate(TORRENTS[:6]):
    L(f3_shell("种子详情", f'''
          <div style="display:grid;grid-template-columns:1.5fr 1fr;gap:14px">
            <div>
              <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px">
                <div style="display:flex;gap:9px;align-items:flex-start"><b style="font-size:15px;flex:1;line-height:1.5">{t[0]}</b><span class="tag tag-free">Free</span></div>
                <p style="color:#99a;font-size:10px;margin:6px 0 0">发布者 gntv · 发布员 · 2026-09-06 · 副标题：教材/章节/栏目</p>
                <div style="display:flex;gap:6px;margin-top:8px;flex-wrap:wrap">{"".join(f'<span style="background:#E8F4FF;color:#2FA8FF;border-radius:8px;padding:2px 8px;font-size:10px">{x}</span>' for x in t[6].split("/"))}</div>
              </div>
              <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:12px;color:#8895aa">基本信息</b><div style="display:grid;grid-template-columns:1fr 1fr;gap:6px;margin-top:8px;font-size:12px"><span style="display:flex;justify-content:space-between;padding:5px 8px;background:#F8FAFC;border-radius:7px"><span style="color:#99a">大小</span><b>{t[4]}</b></span><span style="display:flex;justify-content:space-between;padding:5px 8px;background:#F8FAFC;border-radius:7px"><span style="color:#99a">类型</span><b>{t[3]}</b></span><span style="display:flex;justify-content:space-between;padding:5px 8px;background:#F8FAFC;border-radius:7px"><span style="color:#99a">媒介</span><b>书籍</b></span><span style="display:flex;justify-content:space-between;padding:5px 8px;background:#F8FAFC;border-radius:7px"><span style="color:#99a">格式</span><b>EPUB</b></span><span style="display:flex;justify-content:space-between;padding:5px 8px;background:#F8FAFC;border-radius:7px"><span style="color:#99a">年份</span><b>2026</b></span><span style="display:flex;justify-content:space-between;padding:5px 8px;background:#F8FAFC;border-radius:7px"><span style="color:#99a">制作组</span><b>HX</b></span></div></div>
              <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:12px;color:#8895aa">简介</b><p style="color:#667;font-size:12px;line-height:1.9;margin:8px 0 0">{t[0]}。本站提供简繁双版及扫描页资源，适合古文学习与研究使用，感谢支持好学PT！</p></div>
            </div>
            <div>
              <div style="display:grid;grid-template-columns:1fr 1fr;gap:9px;margin-bottom:10px"><div style="background:#F0F7FF;border-radius:11px;padding:12px;text-align:center"><b style="font-size:18px;color:#2FA8FF">{t[5]}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">做种</p></div><div style="background:#F0F7FF;border-radius:11px;padding:12px;text-align:center"><b style="font-size:18px;color:#2FBF9B">3</b><p style="color:#99a;font-size:10px;margin:2px 0 0">下载</p></div></div>
              <button class="f3-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px;margin-bottom:9px">⬇️ 下载种子</button>
              <div style="background:#fff;border-radius:11px;padding:11px;font-size:11px;color:#667;margin-bottom:9px"><b>📎 种子链接</b><p style="color:#2FA8FF;margin:4px 0 0;font-size:10px">https://www.hxpt.org/download.php?id=19605</p></div>
              <div style="background:#fff;border-radius:11px;padding:11px"><b style="font-size:12px;color:#8895aa">📁 文件列表（48）</b><div style="display:grid;gap:5px;margin-top:7px;font-size:11px"><div style="display:flex;justify-content:space-between"><span>📄 六書正譌.txt</span><span style="color:#99a">128MB</span></div><div style="display:flex;justify-content:space-between"><span>📄 六書正譌.pdf</span><span style="color:#99a">96MB</span></div><div style="display:flex;justify-content:space-between"><span>📁 扫描页/</span><span style="color:#99a">44文件</span></div></div></div>
            </div>
          </div>
''', f"E15{2+ti} · 三折叠 详情-{t[1]}"))

# 4 论坛 ×4
for fi, (fn, subs, c) in enumerate(FORUMS):
    L(f3_shell("论坛", f'''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:14px;color:{c}">{fn}</b><div style="display:grid;gap:8px;margin-top:10px">{"".join(f'<div style="border:1px solid #EEF2F7;border-radius:10px;padding:10px 12px;font-size:12px;display:flex;justify-content:space-between"><b>{s}</b><span style="color:#99a">{cnt}</span></div>' for s,cnt in subs)}</div></div>
            <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px;color:#8895aa">📌 最新主题</b><div style="display:grid;gap:8px;margin-top:10px">
              <div style="border:1px solid #EEF2F7;border-radius:10px;padding:10px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>IYUU辅种保姆级教程</b><span style="color:#99a;font-size:10px">356 阅</span></div><p style="color:#99a;font-size:10px;margin:4px 0 0">gntv · 09-06 · 💬 23</p></div>
              <div style="border:1px solid #EEF2F7;border-radius:10px;padding:10px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>新人必读：站点规则</b><span style="color:#99a;font-size:10px">892 阅</span></div><p style="color:#99a;font-size:10px;margin:4px 0 0">管理组 · 09-05 · 💬 45</p></div>
            </div></div>
          </div>
''', f"E15{8+fi} · 三折叠 论坛-{fn[:3]}"))

# 5 课本 ×4
for gi, (gn, subs) in enumerate([("小学", SUBJECTS[:8]),("初中", SUBJECTS[4:12]),("高中", SUBJECTS[10:18]),("全学段", SUBJECTS[14:])]):
    L(f3_shell("课本中心", f'''
          <div style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);border-radius:14px;padding:16px;color:#fff;margin-bottom:12px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:17px">📚 课本中心 · {gn}</b><p style="font-size:11px;opacity:.9;margin:3px 0 0">小学、初中、高中课本资源一键下载</p></div><div style="display:flex;gap:8px"><span style="background:rgba(255,255,255,.2);border-radius:9px;padding:7px 13px;font-size:11px">📋 我的提交</span><span style="background:#fff;color:#2FA8FF;border-radius:9px;padding:7px 13px;font-size:11px">➕ 创建课本</span></div></div>
          <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px">{"".join(f'<div style="background:#fff;border-radius:12px;padding:14px;text-align:center"><span style="font-size:26px">📘</span><b style="display:block;font-size:13px;margin-top:5px">{s}</b><span style="color:#99a;font-size:10px">人教版 · {12+gi*9}本</span><div style="margin-top:6px;font-size:10px;color:#2FA8FF">查看 ›</div></div>' for s in subs)}</div>
''', f"E16{2+gi} · 三折叠 课本-{gn}"))

# 6 勋章
L(f3_shell("勋章墙", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:12px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:14px">我的收藏</b><p style="color:#99a;font-size:11px;margin:2px 0 0">已收集 30 / 81 · 37% 完成</p></div><div style="width:240px;height:10px;background:#F0F4F8;border-radius:6px;overflow:hidden"><div style="width:37%;height:100%;background:linear-gradient(90deg,#2FA8FF,#FFC93C);border-radius:6px"></div></div></div>
          <div style="display:grid;grid-template-columns:repeat(6,1fr);gap:10px">{"".join(f'<div style="background:#fff;border-radius:12px;padding:12px;text-align:center;{"border:2px solid #E8F4FF" if i<4 else "opacity:.4"}"><span style="font-size:26px">{m}</span><b style="display:block;font-size:11px;margin-top:4px">{nm}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{ds}</p></div>' for i,(m,nm,ds) in enumerate(MEDALS))}</div>
          <div style="display:flex;gap:8px;margin-top:12px;font-size:11px;flex-wrap:wrap"><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px">二十四节气</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px">学习课堂</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px">节日系列</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px">开站系列</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px">工作组</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px">未分类</span></div>
''', "E166 · 三折叠 勋章墙"))

# 7 排行 ×4
for ri, (rn, col) in enumerate([("上传者","上传量"),("下载者","下载量"),("做种","做种数"),("分享率","分享率")]):
    rows = "".join(f'<div style="display:flex;align-items:center;gap:10px;padding:9px 11px;background:{"linear-gradient(135deg,#FFF8E8,#FFF4D6)" if i==0 else "#fff"};border-radius:10px"><span style="width:22px;text-align:center;font-weight:700;color:{"#FFC93C" if i<3 else "#99a"}">{i+1}</span><img src="{OWL}" style="width:30px;height:30px;border-radius:50%"><div style="flex:1"><b style="font-size:12px">{u}</b><p style="color:#99a;font-size:9px;margin:0">{cl}</p></div><b style="font-size:12px;color:{"#FF7A59" if i==0 else "#1F2A44"}">{v}</b></div>' for i,(u,cl,v,_) in enumerate(RANKS))
    L(f3_shell("排行榜", f'''
          <div style="display:flex;gap:8px;margin-bottom:12px;flex-wrap:wrap">{"".join(chip(x, on=(x==rn)) for x in ["上传者","下载者","做种","分享率"])}<span style="margin-left:auto;background:#F0F4F8;color:#667;border-radius:14px;padding:5px 12px;font-size:11px">范围 Top 21</span></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:10px"><div style="display:grid;gap:8px">{rows}</div>
          <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📊 {col}统计</b><div style="display:grid;gap:8px;margin-top:10px;font-size:12px"><div style="display:flex;justify-content:space-between;padding:8px 0;border-bottom:1px solid #F0F4F8"><span style="color:#99a">TOP1 {col}</span><b>109.697 TB</b></div><div style="display:flex;justify-content:space-between;padding:8px 0;border-bottom:1px solid #F0F4F8"><span style="color:#99a">TOP5 均值</span><b>90.2 TB</b></div><div style="display:flex;justify-content:space-between;padding:8px 0"><span style="color:#99a">上榜门槛</span><b>62.1 TB</b></div></div></div></div>
''', f"E16{7+ri} · 三折叠 排行-{rn}"))

# 8 站免池
L(f3_shell("站免池", f'''
          <div style="background:linear-gradient(135deg,#FF7A59,#FFC93C);border-radius:16px;padding:20px;color:#fff;margin-bottom:12px"><div style="display:flex;justify-content:space-between;align-items:center"><b style="font-size:18px">🎉 站免池 · 大家一起冲</b><span style="background:rgba(255,255,255,.25);border-radius:14px;padding:4px 12px;font-size:11px">距双倍免费还差 954,000</span></div><div style="margin-top:12px"><div style="display:flex;justify-content:space-between;font-size:12px;margin-bottom:5px"><span>1,046,000 / 2,000,000 火花</span><b>52.3%</b></div><div style="height:10px;background:rgba(255,255,255,.3);border-radius:6px;overflow:hidden"><div style="width:52.3%;height:100%;background:#fff;border-radius:6px"></div></div></div><p style="font-size:11px;opacity:.95;margin:9px 0 0">当月达 200 万 → 下月 1-3 号自动开启全局双倍免费促销 🚀</p></div>
          <div style="display:grid;grid-template-columns:repeat(7,1fr);gap:9px;margin-bottom:12px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><b style="font-size:13px;color:#FF7A59">{t}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">火花</p></div>' for t in POOL_TIERS)}</div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px"><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:12px;color:#8895aa">📜 最近捐赠</b><div style="display:grid;gap:5px;margin-top:8px;font-size:11px"><div style="display:flex;justify-content:space-between"><span>gntv</span><b style="color:#FF7A59">+1,000,000</b></div><div style="display:flex;justify-content:space-between"><span>dgvge</span><b style="color:#FF7A59">+500,000</b></div><div style="display:flex;justify-content:space-between"><span>kiririn</span><b style="color:#FF7A59">+100,000</b></div></div></div><div style="background:#FFF8E8;border-radius:12px;padding:14px;font-size:12px;color:#667;line-height:1.9"><b>💡 规则：</b>捐赠火花进入站免池，当月达 200 万，下月 1-3 号全站种子免费且双倍上传！一起冲鸭 🌱</div></div>
''', "E171 · 三折叠 站免池"))

# 9 银行 ×3
for bi, bt in enumerate(["总览","定期存款","贷款"]):
    if bt == "总览":
        body = f'''<div style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);border-radius:16px;padding:20px;color:#fff;margin-bottom:12px"><p style="font-size:12px;opacity:.9;margin:0">总资产</p><b style="font-size:28px">85,814,940.71</b><p style="font-size:11px;opacity:.9;margin:6px 0 0">时魔每小时 +469.144 · 最大可贷 46,924.38</p></div>
          <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:9px;margin-bottom:12px"><div style="background:#fff;border-radius:11px;padding:12px"><p style="color:#99a;font-size:10px;margin:0">活期余额</p><b style="font-size:15px">3,321.81</b></div><div style="background:#fff;border-radius:11px;padding:12px"><p style="color:#99a;font-size:10px;margin:0">在投定期</p><b style="font-size:15px">0</b></div><div style="background:#fff;border-radius:11px;padding:12px"><p style="color:#99a;font-size:10px;margin:0">贷款负债</p><b style="font-size:15px">0</b></div><div style="background:#fff;border-radius:11px;padding:12px"><p style="color:#99a;font-size:10px;margin:0">站内余额</p><b style="font-size:15px">85,811,618.9</b></div></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px"><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📌 利率表</b>{"".join(f'<div style="display:flex;justify-content:space-between;font-size:11px;padding:6px 0;border-bottom:1px solid #F0F4F8"><span>{r}</span><b style="color:{c}">{v}</b></div>' for r,v,c in RATES)}</div><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📈 近期收益</b><div style="display:flex;align-items:flex-end;gap:9px;height:90px;margin-top:12px"><div style="flex:1;background:#E8F4FF;height:40%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#E8F4FF;height:55%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#E8F4FF;height:48%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#2FA8FF;height:65%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#2FA8FF;height:72%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#2FA8FF;height:60%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#FFC93C;height:85%;border-radius:4px 4px 0 0"></div></div><p style="color:#99a;font-size:9px;margin:6px 0 0;text-align:center">近 7 天活期收益</p></div></div>'''
    else:
        body = f'''<div style="display:grid;grid-template-columns:1fr 1fr;gap:12px"><div style="background:#fff;border-radius:12px;padding:16px"><b style="font-size:14px">{"📈 定期存款" if bt=="定期存款" else "🏦 申请贷款"}</b><div style="display:grid;gap:10px;margin-top:12px"><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">金额</label><input class="gi" placeholder="最小 10,000 火花" style="width:100%;height:38px;font-size:12px"></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">期限</label><div style="display:flex;gap:6px;flex-wrap:wrap">{"".join(chip(r.replace(" 定期",""), on=(i==1)) for i,(r,v,c) in enumerate(RATES[1:]))}</div></div></div><button class="f3-btn" style="background:{"#5B6BF5" if bt=="定期存款" else "#FF7A59"};color:#fff;border:none;width:100%;margin-top:12px;padding:11px;border-radius:10px;font-size:13px">确认存入</button></div>
        <div style="display:grid;gap:10px"><div style="background:#FFF8E8;border-radius:12px;padding:14px;font-size:12px;color:#667;line-height:1.9"><b>💡 说明</b><br>{"最小存款 10,000 火花；提前支取手续费 1%；到期自动续存。" if bt=="定期存款" else "贷款利息：7天0.08% / 30天0.12% / 90天0.18% / 180天0.2% / 365天0.22%；贷款需按时还款，逾期将影响信誉。"}</div><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:12px;color:#8895aa">📋 交易记录</b><div style="display:grid;gap:5px;margin-top:8px;font-size:11px"><div style="display:flex;justify-content:space-between"><span>活期结息</span><b style="color:#2FBF9B">+4.69</b></div><div style="display:flex;justify-content:space-between"><span>存入定期</span><b>0</b></div></div></div></div></div>'''
    L(f3_shell("火花银行", f'<div style="display:flex;gap:8px;margin-bottom:12px">{"".join(chip(x, on=(x==bt)) for x in ["总览","定期存款","贷款"])}</div>{body}', f"E17{2+bi} · 三折叠 银行-{bt}"))

# 10 农场 ×2
L(f3_shell("好学农场", f'''
          <div style="display:flex;gap:8px;margin-bottom:12px">{"".join(chip(x, on=(i==0)) for i,x in enumerate(["🌾 农作物","🐔 动物","🥬 菜市场"]))}</div>
          <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px;margin-bottom:12px">{"".join(f'<div style="background:#fff;border-radius:12px;padding:14px;text-align:center;{"border:2px solid #FFF4E0" if i==0 else ""}"><span style="font-size:30px">{ic}</span><b style="display:block;font-size:13px;margin-top:5px">{nm}</b><p style="color:#99a;font-size:10px;margin:2px 0">{tm} · {"生长中2h" if i==0 else "空闲"}</p><b style="color:#FF7A59;font-size:12px">{v} 火花</b></div>' for i,(ic,nm,tm,v) in enumerate(CROPS))}</div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px"><div style="background:#FFF8E8;border-radius:12px;padding:14px;font-size:12px;color:#667;line-height:1.9"><b>💡 农场规则：</b><br>作物有效期 5 天，收获有 20% 概率双倍；菜市场每日 0/4/8/12/16/20 点刷新，价格波动 ±50%</div><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📅 种植日志</b><div style="display:grid;gap:5px;margin-top:8px;font-size:11px;color:#667"><div style="display:flex;justify-content:space-between"><span>🌾 小麦成熟</span><b style="color:#2FBF9B">+500</b></div><div style="display:flex;justify-content:space-between"><span>🥔 土豆双倍</span><b style="color:#2FBF9B">+4,000</b></div><div style="display:flex;justify-content:space-between"><span>🥜 花生出售</span><b style="color:#2FBF9B">+1,420</b></div></div></div></div>
''', "E175 · 三折叠 农场"))
L(f3_shell("好学农场", f'''
          <div style="display:flex;gap:8px;margin-bottom:12px">{"".join(chip(x, on=(i==1)) for i,x in enumerate(["🌾 农作物","🐔 动物","🥬 菜市场"]))}</div>
          <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px;margin-bottom:12px">{"".join(f'<div style="background:#fff;border-radius:12px;padding:14px;text-align:center;{"border:2px solid #E8F8F2" if i==0 else ""}"><span style="font-size:30px">{ic}</span><b style="display:block;font-size:13px;margin-top:5px">{nm}</b><p style="color:#99a;font-size:10px;margin:2px 0">{tm} · {"已成熟🎉" if i==0 else "空闲"}</p><b style="color:#FF7A59;font-size:12px">{v} 火花</b></div>' for i,(ic,nm,tm,v) in enumerate(ANIMALS))}</div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px"><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">🐔 我的动物</b><div style="display:grid;gap:6px;margin-top:8px;font-size:12px"><div style="display:flex;justify-content:space-between;border:1px solid #EEF2F7;border-radius:9px;padding:8px 10px"><span>🐔 鸡 ×2</span><span style="color:#2FBF9B">已成熟</span></div><div style="display:flex;justify-content:space-between;border:1px solid #EEF2F7;border-radius:9px;padding:8px 10px"><span>🐷 猪 ×1</span><span style="color:#99a">24h 后</span></div></div></div><div style="background:#FFF8E8;border-radius:12px;padding:14px;font-size:12px;color:#667;line-height:1.9"><b>💡 动物提示：</b><br>动物收获后可前往菜市场出售换取火花，不同动物产出不同哦～</div></div>
''', "E176 · 三折叠 农场-动物"))

# 11 菜市场
L(f3_shell("菜市场", f'''
          <div style="background:linear-gradient(135deg,#2FBF9B,#FFC93C);border-radius:14px;padding:16px;color:#fff;margin-bottom:12px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:16px">🥬 今日菜市场</b><p style="font-size:11px;opacity:.9;margin:3px 0 0">下次刷新 12:00 · 价格波动 ±50%</p></div><span style="background:rgba(255,255,255,.25);border-radius:10px;padding:6px 14px;font-size:12px">🔄 刷新</span></div>
          <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px">{"".join(f'<div style="background:#fff;border-radius:12px;padding:14px;text-align:center"><span style="font-size:26px">{ic}</span><b style="display:block;font-size:13px;margin-top:4px">{nm}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">收购价</p><b style="font-size:16px;color:{"#2FBF9B" if i==0 else "#FF7A59"}">{["620 🔥","880","1,420","1,980"][i]}</b></div>' for i,(ic,nm,tm,v) in enumerate(CROPS))}</div>
          <div style="background:#E8F8F2;border-radius:12px;padding:12px 14px;margin-top:12px;font-size:12px;color:#2FBF9B">📈 小麦今日高价 620（+24%），趁早出售！我的库存：小麦×12 · 玉米×8</div>
''', "E177 · 三折叠 菜市场"))

# 12 商店 ×3
for si, (st, items) in enumerate([("全部", SHOP),("流量", SHOP[:6]),("个性", SHOP[6:])]):
    L(f3_shell("火花商店", f'''
          <div style="display:flex;gap:8px;margin-bottom:12px">{"".join(chip(x, on=(x==st)) for x in ["全部","流量","个性"])}</div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:10px">{"".join(f'<div style="background:#fff;border-radius:12px;padding:14px"><div style="display:flex;justify-content:space-between;align-items:center"><span style="width:36px;height:36px;border-radius:10px;background:{"#E8F4FF" if "上传" in nm or "下载" in nm else "#FFE8F5"};display:flex;align-items:center;justify-content:center;font-size:16px">{"📤" if "上传" in nm else "📥" if "下载" in nm else "🎁"}</span><span style="background:#F0F4F8;color:#99a;border-radius:9px;padding:2px 8px;font-size:9px">{cat}</span></div><b style="display:block;font-size:13px;margin-top:9px">{nm}</b><div style="display:flex;justify-content:space-between;align-items:center;margin-top:8px"><b style="color:#FF7A59;font-size:13px">{pr}</b><span style="background:#2FA8FF;color:#fff;border-radius:9px;padding:4px 12px;font-size:11px">购买</span></div></div>' for nm,pr,cat in items)}</div>
''', f"E17{8+si} · 三折叠 商店-{st}"))

# 13 任务
L(f3_shell("任务中心", f'''
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:10px;margin-bottom:12px">{"".join(f'<div style="background:#fff;border-radius:12px;padding:13px;display:flex;gap:9px;align-items:center"><span style="font-size:22px">{ic}</span><div style="flex:1"><b style="font-size:12px">{nm}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">奖励 {rw}</p></div><span style="background:{c}20;color:{c};border-radius:9px;padding:3px 8px;font-size:9px">{st}</span></div>' for ic,nm,rw,st,c in TASKS)}</div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px"><div style="background:linear-gradient(135deg,#E8F4FF,#E8F0FF);border-radius:12px;padding:14px"><b style="font-size:14px">📋 绩效考核（保种员 5T 版）</b><p style="color:#99a;font-size:11px;margin:3px 0 0">月领 200,000 火花</p><div style="display:flex;gap:10px;margin-top:9px"><div style="flex:1;background:#fff;border-radius:9px;padding:9px;text-align:center"><b style="font-size:15px;color:#2FA8FF">28/30</b><p style="color:#99a;font-size:9px;margin:1px 0 0">操作总数</p></div><div style="flex:1;background:#fff;border-radius:9px;padding:9px;text-align:center"><b style="font-size:15px;color:#2FA8FF">25/30</b><p style="color:#99a;font-size:9px;margin:1px 0 0">通过审核</p></div></div></div><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">🏅 领取历史</b><div style="display:grid;gap:5px;margin-top:8px;font-size:11px;color:#667"><div style="display:flex;justify-content:space-between"><span>gntv</span><b style="color:#2FBF9B">200,000</b></div><div style="display:flex;justify-content:space-between"><span>冷冷的风</span><b style="color:#2FBF9B">200,000</b></div><div style="display:flex;justify-content:space-between"><span>alan5914</span><b style="color:#2FBF9B">200,000</b></div></div></div></div>
''', "E181 · 三折叠 任务中心"))

# 14 签到
L(f3_shell("签到", f'''
          <div style="background:linear-gradient(135deg,#2FBF9B,#2FA8FF);border-radius:16px;padding:18px;color:#fff;margin-bottom:12px;display:flex;justify-content:space-between;align-items:center"><div><p style="font-size:11px;opacity:.9;margin:0">已连续签到</p><b style="font-size:28px">120 天</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">累计获得 42,000 火花</p></div><div style="background:rgba(255,255,255,.2);border-radius:14px;padding:12px 16px;text-align:center"><span style="font-size:24px">✅</span><p style="font-size:10px;margin:2px 0 0">今日已签</p></div></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px"><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📅 九月签到</b><div style="display:grid;grid-template-columns:repeat(7,1fr);gap:6px;margin-top:10px;text-align:center;font-size:10px"><span style="color:#99a">一</span><span style="color:#99a">二</span><span style="color:#99a">三</span><span style="color:#99a">四</span><span style="color:#99a">五</span><span style="color:#99a">六</span><span style="color:#99a">日</span><span></span><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">1</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">2</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">3</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">4</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">5</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">6</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">7</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">8</div><div style="background:#E8F8F2;color:#2FBF9B;border-radius:7px;padding:3px 0">9</div><div style="background:#2FBF9B;color:#fff;border-radius:7px;padding:3px 0;font-weight:700">10</div></div></div><div style="background:#FFF8E8;border-radius:12px;padding:14px;font-size:12px;color:#667;line-height:2"><b>🎁 连续奖励：</b><br>10天 200火花<br>20天 500火花<br>30天 1,000火花<br>60天 3,000火花<br>100天 10,000+勋章<br>365天 50,000+年限勋章</div></div>
''', "E182 · 三折叠 签到"))

# 15 消息
L(f3_shell("消息中心", f'''
          <div style="display:grid;grid-template-columns:240px 1fr;gap:12px">
            <div style="background:#fff;border-radius:12px;padding:10px;display:grid;gap:3px;align-content:start;font-size:13px"><div style="background:#E8F4FF;color:#2FA8FF;border-radius:9px;padding:10px 12px;font-weight:600">📥 收件箱 <span style="float:right">5</span></div><div style="padding:10px 12px;color:#667">📤 发件箱</div><div style="padding:10px 12px;color:#667">🗑️ 回收站</div><div style="padding:10px 12px;color:#667">📝 草稿箱</div><div style="padding:10px 12px;color:#667">⚙️ 设置</div></div>
            <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📥 收件箱</b><div style="display:grid;gap:8px;margin-top:10px">{"".join(f'<div style="border:1px solid #EEF2F7;border-radius:10px;padding:10px 12px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>{f}</b><span style="color:#99a;font-size:10px">{t}</span></div><p style="color:#667;font-size:12px;margin:5px 0 0;line-height:1.6">{m}</p></div>' for f,m,t in MSGS)}</div></div>
          </div>
''', "E183 · 三折叠 消息中心"))

# 16 我的数据
L(f3_shell("我的数据", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:12px;display:flex;align-items:center;gap:12px"><img src="{OWL}" style="width:48px;height:48px;border-radius:50%"><div><b style="font-size:16px">gntv</b><p style="color:#99a;font-size:11px;margin:2px 0 0">发布员 · 加入 2025-07-16 · 邮箱 95836184@qq.com</p></div><div style="flex:1;text-align:right"><span style="background:#FFE8F5;color:#FF8FC7;border-radius:10px;padding:4px 12px;font-size:11px">🎖️ 30/81</span></div></div>
          <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px;margin-bottom:12px"><div style="background:#F0F7FF;border-radius:11px;padding:12px;text-align:center"><b style="font-size:17px;color:#2FA8FF">108.97T</b><p style="color:#99a;font-size:10px;margin:1px 0 0">上传</p></div><div style="background:#F0F7FF;border-radius:11px;padding:12px;text-align:center"><b style="font-size:17px;color:#2FBF9B">2.83T</b><p style="color:#99a;font-size:10px;margin:1px 0 0">下载</p></div><div style="background:#F0F7FF;border-radius:11px;padding:12px;text-align:center"><b style="font-size:17px">38.5</b><p style="color:#99a;font-size:10px;margin:1px 0 0">分享率</p></div><div style="background:#F0F7FF;border-radius:11px;padding:12px;text-align:center"><b style="font-size:17px;color:#FFC93C">562.8/h</b><p style="color:#99a;font-size:10px;margin:1px 0 0">火花收益</p></div></div>
          <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:13px"><b style="font-size:13px">{ic} {nm}</b><p style="color:#99a;font-size:10px;margin:3px 0 0">{v}</p></div>' for ic,nm,v in MYMENU)}</div>
''', "E184 · 三折叠 我的数据"))

# 17 火花收益
L(f3_shell("我的火花收益", f'''
          <div style="display:grid;grid-template-columns:1.2fr 1fr;gap:14px">
            <div><div style="background:linear-gradient(135deg,#FFC93C,#FF7A59);border-radius:16px;padding:18px;color:#fff;margin-bottom:10px"><p style="font-size:11px;opacity:.9;margin:0">当前火花</p><b style="font-size:30px">85,815,181.6</b><p style="font-size:11px;opacity:.9;margin:5px 0 0">每小时收益 562.845</p></div>
            <div style="display:grid;grid-template-columns:1fr 1fr;gap:9px"><div style="background:#fff;border-radius:11px;padding:12px"><p style="color:#99a;font-size:10px;margin:0">基本奖励</p><b style="font-size:15px">74.2</b></div><div style="background:#fff;border-radius:11px;padding:12px"><p style="color:#99a;font-size:10px;margin:0">勋章加成</p><b style="font-size:15px">69.215</b><p style="color:#99a;font-size:9px;margin:0">1.03x</p></div><div style="background:#fff;border-radius:11px;padding:12px"><p style="color:#99a;font-size:10px;margin:0">官种加成</p><b style="font-size:15px">325.750</b><p style="color:#99a;font-size:9px;margin:0">5x</p></div><div style="background:#fff;border-radius:11px;padding:12px"><p style="color:#99a;font-size:10px;margin:0">后宫加成</p><b style="font-size:15px">93.680</b><p style="color:#99a;font-size:9px;margin:0">0.1x</p></div></div></div>
            <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📈 近 7 天收益趋势</b><div style="display:flex;align-items:flex-end;gap:10px;height:110px;margin-top:14px"><div style="flex:1;background:#E8F4FF;height:30%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#E8F4FF;height:45%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#E8F4FF;height:40%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#2FA8FF;height:60%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#2FA8FF;height:55%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#2FA8FF;height:70%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#FFC93C;height:85%;border-radius:4px 4px 0 0"></div></div><div style="display:flex;gap:10px;margin-top:5px;font-size:9px;color:#99a"><span style="flex:1;text-align:center">一</span><span style="flex:1;text-align:center">二</span><span style="flex:1;text-align:center">三</span><span style="flex:1;text-align:center">四</span><span style="flex:1;text-align:center">五</span><span style="flex:1;text-align:center">六</span><span style="flex:1;text-align:center">日</span></div><div style="border-top:1px solid #F0F4F8;margin-top:12px;padding-top:10px;font-size:12px;color:#667"><b>💡 提升收益：</b>多做种、上传官种、集齐勋章</div></div>
          </div>
''', "E185 · 三折叠 火花收益"))

# 18-30: 信息卡页面 ×13
def info3(title, label, items, cols=3):
    cards = "".join(f'<div style="background:#fff;border-radius:12px;padding:14px;display:flex;gap:10px;align-items:center"><span style="width:36px;height:36px;border-radius:10px;background:#F0F7FF;display:flex;align-items:center;justify-content:center;font-size:16px;flex-shrink:0">{ic}</span><div><b style="font-size:12px">{a}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">{b}</p></div></div>' for ic,a,b in items)
    return f3_shell(title, f'<div style="display:grid;grid-template-columns:repeat({cols},1fr);gap:10px">{cards}</div>', label)

L(info3("邀请", "E186 · 三折叠 邀请", [("🎁","我的邀请名额","1120 个 · 已用 0"),("📨","已发送邀请","3 封"),("⏳","待接受","1 封")]))
L(info3("求种", "E187 · 三折叠 求种", [("🙋","小学数学思维训练","进行中 · 3人已求"),("🙋","高中生物实验 4K","进行中 · 2人已求"),("✅","物理竞赛教程","已解决 09-01")]))
L(info3("字幕", "E188 · 三折叠 字幕", [("🎬","蓝色星球 E01 中英","ASS · 09-06"),("🎬","小猪佩奇 S1","SRT · 09-05"),("➕","上传字幕","发布字幕赚火花")]))
L(info3("站点规则", "E189 · 三折叠 规则", [("📜","第一章 总则","站点宗旨与定位"),("👤","第二章 账号","注册邀请等级"),("📊","第三章 数据","分享率计算"),("📤","第四章 发布","发布规范奖励"),("⬇️","第五章 下载","下载保种义务"),("🚫","第六章 违规","违规与处罚")], cols=3))
L(info3("帮助 FAQ", "E190 · 三折叠 帮助FAQ", [("🏠","站点信息","什么是好学PT"),("👤","用户信息","资料修改找回"),("📊","数据统计","分享率如何算"),("📤","发布","如何发布种子"),("⬇️","下载","下载慢怎么办"),("🌐","网络问题","Tracker排查")], cols=3))
L(info3("工具箱", "E191 · 三折叠 工具箱", [(ic,nm,ds) for ic,nm,ds in TOOLS], cols=3))
L(info3("管理组", "E192 · 三折叠 管理组", [("🎧","一线客服","10人 · 可申请"),("💬","批评家","5人 · 可申请"),("🛡️","论坛版主","8人 · 可申请"),("⚙️","管理员","6人 · 可申请"),("👑","VIP","45人 · 可申请")], cols=5))
L(info3("保种区", "E193 · 三折叠 保种区", [("🛡️","官方保种","≥5天且做种"),("🛡️","全部保种","所有≥5天做种"),("💡","机制",">7天移出免费3天"),("📂","分类","教育/高中/高职/初中/小学")], cols=4))
L(info3("官种", "E194 · 三折叠 官种", [(t[0][:12], f"{t[1]} · {t[2]}", "") for t in OFFICIALS], cols=3))
L(info3("娱乐市场", "E195 · 三折叠 娱乐市场", [(ic,nm,ds) for ic,nm,ds,_ in GAMES], cols=3))
L(info3("我的收藏", "E196 · 三折叠 收藏", [("⭐","识典古籍 六書正譌","收藏 09-06"),("⭐","蓝色星球 第三季","收藏 09-03"),("⭐","高中英语词汇 3500","收藏 08-28")]))
L(info3("PT人生", "E197 · 三折叠 PT人生", [("🎯","我的等级","发布员·下一级主管"),("📈","成长轨迹","加入342天"),("🏅","成就解锁","12/40"),("🎮","游戏记录","五子棋18胜12负"),("💎","总数据","上传108.97T")], cols=5))
L(info3("公告中心", "E198 · 三折叠 公告", [(ic,nm,f"{t} · {ds}") for ic,nm,t,ds in NEWS[:6]], cols=3))
L(info3("站点事件", "E199 · 三折叠 事件", [(f"📋","{t} {nm}",ds) for t,nm,_,ds in EVENTS[:6]], cols=3))
L(info3("站点统计", "E200 · 三折叠 统计", [("🧲","种子","19,434"),("📦","总大小","85.55TB"),("🔄","同伴","142,384"),("👥","用户","4,645"),("📤","总上传","4.196PB"),("📥","总下载","467.77TB")], cols=3))
L(info3("在线用户", "E201 · 三折叠 在线", [("🟢","在线 2,226","24小时活跃"),("🔵","下载中 282","正在下载"),("🔴","做种 142,102","正在做种")]))
L(info3("最新上传", "E202 · 三折叠 最新上传", [(t[1], t[0][:18], f"{t[3]} · {t[4]}") for t in TORRENTS[:6]], cols=3))
L(info3("热门种子", "E203 · 三折叠 热门", [("🔥", t[0][:16], f"{t[5]}做种 · {t[4]}") for t in TORRENTS[:6]], cols=3))
L(info3("断种", "E204 · 三折叠 断种", [("⚠️","蓝色星球 S2 4K","0做种 · 142GB"),("⚠️","三年级英语 旧版","0做种 · 1.2GB"),("⚠️","高中数学选修 旧","0做种 · 3.4GB")]))
L(info3("免费种子", "E205 · 三折叠 免费", [("🎉", t[0][:16], "免费") for t in TORRENTS[:6]], cols=3))
L(info3("标签云", "E206 · 三折叠 标签", [("🏷️", nm, f"{v} 个种子") for nm,v in [("Free","1,203"),("保种","856"),("官方","1,234"),("古籍","189"),("2x","672"),("课本","445")]], cols=3))
L(info3("绩效考核", "E207 · 三折叠 绩效", [("🏆","保种员 5T 版","月领200,000"),("📊","操作总数","28/30"),("✅","通过审核","25/30")]))
L(info3("好友", "E208 · 三折叠 好友", [("👥","alan5914","上传16,348"),("👥","study_mom","上传892"),("👥","kiririn","上传3,204")]))
L(info3("访问日志", "E209 · 三折叠 访问日志", [("🕐","最近访问","识典古籍·10:20"),("🕐","最近访问","蓝色星球·09:45"),("🕐","最近访问","高中数学·昨天")]))
L(info3("举报中心", "E210 · 三折叠 举报", [("🚩","举报规则","如实举报禁滥用"),("🚩","举报记录","3条处理中"),("🆕","发起举报","选择类型对象")]))

# 31 用户详情 ×3
for ui in range(3):
    L(f3_shell("用户详情", f'''
          <div style="display:flex;gap:8px;margin-bottom:12px;flex-wrap:wrap">{"".join(chip(x, on=(i==ui)) for i,x in enumerate(["概览","上传种子","做种种子","下载历史","评论","收藏","好友","勋章"]))}</div>
          <div style="display:grid;grid-template-columns:1fr 1.4fr;gap:14px">
            <div style="background:#fff;border-radius:12px;padding:14px;align-content:start"><img src="{OWL}" style="width:52px;height:52px;border-radius:50%"><b style="display:block;font-size:15px;margin-top:6px">gntv</b><p style="color:#99a;font-size:11px;margin:3px 0 0">发布员 · 加入 2025-07-16</p><div style="display:grid;gap:6px;margin-top:12px;font-size:11px;color:#667"><div style="display:flex;justify-content:space-between;padding:6px 0;border-bottom:1px solid #F0F4F8"><span>邮箱</span><b>95836184@qq.com</b></div><div style="display:flex;justify-content:space-between;padding:6px 0;border-bottom:1px solid #F0F4F8"><span>当前活动</span><b>14,247</b></div><div style="display:flex;justify-content:space-between;padding:6px 0"><span>连接数</span><b>无限制</b></div></div></div>
            <div><div style="display:grid;grid-template-columns:repeat(4,1fr);gap:9px;margin-bottom:10px"><div style="background:#F0F7FF;border-radius:11px;padding:11px;text-align:center"><b style="font-size:16px;color:#2FA8FF">108.97T</b><p style="color:#99a;font-size:9px;margin:1px 0 0">上传</p></div><div style="background:#F0F7FF;border-radius:11px;padding:11px;text-align:center"><b style="font-size:16px;color:#2FBF9B">2.83T</b><p style="color:#99a;font-size:9px;margin:1px 0 0">下载</p></div><div style="background:#F0F7FF;border-radius:11px;padding:11px;text-align:center"><b style="font-size:16px">38.5</b><p style="color:#99a;font-size:9px;margin:1px 0 0">分享率</p></div><div style="background:#F0F7FF;border-radius:11px;padding:11px;text-align:center"><b style="font-size:16px">14,247</b><p style="color:#99a;font-size:9px;margin:1px 0 0">做种中</p></div></div>
            <div style="display:grid;gap:9px">{"".join(f3_tor(t) for t in TORRENTS[:3])}</div></div>
          </div>
''', f"E21{1+ui} · 三折叠 用户详情-{'概览上传做种下载评论收藏好友勋章'.split('')[ui] if False else ['概览','上传种子','做种种子'][ui]}"))

# 32 管理后台 ×6
for pi, (pn, pic, stats, note) in enumerate([
    ("仪表盘","📊",[("🧲","种子","19,434"),("👥","用户","4,645"),("🔄","同伴","142,384"),("⏳","待审","12")], "今日审核 45 · 通过率 71.1%"),
    ("种子管理","🧲",[("⏳","待审核","12"),("🚩","举报","3"),("🏅","官种","1,234"),("📦","总数","19,434")], "最新待审：高中英语词汇 3500"),
    ("用户管理","👥",[("👥","注册","4,645"),("⚠️","未验证","39"),("🚫","被禁","1,341"),("👑","贵宾","45")], "新注册 3 人 · 待晋升 2 人"),
    ("公告管理","📢",[("📢","公告","18"),("📝","草稿","3"),("⏰","定时","1"),("📊","阅读","45,892")], "最新：站免池月度进度通报"),
    ("勋章管理","🎖️",[("🏅","勋章","81"),("🃏","卡牌","45"),("📤","已发","1,208"),("⏳","待审","5")], "最新上架：开学季限定"),
    ("系统管理","🔒",[("⚙️","维护","正常"),("💾","备份","45份"),("🔑","API","正常"),("📋","安全","已开")], "最近备份 09-07 · 12.8GB"),
]):
    L(f3_shell("管理后台", f'''
          <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px;margin-bottom:12px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:13px"><p style="color:#99a;font-size:10px;margin:0">{ic} {nm}</p><b style="font-size:16px">{v}</b></div>' for ic,nm,v in stats)}</div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px"><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📌 {note}</b><div style="display:grid;gap:7px;margin-top:10px;font-size:12px"><div style="display:flex;justify-content:space-between;border:1px solid #EEF2F7;border-radius:9px;padding:9px 11px"><span>待处理事项</span><b style="color:#FF7A59">12</b></div><div style="display:flex;justify-content:space-between;border:1px solid #EEF2F7;border-radius:9px;padding:9px 11px"><span>今日完成</span><b style="color:#2FBF9B">32</b></div></div></div><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📊 近7天趋势</b><div style="display:flex;align-items:flex-end;gap:9px;height:100px;margin-top:12px"><div style="flex:1;background:#E8F4FF;height:35%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#E8F4FF;height:50%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#2FA8FF;height:62%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#2FA8FF;height:48%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#2FA8FF;height:70%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#FFC93C;height:82%;border-radius:4px 4px 0 0"></div><div style="flex:1;background:#FFC93C;height:65%;border-radius:4px 4px 0 0"></div></div></div></div>
''', f"E21{4+pi} · 三折叠 后台-{pn}", dark=True))

# 33 发布类 ×4
L(f3_shell("发布种子", f'''
          <div style="display:grid;grid-template-columns:1.4fr 1fr;gap:14px">
            <div style="background:#fff;border-radius:14px;padding:18px;display:grid;gap:12px"><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">种子文件 *</label><div style="border:2px dashed #2FA8FF;border-radius:11px;padding:16px;text-align:center;color:#2FA8FF;font-size:13px">📎 点击选择 .torrent 文件</div></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">标题 *</label><input class="gi" placeholder="例：识典古籍 六書正譌 简繁双版" style="width:100%;height:38px;font-size:13px"></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">副标题</label><input class="gi" placeholder="教材/章节/栏目/资源" style="width:100%;height:38px;font-size:13px"></div><div style="display:grid;grid-template-columns:1fr 1fr;gap:10px"><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">类型 *</label><select class="gi" style="width:100%;height:38px;font-size:12px"><option>教育</option><option>影音</option><option>纪录片</option></select></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">媒介</label><select class="gi" style="width:100%;height:38px;font-size:12px"><option>书籍</option><option>视频</option><option>音频</option></select></div></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">简介 *</label><textarea class="gi" placeholder="资源介绍…" style="width:100%;height:64px;font-size:12px;resize:none"></textarea></div></div>
            <div style="display:grid;gap:10px;align-content:start"><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:12px;color:#8895aa">标签</b><div style="display:flex;gap:6px;flex-wrap:wrap;margin-top:8px;font-size:11px"><span style="background:#E8F4FF;color:#2FA8FF;border-radius:12px;padding:5px 11px">免费</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 11px">保种</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 11px">2x</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:5px 11px">官方</span></div></div><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:12px;color:#8895aa">价格</b><input class="gi" value="0" style="width:100%;height:36px;font-size:13px;margin-top:6px"><p style="color:#99a;font-size:10px;margin:4px 0 0">最大 1,000,000 火花</p></div><div style="background:#fff;border-radius:12px;padding:14px;font-size:11px;color:#667"><b>📋 发布规范</b><p style="margin:6px 0 0;line-height:1.8">真实有效资源 · 正确分类 · 尊重版权</p></div><button class="f3-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:13px;border-radius:11px;font-size:14px">🚀 发布种子</button></div>
          </div>
''', "E220 · 三折叠 发布种子"))
L(f3_shell("发布字幕", f'''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;max-width:820px;margin:0 auto">
            <div style="background:#fff;border-radius:14px;padding:18px;display:grid;gap:12px"><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">字幕文件 *</label><div style="border:2px dashed #2FBF9B;border-radius:11px;padding:16px;text-align:center;color:#2FBF9B;font-size:13px">📎 点击选择 .ass/.srt</div></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">关联种子</label><input class="gi" placeholder="输入种子ID或名称" style="width:100%;height:38px;font-size:13px"></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">语言</label><div style="display:flex;gap:7px;flex-wrap:wrap">{"".join(chip(x, on=(i==0)) for i,x in enumerate(["简体中文","繁体中文","英语","双语"]))}</div></div></div>
            <div style="background:#fff;border-radius:14px;padding:18px"><b style="font-size:13px">📋 上传规范</b><div style="display:grid;gap:7px;margin-top:10px;font-size:12px;color:#667"><div style="display:flex;gap:8px"><span>✅</span>字幕需与视频同步</div><div style="display:flex;gap:8px"><span>✅</span>标注语言与格式</div><div style="display:flex;gap:8px"><span>✅</span>原创或授权转发</div></div><button class="f3-btn" style="background:#2FBF9B;color:#fff;border:none;width:100%;margin-top:16px;padding:12px;border-radius:11px;font-size:14px">📤 发布字幕</button></div>
          </div>
''', "E221 · 三折叠 发布字幕"))
L(f3_shell("提交候选", f'''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;max-width:820px;margin:0 auto">
            <div style="background:#fff;border-radius:14px;padding:18px;display:grid;gap:12px"><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">候选名称 *</label><input class="gi" placeholder="你想看到什么资源？" style="width:100%;height:38px;font-size:13px"></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">分类</label><select class="gi" style="width:100%;height:38px;font-size:12px"><option>学前教育</option><option>小学</option><option>初中</option><option>高中</option></select></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">理由</label><textarea class="gi" placeholder="为什么需要这个资源？" style="width:100%;height:70px;font-size:12px;resize:none"></textarea></div></div>
            <div style="background:#fff;border-radius:14px;padding:18px"><b style="font-size:13px">💡 候选流程</b><div style="display:grid;gap:10px;margin-top:12px;font-size:12px;color:#667"><div><b style="color:#2FA8FF">1.</b> 提交候选资源</div><div><b style="color:#2FA8FF">2.</b> 其他会员投票支持</div><div><b style="color:#2FA8FF">3.</b> 票数达标进入审核</div><div><b style="color:#2FA8FF">4.</b> 管理组审核后发布</div></div><button class="f3-btn" style="background:#FF7A59;color:#fff;border:none;width:100%;margin-top:14px;padding:12px;border-radius:11px;font-size:14px">🙋 提交候选</button></div>
          </div>
''', "E222 · 三折叠 提交候选"))
L(f3_shell("发起求种", f'''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:14px;max-width:820px;margin:0 auto">
            <div style="background:#fff;border-radius:14px;padding:18px;display:grid;gap:12px"><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">求种标题 *</label><input class="gi" placeholder="例：小学数学思维训练 4年级" style="width:100%;height:38px;font-size:13px"></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">描述</label><textarea class="gi" placeholder="资源详情、版本要求…" style="width:100%;height:76px;font-size:12px;resize:none"></textarea></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">悬赏火花</label><input class="gi" value="5,000" style="width:100%;height:38px;font-size:13px"></div></div>
            <div style="background:#fff;border-radius:14px;padding:18px"><b style="font-size:13px">🙏 进行中的求种</b><div style="display:grid;gap:8px;margin-top:10px;font-size:12px"><div style="border:1px solid #EEF2F7;border-radius:10px;padding:10px 12px"><b>小学数学思维训练 4年级</b><p style="color:#99a;font-size:10px;margin:3px 0 0">3人已求 · 悬赏5,000</p></div><div style="border:1px solid #EEF2F7;border-radius:10px;padding:10px 12px"><b>高中生物实验视频 4K</b><p style="color:#99a;font-size:10px;margin:3px 0 0">2人已求 · 悬赏3,000</p></div></div><button class="f3-btn" style="background:#2FBF9B;color:#fff;border:none;width:100%;margin-top:14px;padding:12px;border-radius:11px;font-size:14px">🙏 发布求种</button></div>
          </div>
''', "E223 · 三折叠 发起求种"))

# 34 搜索
L(f3_shell("搜索", f'''
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:12px;display:flex;gap:9px"><input class="gi" placeholder="搜索种子/字幕/帖子…" style="flex:1;height:40px;font-size:14px"><span style="background:#2FA8FF;color:#fff;border-radius:10px;padding:0 22px;display:flex;align-items:center;font-size:13px">搜索</span></div>
          <div style="background:#fff;border-radius:10px;padding:10px 14px;margin-bottom:12px;font-size:12px;color:#667">找到 <b style="color:#2FA8FF">24</b> 个与「<b>古籍</b>」相关的结果 · 用时 0.18s</div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:10px">{"".join(f3_tor(t) for t in TORRENTS[:6])}</div>
''', "E224 · 三折叠 搜索"))

# ===== 半展开 12页 =====
def f3_half(left_content, right_title, right_content, label, fold_bar="linear-gradient(180deg,#2FA8FF,#5B6BF5)"):
    aid = next_id()
    return f'''    <dc-artboard id="{aid}-f3h" label="{label}" width="720" height="900">
      <div class="fold3-half">
        <div style="flex:1;background:#F5FAFF;border-radius:12px;overflow:hidden;display:flex;flex-direction:column">{left_content}</div>
        <div style="width:10px;background:{fold_bar};border-radius:5px;margin:0 2px"></div>
        <div style="flex:1;background:#F5FAFF;border-radius:12px;overflow:hidden;display:flex;flex-direction:column">
          <div style="background:#fff;padding:10px 14px;display:flex;justify-content:space-between;align-items:center;border-bottom:1px solid #EEF2F7"><b style="font-size:14px">{right_title}</b><span style="font-size:11px;color:#99a">详情 ›</span></div>
          <div style="padding:12px;flex:1;overflow:hidden">{right_content}</div>
        </div>
      </div>
    </dc-artboard>
'''

def hleft(title, inner):
    return f'''<div style="background:#fff;padding:10px 14px;display:flex;justify-content:space-between;align-items:center;border-bottom:1px solid #EEF2F7"><b style="font-size:14px">{title}</b><span style="font-size:11px;color:#99a">•••</span></div><div style="padding:12px;flex:1;overflow:hidden">{inner}</div>'''

half_pages = [
    ("浏览+详情", "🦉 资源浏览", "".join(f3_tor(t, show_tags=False) for t in TORRENTS[:3]), "📄 文件列表", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;display:flex;justify-content:space-between;margin-bottom:6px"><b>{n}</b><span style="color:#99a">{s}</span></div>' for n,s in [("六書正譌.txt","128MB"),("六書正譌.pdf","96MB"),("扫描页/","44文件")])),
    ("论坛+帖子", "💬 论坛版块", "".join(f'<div style="background:#fff;border-radius:10px;padding:10px 12px;margin-bottom:7px"><b style="font-size:12px;color:{c}">{fn}</b><p style="color:#99a;font-size:9px;margin:3px 0 0">{" · ".join(f"{s}({cnt})" for s,cnt in subs[:2])}</p></div>' for fn,subs,c in FORUMS), "📌 最新主题", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;margin-bottom:6px"><b>{t}</b><p style="color:#99a;font-size:9px;margin:2px 0 0">{u} · {d}</p></div>' for t,u,d in [("IYUU辅种教程","gntv","356阅"),("新人必读规则","管理组","892阅"),("求推荐化学资料","study_mom","45阅")])),
    ("课本+科目", "📚 课本中心", '<div style="display:grid;grid-template-columns:1fr 1fr;gap:8px">' + "".join(f'<div style="background:#fff;border-radius:10px;padding:11px;text-align:center"><span style="font-size:22px">📘</span><b style="display:block;font-size:12px;margin-top:3px">{s}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">人教版</p></div>' for s in SUBJECTS[:8]) + '</div>', "📊 我的学习", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;display:flex;justify-content:space-between;margin-bottom:6px"><b>{n}</b><b style="color:#2FA8FF">{v}</b></div>' for n,v in [("已下载课本","24本"),("学习时长","86h"),("本月新增","5本")])),
    ("勋章+卡牌", "🏅 勋章墙", '<div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:8px">' + "".join(f'<div style="background:#fff;border-radius:10px;padding:10px;text-align:center;{"opacity:.4" if i>=4 else ""}"><span style="font-size:22px">{m}</span><b style="display:block;font-size:10px;margin-top:2px">{nm}</b></div>' for i,(m,nm,ds) in enumerate(MEDALS[:9])) + '</div>', "🃏 我的卡牌", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;display:flex;justify-content:space-between;margin-bottom:6px"><b>{n}</b><span style="color:#99a">{v}</span></div>' for n,v in [("闪耀卡","×3"),("古籍卡","×1"),("碎片","18/30")])),
    ("排行+详情", "🏆 排行榜", "".join(f'<div style="display:flex;align-items:center;gap:8px;background:#fff;border-radius:9px;padding:8px 10px;margin-bottom:6px"><span style="width:18px;text-align:center;font-weight:700;color:{"#FFC93C" if i<3 else "#99a"}">{i+1}</span><img src="{OWL}" style="width:24px;height:24px;border-radius:50%"><b style="flex:1;font-size:11px">{u}</b><b style="font-size:10px;color:#FF7A59">{v}</b></div>' for i,(u,cl,v,_) in enumerate(RANKS[:5])), "📊 榜单统计", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;display:flex;justify-content:space-between;margin-bottom:6px"><b>{n}</b><b style="color:#5B6BF5">{v}</b></div>' for n,v in [("TOP1 上传","109.7TB"),("TOP5 均值","90.2TB"),("上榜门槛","62.1TB")])),
    ("商店+购物车", "🛒 火花商店", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;display:flex;justify-content:space-between;margin-bottom:6px"><b>{nm}</b><b style="color:#FF7A59">{pr}</b></div>' for nm,pr,_ in SHOP[:5]), "🛍️ 购物车", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;display:flex;justify-content:space-between;margin-bottom:6px"><b>{n}</b><span style="color:#99a">{v}</span></div>' for n,v in [("1GB上传","400火花"),("补签卡","1,000火花"),("合计","1,400火花")]) + '<div style="background:#2FA8FF;color:#fff;border-radius:9px;padding:10px;text-align:center;font-size:12px;margin-top:8px">去结算</div>'),
    ("任务+绩效", "📋 任务中心", "".join(f'<div style="display:flex;align-items:center;gap:8px;background:#fff;border-radius:9px;padding:9px 10px;font-size:11px;margin-bottom:6px"><span style="font-size:16px">{ic}</span><b style="flex:1">{nm}</b><span style="background:{c}20;color:{c};border-radius:8px;padding:2px 7px;font-size:9px">{st}</span></div>' for ic,nm,rw,st,c in TASKS[:5]), "🏅 绩效考核", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;display:flex;justify-content:space-between;margin-bottom:6px"><b>{n}</b><b style="color:#2FA8FF">{v}</b></div>' for n,v in [("操作总数","28/30"),("通过审核","25/30"),("月领","200,000")])),
    ("保种+官种", "🛡️ 保种区", "".join(f'<div style="display:flex;gap:8px;align-items:center;background:#fff;border-radius:9px;padding:9px 10px;margin-bottom:6px;font-size:11px"><span style="width:26px;height:26px;border-radius:7px;background:{c};color:#fff;display:flex;align-items:center;justify-content:center;font-size:10px;flex-shrink:0">{ic}</span><b style="flex:1;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{nm}</b><span style="color:#99a;font-size:9px">{tm}</span></div>' for nm,ic,c,_,_,_,tm in PRESERVE[:4]), "🏅 官种", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;display:flex;justify-content:space-between;margin-bottom:6px"><b>{nm}</b><span style="color:#99a">{sz} · {dt}</span></div>' for nm,sz,dt in OFFICIALS[:5])),
    ("消息+详情", "📥 收件箱", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;margin-bottom:6px"><b>{f}</b><p style="color:#667;font-size:10px;margin:3px 0 0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{m}</p></div>' for f,m,t in MSGS), "🔍 搜索消息", '<input class="gi" placeholder="搜索消息…" style="width:100%;height:36px;font-size:12px;margin-bottom:10px"><div style="background:#E8F4FF;border-radius:9px;padding:10px;font-size:11px;color:#2FA8FF">找到 2 条与「官种」相关的消息</div>'),
    ("农场+菜市", "🌾 农场", '<div style="display:grid;grid-template-columns:1fr 1fr;gap:8px">' + "".join(f'<div style="background:#fff;border-radius:10px;padding:10px;text-align:center"><span style="font-size:22px">{ic}</span><b style="display:block;font-size:11px;margin-top:2px">{nm}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{"生长中" if i==0 else "空闲"}</p></div>' for i,(ic,nm,tm,v) in enumerate(CROPS)) + '</div>', "🥬 菜市场", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;display:flex;justify-content:space-between;margin-bottom:6px"><b>{nm}</b><b style="color:{"#2FBF9B" if i==0 else "#FF7A59"}">{["620 🔥","880","1,420","1,980"][i]}</b></div>' for i,(ic,nm,tm,v) in enumerate(CROPS))),
    ("我的+收益", "👤 我的数据", '<div style="display:grid;grid-template-columns:1fr 1fr;gap:8px">' + "".join(f'<div style="background:#fff;border-radius:9px;padding:9px;text-align:center;font-size:10px"><b style="display:block;font-size:12px">{v}</b><span style="color:#99a">{nm}</span></div>' for _,nm,v in MYMENU[:6]) + '</div>', "💰 火花收益", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;display:flex;justify-content:space-between;margin-bottom:6px"><b>{n}</b><b style="color:#FF7A59">{v}</b></div>' for n,v in [("当前火花","85.8M"),("每小时","562.8"),("今日获得","13,507")])),
    ("公告+事件", "📢 公告", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;margin-bottom:6px"><b>{nm}</b><p style="color:#99a;font-size:9px;margin:3px 0 0">{t} · {ds}</p></div>' for nm,_,t,ds in NEWS[:4]), "📋 站点事件", "".join(f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;display:flex;justify-content:space-between;margin-bottom:6px"><b>{nm}</b><span style="color:#99a">{t}</span></div>' for t,nm,_,ds in EVENTS[:5])),
]
for lh, lt, lc, rt, rc in half_pages:
    L(f3_half(hleft(lt, lc), rt, rc, f"E{n[0]} · 三折叠 半展开-{lt.split(' ')[-1]}"))

# ===== 折叠态 10页 =====
def f3_closed(title, content, label, gradient="linear-gradient(160deg,#1E3A5F,#0F172A 70%)"):
    aid = next_id()
    return f'''    <dc-artboard id="{aid}-f3c" label="{label}" width="340" height="720">
      <div class="fold3-closed">
        <div style="position:absolute;inset:0;background:{gradient}"></div>
        <div style="position:relative;height:100%;display:flex;flex-direction:column;align-items:center;padding:18px 14px;color:#fff">
          <div style="align-self:flex-start;font-size:12px;opacity:.8">9:41</div>
          <div style="margin-top:20px;width:50px;height:50px;border-radius:14px;background:rgba(255,255,255,.16);display:flex;align-items:center;justify-content:center;font-size:24px">🦉</div>
          <div style="margin-top:10px;text-align:center"><b style="font-size:15px">{title}</b><p style="font-size:10px;opacity:.7;margin:3px 0 0">好学 HxPT</p></div>
          <div style="margin-top:18px;width:100%;display:grid;gap:8px">{content}</div>
          <div style="margin-top:auto;font-size:10px;opacity:.6">好学PT · 种子⇄成长</div>
        </div>
      </div>
    </dc-artboard>
'''
def fc3_card(title, text, time="09:30", hl=False):
    bg = 'rgba(255,255,255,.16)' if not hl else 'linear-gradient(135deg,#2FA8FF44,#5B6BF544)'
    return f'<div style="background:{bg};backdrop-filter:blur(10px);border-radius:13px;padding:10px 12px"><div style="display:flex;justify-content:space-between;font-size:11px"><b>{title}</b><span style="opacity:.6">{time}</span></div><p style="font-size:11px;opacity:.9;margin:4px 0 0;line-height:1.6">{text}</p></div>'

closed_pages = [
    ("今日概览", [fc3_card("🧲 站点数据","19,434 种子 · 4,645 用户 · 142,384 同伴","实时"), fc3_card("📢 公告","站免池进度 52.3%，距双倍免费还差 954,000","09-07")]),
    ("快捷操作", [fc3_card("📥 下载","识典古籍 六書正譌 · 65%","10:20"), fc3_card("📤 上传","做种 14,247 个 · 上传 2.4MB/s","实时"), fc3_card("✅ 签到","今日签到成功 +200 火花","09:00")]),
    ("资源速览", [fc3_card("🔥 热门","小猪佩奇英文版 · 210下载","实时"), fc3_card("🎉 免费","识典古籍系列 · 剩2天","09-06"), fc3_card("🏅 官种","识典古籍 六書正譌 上线","09-06")]),
    ("学习统计", [fc3_card("📚 课本","已下载 24 本 · 学习 86h","本周"), fc3_card("🏆 任务","3 项进行中 · 1 项待领取","实时"), fc3_card("🎖️ 勋章","30/81 已收集 · 37%","实时")]),
    ("经济速览", [fc3_card("💰 火花","85,815,181 · +562.8/h","实时"), fc3_card("🏦 银行","活期 3,321.81 · 定期 0","实时"), fc3_card("🎉 站免池","52.3% · 距双倍免费 954,000","实时")]),
    ("通知中心", [fc3_card("🌟 管理组","您的种子已设为官种","10:30",True), fc3_card("💬 alan5914","求种回复：已发布 ✓","昨天"), fc3_card("🎖️ 系统","签到100天奖励勋章","09-05")]),
    ("农场状态", [fc3_card("🌾 小麦","2小时后成熟 · 500火花","生长中"), fc3_card("🐔 鸡","已成熟，可收获 1,000火花","待收获",True), fc3_card("🥬 菜市场","小麦今日高价 620","12:00刷新")]),
    ("下载管理", [fc3_card("📥 识典古籍","下载中 · 65% · 2.1MB/s","实时"), fc3_card("📥 人教版语文","下载中 · 32% · 1.4MB/s","实时"), fc3_card("📤 做种中","14,247 个 · 上传 2.4MB/s","实时")]),
    ("论坛热帖", [fc3_card("💬 IYUU辅种教程","356 阅读 · 23 回复","09-06"), fc3_card("💬 新人必读规则","892 阅读 · 45 回复","09-05",True), fc3_card("💬 求化学资料","45 阅读 · 8 回复","09-05")]),
    ("我的成长", [fc3_card("🎯 等级","发布员 · 下一级 主管","342天"), fc3_card("📈 数据","上传108.97T · 下载2.83T","实时"), fc3_card("🏅 成就","12/40 已解锁 · 继续加油","实时")]),
]
for cp_title, cards in closed_pages:
    L(f3_closed(cp_title, "".join(cards), f"E{n[0]} · 三折叠 折叠态-{cp_title}"))

# ===== 分屏 8页 =====
def pane(title, inner):
    return f'<div style="flex:1;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:9px 12px;font-size:12px;border-bottom:1px solid #EEF2F7;display:flex;justify-content:space-between"><b>{title}</b><span style="color:#99a;font-size:10px">•••</span></div><div style="padding:10px;flex:1;overflow:hidden;display:grid;gap:7px">{inner}</div></div>'

def f3_split(cols, label):
    aid = next_id()
    return f'''    <dc-artboard id="{aid}-f3s" label="{label}" width="1000" height="760">
      <div class="fold3-screen" style="flex-direction:row;overflow:hidden;border:none">{cols}</div>
    </dc-artboard>
'''
split_pages = [
    ("资源库+论坛+课本", [f'<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px"><b>{t[0][:16]}</b><p style="color:#99a;font-size:9px;margin:3px 0 0">{t[4]} · {t[5]}做种</p></div>' for t in TORRENTS[:4]], ['<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px"><b>小学部交流</b><p style="color:#99a;font-size:9px;margin:3px 0 0">56帖 · 最新10:20</p></div>','<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px"><b>初中部交流</b><p style="color:#99a;font-size:9px;margin:3px 0 0">48帖 · 最新09:45</p></div>','<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px"><b>发邀专区</b><p style="color:#99a;font-size:9px;margin:3px 0 0">20帖 · 最新09:10</p></div>'], ['<div style="background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px"><span style="font-size:18px">📘</span><b style="display:block">语文</b></div>','<div style="background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px"><span style="font-size:18px">📗</span><b style="display:block">数学</b></div>','<div style="background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px"><span style="font-size:18px">🔬</span><b style="display:block">物理</b></div>','<div style="background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px"><span style="font-size:18px">📕</span><b style="display:block">英语</b></div>']),
    ("详情+评论区+文件", ['<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px"><b>识典古籍 六書正譌</b><p style="color:#99a;font-size:9px;margin:3px 0 0">384MB · 12做种 · Free</p></div>','<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;color:#667"><b style="display:block;margin-bottom:3px">📝 简介</b>简繁双版古籍，适合古文学习</div>'], ['<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px"><b>study_mom</b><p style="color:#667;font-size:10px;margin:3px 0 0">扫描页很清晰，感谢！</p></div>','<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px"><b>kiririn</b><p style="color:#667;font-size:10px;margin:3px 0 0">EPUB 体验极佳</p></div>'], ['<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:10px;display:flex;justify-content:space-between"><b>六書正譌.txt</b><span style="color:#99a">128M</span></div>','<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:10px;display:flex;justify-content:space-between"><b>六書正譌.pdf</b><span style="color:#99a">96M</span></div>','<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:10px;display:flex;justify-content:space-between"><b>扫描页/</b><span style="color:#99a">44</span></div>']),
    ("商店+银行+站免池", ['<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between"><b>1GB上传</b><b style="color:#FF7A59">400</b></div>','<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between"><b>补签卡</b><b style="color:#FF7A59">1,000</b></div>','<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between"><b>APP VIP</b><b style="color:#FF7A59">5,000</b></div>'], ['<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px"><b>总资产</b><p style="font-size:13px;margin:3px 0 0">85,814,940</p></div>','<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between"><b>活期</b><b>3,321</b></div>'], ['<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px"><b>站免池</b><p style="font-size:13px;margin:3px 0 0">52.3%</p></div>','<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;color:#FF7A59">距双倍免费 954,000</div>']),
    ("农场+任务+勋章", ['<div style="background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px"><span style="font-size:20px">🌾</span><b style="display:block">小麦</b></div>','<div style="background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px"><span style="font-size:20px">🐔</span><b style="display:block">鸡</b></div>','<div style="background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px"><span style="font-size:20px">🥔</span><b style="display:block">土豆</b></div>'], ['<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between"><b>上传10种子</b><span style="color:#2FBF9B">✓</span></div>','<div style="background:#fff;border-radius:8px;padding:9px 10px;font-size:11px;display:flex;justify-content:space-between"><b>下载5种子</b><span style="color:#FF7A59">3/5</span></div>'], ['<div style="background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px"><span style="font-size:18px">🏅</span><b style="display:block">新人</b></div>','<div style="background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px"><span style="font-size:18px">📚</span><b style="display:block">初出</b></div>','<div style="background:#fff;border-radius:8px;padding:9px;text-align:center;font-size:10px;opacity:.4"><span style="font-size:18px">🎓</span><b style="display:block">学霸</b></div>']),
]
for sp_title, pane1, pane2, pane3 in split_pages:
    titles = sp_title.split("+") + ["面板3"]
    cols_html = pane(titles[0], "".join(pane1)) + pane(titles[1], "".join(pane2)) + pane(titles[2], "".join(pane3))
    L(f3_split(cols_html, f"E{n[0]} · 三折叠 分屏-{sp_title}"))

html = "\n".join(out)
open('/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen/fold3_boards.html','w',encoding='utf-8').write(html)
print("fold3 boards:", len(out), "last id:", n[0])
