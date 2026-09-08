# -*- coding: utf-8 -*-
"""三折叠第三批: 用户详情页签/消息/游戏/课本/浏览/后台/银行/论坛/半展开/折叠态/分屏 ~100页"""
import sys
sys.path.insert(0, '/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen')
from data import *

n = [501]
def next_id():
    n[0] += 1
    return f"e{n[0]}"

def f3_shell(title, content, label):
    aid = next_id()
    return f'''    <dc-artboard id="{aid}-f3p" label="{label}" width="1000" height="760">
      <div class="fold3-screen">
        <div class="f3-statusbar"><span>9:41</span><span>📶 🔋 100%</span></div>
        <div class="f3-topbar"><div class="f3-logo"><img src="{OWL}" alt=""><b>{title}</b></div><div class="f3-search">🔍 搜索…</div><div class="f3-actions"><span>🔔</span><img class="f3-avatar" src="{OWL}" alt=""></div></div>
        <div class="f3-body">{content}</div>
        <div class="f3-dock"><span>🏠</span><span>🔍</span><span>📥</span><span>💬</span><span>👤</span></div>
      </div>
    </dc-artboard>
'''

def card3(icon, bg, name, sub, right=''):
    return f'<div style="display:flex;gap:10px;align-items:center;background:#fff;border-radius:12px;padding:13px 14px"><span style="width:38px;height:38px;border-radius:10px;background:{bg};color:#fff;display:flex;align-items:center;justify-content:center;font-size:15px;flex-shrink:0">{icon}</span><div style="flex:1;min-width:0"><b style="font-size:13px;display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{name}</b><p style="color:#99a;font-size:11px;margin:2px 0 0">{sub}</p></div>{f"<div style=text-align:right;flex-shrink:0>{right}</div>" if right else ""}</div>'

def chip3(t, on=False):
    return f'<span style="background:{"#2FA8FF" if on else "#F0F4F8"};color:{"#fff" if on else "#667"};border-radius:16px;padding:6px 14px;font-size:12px">{t}</span>'

def tag3(t, c="#2FA8FF", bg="#E8F4FF"):
    return f'<span style="background:{bg};color:{c};border-radius:8px;padding:2px 7px;font-size:10px">{t}</span>'

def hc(title, sub):
    return f'<div style="background:#fff;border-radius:9px;padding:9px 11px;font-size:11px;margin-bottom:6px"><b>{title}</b><p style="color:#99a;font-size:9px;margin:2px 0 0">{sub}</p></div>'

out = []
L = out.append

def info3(title, label, items, cols=3, bg="#E8F4FF"):
    cards = "".join(card3(ic, bg, a, b) for ic,a,b in items)
    L(f3_shell(title, f'<div style="display:grid;grid-template-columns:repeat({cols},1fr);gap:11px">{cards}</div>', label))

# ===== 1. 用户详情页签 ×8 =====
tabs3 = ["概览","上传种子","做种种子","下载历史","评论","收藏","好友","勋章"]
for ti, tab in enumerate(tabs3):
    if tab == "概览":
        body = f'''
          <div style="display:grid;grid-template-columns:1fr 2fr;gap:11px">
            <div style="background:#fff;border-radius:14px;padding:16px;text-align:center"><img src="{OWL}" style="width:64px;height:64px;border-radius:50%"><b style="display:block;font-size:16px;margin-top:8px">gntv</b><p style="color:#99a;font-size:11px;margin:3px 0 0">发布员 · 加入 2025-07-16</p><div style="background:#FFE8F5;color:#FF8FC7;border-radius:10px;padding:5px;font-size:11px;margin-top:8px">🎖️ 勋章 30/81</div></div>
            <div style="display:grid;gap:9px">
              <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px"><div style="background:#F0F7FF;border-radius:11px;padding:12px;text-align:center"><b style="font-size:17px;color:#2FA8FF">108.97T</b><p style="color:#99a;font-size:10px;margin:2px 0 0">上传</p></div><div style="background:#F0F7FF;border-radius:11px;padding:12px;text-align:center"><b style="font-size:17px;color:#2FBF9B">2.83T</b><p style="color:#99a;font-size:10px;margin:2px 0 0">下载</p></div><div style="background:#F0F7FF;border-radius:11px;padding:12px;text-align:center"><b style="font-size:17px">38.5</b><p style="color:#99a;font-size:10px;margin:2px 0 0">分享率</p></div></div>
              <div style="background:#fff;border-radius:12px;padding:13px;display:grid;gap:7px;font-size:12px"><div style="display:flex;justify-content:space-between;padding-bottom:6px;border-bottom:1px solid #F0F4F8"><span style="color:#99a">邮箱</span><b>95836184@qq.com</b></div><div style="display:flex;justify-content:space-between;padding-bottom:6px;border-bottom:1px solid #F0F4F8"><span style="color:#99a">当前活动</span><b>14,247</b></div><div style="display:flex;justify-content:space-between"><span style="color:#99a">连接数</span><b>无限制</b></div></div>
              <div style="background:linear-gradient(135deg,#FFE8F5,#FFF8E8);border-radius:12px;padding:13px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:13px">💛 站免池进度</b><p style="color:#99a;font-size:10px;margin:2px 0 0">52.3% · 距双倍免费差 95.4万</p></div><span style="font-size:20px">🚀</span></div>
            </div>
          </div>'''
    elif tab == "上传种子":
        body = f'<div style="display:grid;grid-template-columns:1fr 1fr;gap:10px">{"".join(card3(t[1], t[2], t[0][:26]+"…" if len(t[0])>26 else t[0], f"{t[4]} · {t[5]}做种", tag3("Free") if t[6].startswith("Free") else "") for t in TORRENTS[:8])}</div><div style="padding:12px;text-align:center;color:#99a;font-size:12px">共 16,348 个上传种子 · 第 1/2724 页</div>'
    elif tab == "做种种子":
        body = f'<div style="display:grid;grid-template-columns:1fr 1fr;gap:10px">{"".join(card3(t[1], t[2], t[0][:26]+"…" if len(t[0])>26 else t[0], f"做种中 · 上传 1.2MB/s") for t in TORRENTS[:8])}<div style="background:#fff;border-radius:12px;padding:14px;text-align:center;color:#99a;font-size:12px">共 14,247 个做种种子</div></div>'
    elif tab == "下载历史":
        body = f'<div style="display:grid;grid-template-columns:1fr 1fr;gap:10px">{"".join(card3(t[1], t[2], t[0][:26]+"…" if len(t[0])>26 else t[0], f"完成 · 剩余 0 · {t[4]}") for t in TORRENTS[:6])}</div><div style="padding:12px;text-align:center;color:#99a;font-size:12px">共 282 条下载记录</div>'
    elif tab == "评论":
        body = '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:10px">
            <div style="background:#fff;border-radius:12px;padding:14px"><div style="display:flex;justify-content:space-between;font-size:13px"><b>识典古籍 六書正譌</b><span style="color:#99a;font-size:10px">09-06</span></div><p style="font-size:12px;color:#667;margin:6px 0 0">资源制作精良，扫描页清晰！</p><div style="display:flex;gap:10px;margin-top:8px;font-size:10px;color:#99a">👍 12 · 💬 2</div></div>
            <div style="background:#fff;border-radius:12px;padding:14px"><div style="display:flex;justify-content:space-between;font-size:13px"><b>蓝色星球 第三季</b><span style="color:#99a;font-size:10px">09-03</span></div><p style="font-size:12px;color:#667;margin:6px 0 0">纪录片画质一流，孩子最爱！</p><div style="display:flex;gap:10px;margin-top:8px;font-size:10px;color:#99a">👍 25 · 💬 3</div></div>
            <div style="background:#fff;border-radius:12px;padding:14px"><div style="display:flex;justify-content:space-between;font-size:13px"><b>人教版语文四年级</b><span style="color:#99a;font-size:10px">08-28</span></div><p style="font-size:12px;color:#667;margin:6px 0 0">课本齐全，排版精美。</p><div style="display:flex;gap:10px;margin-top:8px;font-size:10px;color:#99a">👍 8 · 💬 1</div></div>
            <div style="background:#fff;border-radius:12px;padding:14px"><div style="display:flex;justify-content:space-between;font-size:13px"><b>初中物理实验合集</b><span style="color:#99a;font-size:10px">08-20</span></div><p style="font-size:12px;color:#667;margin:6px 0 0">实验视频很全，强烈推荐！</p><div style="display:flex;gap:10px;margin-top:8px;font-size:10px;color:#99a">👍 31 · 💬 5</div></div>
          </div>'''
    elif tab == "收藏":
        body = f'<div style="display:grid;grid-template-columns:1fr 1fr;gap:10px">{"".join(card3("⭐", "#FFF8E8", t[0][:26]+"…" if len(t[0])>26 else t[0], f"收藏于 09-0{6-i}") for i,t in enumerate(TORRENTS[:8]))}</div>'
    elif tab == "好友":
        body = f'''
          <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:10px">
            <div style="background:#fff;border-radius:12px;padding:14px;text-align:center"><img src="{OWL}" style="width:46px;height:46px;border-radius:50%"><b style="display:block;font-size:13px;margin-top:6px">alan5914</b><p style="color:#99a;font-size:10px;margin:2px 0 0">上传 16,348 · 在线</p><span style="display:inline-block;background:#E8F4FF;color:#2FA8FF;border-radius:9px;padding:4px 14px;font-size:11px;margin-top:8px">💬 发消息</span></div>
            <div style="background:#fff;border-radius:12px;padding:14px;text-align:center"><img src="{OWL}" style="width:46px;height:46px;border-radius:50%"><b style="display:block;font-size:13px;margin-top:6px">study_mom</b><p style="color:#99a;font-size:10px;margin:2px 0 0">上传 892 · 离线</p><span style="display:inline-block;background:#F0F4F8;color:#99a;border-radius:9px;padding:4px 14px;font-size:11px;margin-top:8px">💬 发消息</span></div>
            <div style="background:#fff;border-radius:12px;padding:14px;text-align:center"><img src="{OWL}" style="width:46px;height:46px;border-radius:50%"><b style="display:block;font-size:13px;margin-top:6px">kiririn</b><p style="color:#99a;font-size:10px;margin:2px 0 0">上传 3,204 · 在线</p><span style="display:inline-block;background:#E8F4FF;color:#2FA8FF;border-radius:9px;padding:4px 14px;font-size:11px;margin-top:8px">💬 发消息</span></div>
          </div>'''
    else:
        cards = ""
        for i,(m,nm,ds) in enumerate(MEDALS):
            brd = 'border:2px solid #E8F4FF' if i<6 else 'opacity:.5'
            cards += f'<div style="background:#fff;border-radius:12px;padding:13px;text-align:center;{brd}"><span style="font-size:26px">{m}</span><b style="display:block;font-size:11px;margin-top:4px">{nm}</b></div>'
        body = f'<div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px">{cards}</div><div style="padding:12px;text-align:center;color:#99a;font-size:12px">已收集 30/81 枚勋章 · 37%</div>'
    L(f3_shell("用户详情", f'<div style="display:flex;gap:6px;margin-bottom:12px;flex-wrap:wrap">{"".join(chip3(x, on=(x==tab)) for x in tabs3)}</div>{body}', f"E50{2+ti} · 三折叠 用户详情-{tab}"))

# ===== 2. 消息中心 ×4 =====
L(f3_shell("收件箱", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:10px">
            <div style="background:#fff;border-radius:12px;padding:14px"><div style="display:flex;justify-content:space-between;font-size:13px"><b>🌟 管理组</b><span style="color:#99a;font-size:10px">10:30</span></div><p style="font-size:12px;color:#667;margin:6px 0 0">您的种子「识典古籍 六書正譌」已设为官种，获得 5x 收益加成！</p></div>
            <div style="background:#fff;border-radius:12px;padding:14px"><div style="display:flex;justify-content:space-between;font-size:13px"><b>💬 alan5914</b><span style="color:#99a;font-size:10px">昨天</span></div><p style="font-size:12px;color:#667;margin:6px 0 0">求种回复：已发布 ✓ 快来下载吧</p></div>
            <div style="background:#fff;border-radius:12px;padding:14px"><div style="display:flex;justify-content:space-between;font-size:13px"><b>🎖️ 系统</b><span style="color:#99a;font-size:10px">09-05</span></div><p style="font-size:12px;color:#667;margin:6px 0 0">签到 100 天奖励已发放：10,000 火花 + 勋章</p></div>
            <div style="background:linear-gradient(135deg,#E8F4FF,#E8F8F2);border-radius:12px;padding:14px;display:flex;align-items:center;justify-content:center;font-size:13px;color:#2FA8FF">📨 查看全部消息 →</div>
          </div>
''', "E510 · 三折叠 收件箱"))
for ci, (un, msgs) in enumerate([
    ("管理组", [("管理组","您的种子已设为官种 🎉", "10:30"), ("我","感谢支持！继续努力 🌱", "10:32"), ("管理组","继续保持高质量发布", "10:35")]),
    ("alan5914", [("alan5914","求种回复：已发布 ✓", "昨天 21:15"), ("我","太感谢啦！马上去下载", "昨天 21:20"), ("alan5914","记得做种哦，保种区福利多", "昨天 21:22"), ("我","好嘞！已经在保种区挂上了", "昨天 21:25")]),
    ("kiririn", [("kiririn","蓝色星球字幕还有吗？", "09-05 14:02"), ("我","有的！刚补了内封官方中字", "09-05 14:10"), ("kiririn","太棒了，马上收藏！", "09-05 14:12")]),
]):
    bubbles = "".join(f'<div style="display:flex;{"justify-content:flex-end" if u=="我" else "justify-content:flex-start"};margin-bottom:9px"><div style="max-width:70%;background:{"linear-gradient(135deg,#2FA8FF,#5B6BF5)" if u=="我" else "#fff"};color:{"#fff" if u=="我" else "#1F2A44"};border-radius:13px;padding:10px 13px;font-size:12px;line-height:1.6"><b style="display:block;font-size:9px;opacity:.7;margin-bottom:3px">{u} · {t}</b>{m}</div></div>' for u,m,t in msgs)
    L(f3_shell(f"会话-{un}", f'<div style="background:#F5FAFF;border-radius:13px;padding:14px;margin-bottom:12px;min-height:420px">{bubbles}</div><div style="background:#fff;border-radius:12px;padding:11px;display:flex;gap:9px"><input class="gi" placeholder="输入消息…" style="flex:1;height:40px;font-size:13px"><span style="background:#2FA8FF;color:#fff;border-radius:10px;padding:0 20px;display:flex;align-items:center;font-size:13px">发送</span></div>', f"E51{1+ci} · 三折叠 会话-{un}"))
info3("消息设置", "E514 · 三折叠 消息设置", [("🔔","站内通知","开启"),("📧","邮件通知","开启"),("📲","手机推送","关闭"),("🔄","提醒频率","即时"),("🔕","免打扰时段","23:00-07:00"),("🧹","自动清理","30天前自动归档")], 3, "#E8F4FF")

# ===== 3. 游戏状态 ×6 =====
L(f3_shell("五子棋-对战", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="background:#fff;border-radius:13px;padding:14px"><div style="display:flex;justify-content:space-between;margin-bottom:10px"><b style="font-size:13px">⚫ 我（白）</b><span style="color:#99a;font-size:11px">12 胜</span></div>
              <div style="background:#F5E8C8;border-radius:9px;padding:14px"><div style="display:grid;grid-template-columns:repeat(6,1fr);gap:0;max-width:380px;margin:0 auto"><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A;border-radius:50%;box-shadow:inset 0 0 0 3px #fff"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A;border-radius:50%"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div></div></div>
              <div style="background:#E8F4FF;border-radius:10px;padding:10px;font-size:12px;color:#2FA8FF;margin-top:10px;text-align:center">⏳ 轮到你了 · 限时 30s</div>
            </div>
            <div style="display:grid;gap:10px">
              <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">⚫ 对手：alan5914</b><div style="display:flex;gap:6px;margin-top:8px;flex-wrap:wrap"><span style="background:#F0F4F8;color:#667;border-radius:10px;padding:4px 12px;font-size:11px">积分 2,480</span><span style="background:#FFE8F5;color:#FF8FC7;border-radius:10px;padding:4px 12px;font-size:11px">11 胜</span></div></div>
              <div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📊 棋局信息</b><div style="display:grid;gap:7px;margin-top:8px;font-size:12px"><span style="display:flex;justify-content:space-between"><span style="color:#99a">步数</span><b>18 手</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">用时</span><b>4:32</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">下注</span><b>2,000 火花</b></span></div></div>
              <button class="f2-btn" style="background:linear-gradient(135deg,#FF7A59,#FFC93C);color:#fff;border:none;padding:12px;border-radius:11px;font-size:14px">🏳️ 认输</button>
            </div>
          </div>
''', "E515 · 三折叠 五子棋-对战"))
L(f3_shell("刮刮乐-中奖", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="background:linear-gradient(135deg,#FF7A59,#FFC93C);border-radius:14px;padding:18px;color:#fff;text-align:center"><b style="font-size:18px">🎫 刮刮乐</b><p style="font-size:12px;opacity:.9;margin:5px 0 0">2,000 火花 / 次</p><div style="height:8px;background:rgba(255,255,255,.3);border-radius:4px;overflow:hidden;margin-top:10px"><div style="width:70%;height:100%;background:#fff;border-radius:4px"></div></div><p style="font-size:10px;opacity:.8;margin-top:5px">今日已刮 7 次</p></div>
            <div style="display:grid;gap:10px"><div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px"><div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:10px;height:64px;display:flex;align-items:center;justify-content:center;font-size:24px">🎁</div><div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:10px;height:64px;display:flex;align-items:center;justify-content:center;font-size:24px">💎</div><div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:10px;height:64px;display:flex;align-items:center;justify-content:center;font-size:24px">🎉</div></div><div style="background:#FFF8E8;border-radius:11px;padding:12px;font-size:12px;color:#B8860B;text-align:center">🎊 刮中「幸运礼盒」+5,000 火花！</div><button class="f2-btn" style="background:#FF7A59;color:#fff;border:none;padding:12px;border-radius:11px;font-size:14px">🎫 再刮一次</button></div>
          </div>
''', "E516 · 三折叠 刮刮乐-中奖"))
L(f3_shell("九宫格-开奖", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:7px;max-width:420px"><div style="background:#fff;border-radius:11px;height:110px;display:flex;align-items:center;justify-content:center;font-size:34px;opacity:.5">📚</div><div style="background:#fff;border-radius:11px;height:110px;display:flex;align-items:center;justify-content:center;font-size:34px;opacity:.5">💰</div><div style="background:#fff;border-radius:11px;height:110px;display:flex;align-items:center;justify-content:center;font-size:34px;opacity:.5">🎖️</div><div style="background:#fff;border-radius:11px;height:110px;display:flex;align-items:center;justify-content:center;font-size:34px;opacity:.5">🎁</div><div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:11px;height:110px;display:flex;align-items:center;justify-content:center;font-size:42px;box-shadow:0 0 24px rgba(255,201,60,.8)">💎</div><div style="background:#fff;border-radius:11px;height:110px;display:flex;align-items:center;justify-content:center;font-size:34px;opacity:.5">⭐</div><div style="background:#fff;border-radius:11px;height:110px;display:flex;align-items:center;justify-content:center;font-size:34px;opacity:.5">🌈</div><div style="background:#fff;border-radius:11px;height:110px;display:flex;align-items:center;justify-content:center;font-size:34px;opacity:.5">🔥</div><div style="background:#fff;border-radius:11px;height:110px;display:flex;align-items:center;justify-content:center;font-size:34px;opacity:.5">🎯</div></div>
            <div style="display:grid;gap:10px"><div style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);border-radius:14px;padding:16px;color:#fff;text-align:center"><b style="font-size:16px">🎯 开奖结果</b><p style="font-size:26px;margin:8px 0 0">💎 钻石</p></div><div style="background:#FFF8E8;border-radius:11px;padding:13px;font-size:12px;color:#B8860B;text-align:center">🎉 恭喜抽中钻石！+10,000 火花</div><div style="display:grid;grid-template-columns:1fr 1fr;gap:8px"><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><b style="font-size:14px;color:#2FA8FF">1/8</b><p style="color:#99a;font-size:10px;margin:2px 0 0">今日中奖次数</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><b style="font-size:14px;color:#FF7A59">50,000</b><p style="color:#99a;font-size:10px;margin:2px 0 0">本月累计中奖</p></div></div></div>
          </div>
''', "E517 · 三折叠 九宫格-开奖"))
L(f3_shell("猜大小-开奖", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="background:linear-gradient(135deg,#2FBF9B,#2FA8FF);border-radius:14px;padding:18px;color:#fff;text-align:center"><b style="font-size:17px">🎲 开奖结果</b><div style="display:flex;gap:10px;justify-content:center;margin-top:14px"><span style="background:#fff;color:#1F2A44;border-radius:10px;width:58px;height:58px;display:flex;align-items:center;justify-content:center;font-size:26px;font-weight:700">4</span><span style="background:#fff;color:#1F2A44;border-radius:10px;width:58px;height:58px;display:flex;align-items:center;justify-content:center;font-size:26px;font-weight:700">2</span><span style="background:#fff;color:#1F2A44;border-radius:10px;width:58px;height:58px;display:flex;align-items:center;justify-content:center;font-size:26px;font-weight:700">6</span></div><p style="font-size:14px;margin:12px 0 0">点数合计 12 · 大</p></div>
            <div style="display:grid;gap:10px"><div style="background:#E8F8F2;border-radius:12px;padding:14px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:13px">🎉 押中「大」！</b><p style="color:#99a;font-size:11px;margin:3px 0 0">押注 2,000 × 1.95</p></div><b style="font-size:18px;color:#2FBF9B">+3,900</b></div><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">📊 我的战绩</b><div style="display:grid;gap:7px;margin-top:8px;font-size:12px"><span style="display:flex;justify-content:space-between"><span style="color:#99a">今日</span><b>8 胜 2 负</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">本月</span><b>+23,400 火花</b></span></div></div><button class="f2-btn" style="background:linear-gradient(135deg,#2FBF9B,#2FA8FF);color:#fff;border:none;padding:12px;border-radius:11px;font-size:14px">🎲 再来一局</button></div>
          </div>
''', "E518 · 三折叠 猜大小-开奖"))
L(f3_shell("卡牌合成", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="display:grid;gap:10px"><b style="font-size:14px">🃏 选择素材卡</b><div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px"><div style="background:linear-gradient(135deg,#E8F4FF,#D6E9FF);border-radius:11px;padding:14px;text-align:center;border:2px solid #2FA8FF"><span style="font-size:28px">🦉</span><b style="display:block;font-size:10px;margin-top:3px">猫头鹰·N ×1</b></div><div style="background:linear-gradient(135deg,#E8F4FF,#D6E9FF);border-radius:11px;padding:14px;text-align:center;border:2px solid #2FA8FF"><span style="font-size:28px">🦉</span><b style="display:block;font-size:10px;margin-top:3px">猫头鹰·N ×1</b></div><div style="background:linear-gradient(135deg,#E8F4FF,#D6E9FF);border-radius:11px;padding:14px;text-align:center;border:2px solid #2FA8FF"><span style="font-size:28px">🦉</span><b style="display:block;font-size:10px;margin-top:3px">猫头鹰·N ×1</b></div></div><div style="background:#FFF8E8;border-radius:11px;padding:12px;font-size:12px;color:#667;line-height:1.8">💡 3 张同阶 N 卡 → 1 张 R 卡<br>成功概率 80% · 失败返还 1 张</div></div>
            <div style="display:grid;gap:10px"><div style="background:linear-gradient(135deg,#8B5CF6,#5B6BF5);border-radius:14px;padding:18px;color:#fff;text-align:center"><b style="font-size:15px">🔮 合成预览</b><p style="font-size:30px;margin:10px 0 0">📘</p><b style="display:block;font-size:14px;margin-top:6px">古籍·R</b><p style="font-size:11px;opacity:.85;margin:4px 0 0">消耗：3 × 猫头鹰·N</p></div><button class="f2-btn" style="background:linear-gradient(135deg,#8B5CF6,#5B6BF5);color:#fff;border:none;padding:13px;border-radius:12px;font-size:15px">✨ 开始合成</button><div style="background:#fff;border-radius:12px;padding:13px;font-size:12px;color:#99a;text-align:center">🔄 碎片 18/30 · 可兑换 R 卡包</div></div>
          </div>
''', "E519 · 三折叠 卡牌合成"))
L(f3_shell("猜大小-押注", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="background:linear-gradient(135deg,#2FBF9B,#2FA8FF);border-radius:14px;padding:18px;color:#fff;text-align:center"><b style="font-size:17px">🎲 猜大小</b><p style="font-size:11px;opacity:.9;margin:5px 0 0">三枚骰子 · 4-6 为大 · 1-3 为小</p><div style="display:flex;gap:9px;justify-content:center;margin-top:14px"><span style="background:rgba(255,255,255,.25);border-radius:9px;width:52px;height:52px;display:flex;align-items:center;justify-content:center;font-size:24px">🎲</span><span style="background:rgba(255,255,255,.25);border-radius:9px;width:52px;height:52px;display:flex;align-items:center;justify-content:center;font-size:24px">🎲</span><span style="background:rgba(255,255,255,.25);border-radius:9px;width:52px;height:52px;display:flex;align-items:center;justify-content:center;font-size:24px">🎲</span></div></div>
            <div style="display:grid;gap:10px"><div style="background:#fff;border-radius:12px;padding:14px"><b style="font-size:13px">💰 押注金额</b><input class="gi" value="2,000" style="width:100%;height:42px;font-size:16px;margin-top:7px"><div style="display:flex;gap:6px;margin-top:9px;flex-wrap:wrap">{"".join(chip3(x, on=(i==2)) for i,x in enumerate(["500","1,000","2,000","5,000","10,000"]))}</div></div><div style="display:grid;grid-template-columns:1fr 1fr;gap:9px"><div style="background:#E8F4FF;border-radius:12px;padding:14px;text-align:center"><b style="font-size:16px;color:#2FA8FF">押 大</b><p style="color:#99a;font-size:11px;margin:2px 0 0">4-6 点 · 1.95x</p></div><div style="background:#FFE8F5;border-radius:12px;padding:14px;text-align:center"><b style="font-size:16px;color:#FF8FC7">押 小</b><p style="color:#99a;font-size:11px;margin:2px 0 0">1-3 点 · 1.95x</p></div></div></div>
          </div>
''', "E520 · 三折叠 猜大小-押注"))

# ===== 4. 课本管理 ×6 =====
textbook_pages = [
    ("我的提交", "📋", [("部编版 语文 四年级上册","审核通过 · 09-01"),("人教版 数学 三年级上册","审核通过 · 08-28"),("教科版 科学 五年级上册","审核中 · 08-25")]),
    ("管理课本", "⚙️", [("课本池总量","128 本"),("待审核","5 本"),("已通过","120 本"),("被驳回","3 本")]),
    ("创建课本", "➕", [("课本名称","输入课本名称"),("年级","小学/初中/高中"),("科目","选择科目"),("文件","上传课本资源")]),
    ("课本公告", "📢", [("新增高中物理选修目录","09-06"),("秋季学期课本已更新","09-01"),("课本提交规范更新","08-28")]),
    ("课本帮助", "❓", [("如何提交课本？","查看提交规范"),("课本审核周期","1-3 个工作日"),("资源格式要求","PDF/EPUB/图片")]),
    ("课本统计", "📊", [("累计下载","128,420 次"),("本月新增","36 本"),("最受欢迎","人教版数学")]),
]
for pi, (pn, ic, items) in enumerate(textbook_pages):
    if pn in ("管理课本","课本统计"):
        cards = "".join(f'<div style="background:#fff;border-radius:12px;padding:14px;text-align:center"><b style="font-size:18px;color:#2FA8FF">{v}</b><p style="color:#99a;font-size:11px;margin:3px 0 0">{k}</p></div>' for k,v in items)
        body = f'<div style="display:grid;grid-template-columns:repeat(4,1fr);gap:10px">{cards}</div>'
    else:
        rows = ""
        for k,v in items:
            rows += f'<div style="background:#fff;border-radius:12px;padding:13px 15px;display:flex;gap:11px;align-items:center"><span style="width:40px;height:40px;border-radius:10px;background:#E8F4FF;display:flex;align-items:center;justify-content:center;font-size:17px">{ic}</span><div style="flex:1"><b style="font-size:13px">{k}</b><p style="color:#99a;font-size:11px;margin:2px 0 0">{v}</p></div><span style="color:#2FA8FF;font-size:11px">→</span></div>'
        body = f'<div style="display:grid;gap:10px">{rows}</div>'
    L(f3_shell(f"课本-{pn}", f'<div style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);border-radius:14px;padding:15px;color:#fff;margin-bottom:12px;display:flex;align-items:center;gap:11px"><span style="font-size:26px">{ic}</span><div><b style="font-size:16px">{pn}</b><p style="font-size:11px;opacity:.9;margin:2px 0 0">小学、初中、高中课本资源一键下载</p></div></div>{body}', f"E52{1+pi} · 三折叠 课本-{pn}"))

# ===== 5. 浏览组合 ×4 =====
for bi, (cn, sort, col) in enumerate([("学前教育","热门","#FF8FC7"),("小学","做种数","#2FBF9B"),("初中","大小","#5B6BF5"),("高中","最新","#FF7A59")]):
    L(f3_shell("浏览", f'''
          <div style="display:flex;gap:7px;margin-bottom:12px;flex-wrap:wrap">{"".join(chip3(x, on=(x==cn)) for x in ["全部","学前教育","小学","初中","职高","高中","教育影音","纪录片"])}</div>
          <div style="display:flex;gap:7px;margin-bottom:12px"><span style="background:#2FA8FF;color:#fff;border-radius:14px;padding:5px 13px;font-size:11px">{sort}</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 13px;font-size:11px">最新</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 13px;font-size:11px">免费</span><span style="background:#F0F4F8;color:#667;border-radius:14px;padding:5px 13px;font-size:11px">热门</span></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:10px">{"".join(card3(t[1], col, t[0][:26]+"…" if len(t[0])>26 else t[0], f"{t[3]} · {t[4]} · {t[5]}做种", tag3("Free", bg="#E8F8F2") if t[6].startswith("Free") else "") for t in TORRENTS[bi*4:(bi+1)*4])}</div>
          <div style="padding:13px;text-align:center;color:#99a;font-size:12px">共 {[1200,3200,2800,2100][bi]} 个种子 · 第 1/200 页</div>
''', f"E52{7+bi} · 三折叠 浏览-{cn}-{sort}"))

# ===== 6. 更多详情 ×4 =====
for ti, t in enumerate(TORRENTS[11:15]):
    L(f3_shell("种子详情", f'''
          <div style="display:grid;grid-template-columns:1.4fr 1fr;gap:12px">
            <div style="display:grid;gap:10px">
              <div style="background:#fff;border-radius:13px;padding:15px"><div style="display:flex;gap:8px;align-items:flex-start"><b style="font-size:14px;flex:1;line-height:1.5">{t[0]}</b><span class="tag tag-free">Free</span></div><p style="color:#99a;font-size:11px;margin:7px 0 0">发布者 gntv · 发布员 · 2026-09-06</p><div style="display:flex;gap:6px;margin-top:9px;flex-wrap:wrap">{"".join(tag3(x) for x in t[6].split("/"))}</div></div>
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px;color:#8895aa">基本信息</b><div style="display:grid;grid-template-columns:1fr 1fr;gap:9px;margin-top:10px"><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><p style="color:#99a;font-size:10px;margin:0">大小</p><b style="font-size:14px">{t[4]}</b></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><p style="color:#99a;font-size:10px;margin:0">类型</p><b style="font-size:14px">{t[3]}</b></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><p style="color:#99a;font-size:10px;margin:0">做种</p><b style="font-size:14px;color:#2FA8FF">{t[5]}</b></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><p style="color:#99a;font-size:10px;margin:0">下载</p><b style="font-size:14px;color:#2FBF9B">3</b></div></div></div>
              <button class="f2-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;padding:13px;border-radius:12px;font-size:15px">⬇️ 下载种子</button>
            </div>
            <div style="display:grid;gap:10px">
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px">📄 文件列表</b><div style="display:grid;gap:7px;margin-top:9px;font-size:11px"><span style="display:flex;justify-content:space-between"><span style="color:#667">{t[0][:12]}…</span><b style="color:#99a">主文件</b></span><span style="display:flex;justify-content:space-between"><span style="color:#667">封面/截图</span><b style="color:#99a">附</b></span><span style="display:flex;justify-content:space-between"><span style="color:#667">NFO</span><b style="color:#99a">附</b></span></div></div>
              <div style="background:linear-gradient(135deg,#E8F4FF,#E8F8F2);border-radius:13px;padding:14px"><b style="font-size:13px">🎖️ 官方制作组</b><p style="font-size:11px;color:#667;margin:6px 0 0">本站官方制作组整理发布，品质保障</p></div>
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px">💬 最新评论</b><p style="font-size:12px;color:#667;margin:7px 0 0">「资源很赞，感谢发布！」— study_mom</p><p style="font-size:12px;color:#667;margin:5px 0 0">「做种中，支持！」— kiririn</p></div>
            </div>
          </div>
''', f"E53{1+ti} · 三折叠 详情-{t[1]}"))

# ===== 7. 后台详情 ×4 =====
L(f3_shell("后台-种子管理", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="display:grid;gap:10px"><div style="background:#fff;border-radius:13px;padding:14px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:14px">⏳ 待审核种子</b><p style="color:#99a;font-size:11px;margin:2px 0 0">12 个待处理</p></div><span style="background:#2FA8FF;color:#fff;border-radius:10px;padding:6px 14px;font-size:12px">批量通过</span></div>
              <div style="display:grid;gap:8px"><div style="background:#fff;border-radius:11px;padding:12px 13px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>高中英语词汇 3500</b><span style="color:#99a;font-size:10px">gntv · 09-07</span></div><div style="display:flex;gap:7px;margin-top:8px"><span style="background:#2FBF9B;color:#fff;border-radius:9px;padding:4px 12px;font-size:11px">✓ 通过</span><span style="background:#FF5A5A;color:#fff;border-radius:9px;padding:4px 12px;font-size:11px">✗ 拒绝</span></div></div><div style="background:#fff;border-radius:11px;padding:12px 13px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>小学科学实验合集</b><span style="color:#99a;font-size:10px">alan5914 · 09-07</span></div><div style="display:flex;gap:7px;margin-top:8px"><span style="background:#2FBF9B;color:#fff;border-radius:9px;padding:4px 12px;font-size:11px">✓ 通过</span><span style="background:#FF5A5A;color:#fff;border-radius:9px;padding:4px 12px;font-size:11px">✗ 拒绝</span></div></div></div>
            </div>
            <div style="display:grid;gap:10px">
              <div style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);border-radius:14px;padding:16px;color:#fff"><b style="font-size:14px">📊 今日数据</b><div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-top:10px"><div style="background:rgba(255,255,255,.15);border-radius:10px;padding:10px;text-align:center"><b style="font-size:16px">36</b><p style="font-size:10px;opacity:.85;margin:2px 0 0">新发布</p></div><div style="background:rgba(255,255,255,.15);border-radius:10px;padding:10px;text-align:center"><b style="font-size:16px">5</b><p style="font-size:10px;opacity:.85;margin:2px 0 0">待审核</p></div><div style="background:rgba(255,255,255,.15);border-radius:10px;padding:10px;text-align:center"><b style="font-size:16px">2</b><p style="font-size:10px;opacity:.85;margin:2px 0 0">被举报</p></div><div style="background:rgba(255,255,255,.15);border-radius:10px;padding:10px;text-align:center"><b style="font-size:16px">0</b><p style="font-size:10px;opacity:.85;margin:2px 0 0">申诉</p></div></div></div>
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px">⚠️ 举报中心</b><div style="display:grid;gap:8px;margin-top:8px;font-size:11px"><span style="display:flex;justify-content:space-between"><span style="color:#667">「小学数学思维训练」违规广告</span><b style="color:#FF7A59">处理</b></span><span style="display:flex;justify-content:space-between"><span style="color:#667">「初中物理」命名不规范</span><b style="color:#FF7A59">处理</b></span></div></div>
            </div>
          </div>
''', "E535 · 三折叠 后台-种子"))
L(f3_shell("后台-用户管理", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="display:grid;gap:10px"><div style="background:#fff;border-radius:13px;padding:14px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:14px">👥 用户列表</b><input class="gi" placeholder="搜索用户…" style="width:200px;height:34px;font-size:12px"></div>
              <div style="display:grid;gap:8px"><div style="background:#fff;border-radius:11px;padding:11px 13px;display:flex;gap:10px;align-items:center"><img src="''' + OWL + '''" style="width:32px;height:32px;border-radius:50%"><div style="flex:1"><b style="font-size:12px">gntv</b><p style="color:#99a;font-size:10px;margin:1px 0 0">发布员 · 108.97T</p></div><span style="color:#2FBF9B;font-size:11px">正常</span></div><div style="background:#fff;border-radius:11px;padding:11px 13px;display:flex;gap:10px;align-items:center"><img src="''' + OWL + '''" style="width:32px;height:32px;border-radius:50%"><div style="flex:1"><b style="font-size:12px">dgvge</b><p style="color:#99a;font-size:10px;margin:1px 0 0">贵宾 · 109.697T</p></div><span style="color:#2FBF9B;font-size:11px">正常</span></div><div style="background:#fff;border-radius:11px;padding:11px 13px;display:flex;gap:10px;align-items:center"><img src="''' + OWL + '''" style="width:32px;height:32px;border-radius:50%"><div style="flex:1"><b style="font-size:12px">user_cheat</b><p style="color:#99a;font-size:10px;margin:1px 0 0">User · 0.12 分享率</p></div><span style="color:#FF5A5A;font-size:11px">已封禁</span></div></div>
            </div>
            <div style="display:grid;gap:10px">
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px">📊 用户统计</b><div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-top:10px"><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px;color:#2FA8FF">4,645</b><p style="color:#99a;font-size:10px;margin:2px 0 0">注册用户</p></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px;color:#FF7A59">1,341</b><p style="color:#99a;font-size:10px;margin:2px 0 0">被禁</p></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px;color:#2FBF9B">45</b><p style="color:#99a;font-size:10px;margin:2px 0 0">贵宾</p></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px">39</b><p style="color:#99a;font-size:10px;margin:2px 0 0">未验证</p></div></div></div>
              <div style="background:linear-gradient(135deg,#FFE8E8,#FFF0E8);border-radius:13px;padding:14px"><b style="font-size:13px">⚠️ 违规处理</b><div style="display:grid;gap:8px;margin-top:8px;font-size:11px"><span style="display:flex;justify-content:space-between"><span style="color:#667">「作弊检测」自动封禁 3 人</span><b style="color:#FF5A5A">查看</b></span><span style="display:flex;justify-content:space-between"><span style="color:#667">「分享率过低」警告 8 人</span><b style="color:#FF7A59">查看</b></span></div></div>
            </div>
          </div>
''', "E536 · 三折叠 后台-用户"))
L(f3_shell("后台-公告管理", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="display:grid;gap:10px"><div style="background:#fff;border-radius:13px;padding:15px;display:grid;gap:10px"><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">公告标题 *</label><input class="gi" value="站免池月度进度通报（9月）" style="width:100%;height:40px;font-size:13px"></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">内容 *</label><textarea class="gi" style="width:100%;height:110px;font-size:12px;resize:none">当前站免池进度 52.3%（1,046,000/2,000,000 火花）。当月达 200 万，下月 1-3 号自动开启全局双倍免费促销！大家一起冲鸭 🌱</textarea></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">置顶</label><div style="display:flex;gap:8px">{"".join(chip3(x, on=(i==0)) for i,x in enumerate(["置顶显示","普通")))}</div></div></div>
              <button class="f2-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;padding:13px;border-radius:12px;font-size:15px">📤 发布公告</button>
            </div>
            <div style="display:grid;gap:10px">
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px">📢 已发布</b><div style="display:grid;gap:8px;margin-top:9px;font-size:11px"><span style="display:flex;justify-content:space-between"><span style="color:#667">9月开学季活动公告</span><b style="color:#99a">置顶</b></span><span style="display:flex;justify-content:space-between"><span style="color:#667">勋章墙 24 节气上新</span><b style="color:#99a">09-01</b></span><span style="display:flex;justify-content:space-between"><span style="color:#667">邀请系统规则更新</span><b style="color:#99a">08-28</b></span></div></div>
              <div style="background:linear-gradient(135deg,#E8F8F2,#E8F4FF);border-radius:13px;padding:14px"><b style="font-size:13px">📅 定时发布</b><p style="font-size:11px;color:#667;margin:7px 0 0">中秋活动预告 · 09-15 08:00 定时发布</p></div>
            </div>
          </div>
''', "E537 · 三折叠 后台-公告"))
L(f3_shell("后台-勋章管理", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="display:grid;gap:10px"><div style="background:#fff;border-radius:13px;padding:15px;display:grid;gap:10px"><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">选择勋章 *</label><select class="gi" style="width:100%;height:40px;font-size:13px"><option>开学季限定 · 已上架</option><option>中秋节限定 · 待上架</option><option>签到100天</option></select></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">发放对象</label><textarea class="gi" placeholder="每行一个用户名" style="width:100%;height:80px;font-size:12px;resize:none"></textarea></div><div><label style="font-size:12px;font-weight:600;display:block;margin-bottom:5px">备注</label><input class="gi" placeholder="发放原因…" style="width:100%;height:38px;font-size:13px"></div></div>
              <button class="f2-btn" style="background:#8B5CF6;color:#fff;border:none;padding:13px;border-radius:12px;font-size:15px">🎖️ 发放勋章</button>
            </div>
            <div style="display:grid;gap:10px">
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px">🎖️ 勋章库存</b><div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-top:9px"><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px;color:#8B5CF6">24</b><p style="color:#99a;font-size:10px;margin:2px 0 0">节气系列</p></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px;color:#2FA8FF">18</b><p style="color:#99a;font-size:10px;margin:2px 0 0">学习课堂</p></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px;color:#FF7A59">12</b><p style="color:#99a;font-size:10px;margin:2px 0 0">节日系列</p></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px;color:#2FBF9B">15</b><p style="color:#99a;font-size:10px;margin:2px 0 0">开站系列</p></div></div></div>
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px">📜 发放记录</b><div style="display:grid;gap:8px;margin-top:9px;font-size:11px"><span style="display:flex;justify-content:space-between"><span style="color:#667">gntv · 开学季限定</span><b style="color:#99a">09-01</b></span><span style="display:flex;justify-content:space-between"><span style="color:#667">kiririn · 签到100天</span><b style="color:#99a">08-15</b></span></div></div>
            </div>
          </div>
''', "E538 · 三折叠 后台-勋章"))

# ===== 8. 银行/站免池 ×4 =====
L(f3_shell("银行-主页", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="display:grid;gap:10px"><div style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);border-radius:14px;padding:17px;color:#fff"><p style="font-size:11px;opacity:.9;margin:0">总资产</p><b style="font-size:26px">85,814,940.71</b><div style="display:flex;gap:10px;margin-top:8px;font-size:11px"><span style="background:rgba(255,255,255,.2);border-radius:8px;padding:3px 10px">时魔 +469.1/h</span><span style="background:rgba(255,255,255,.2);border-radius:8px;padding:3px 10px">站内余额 85,811,618.90</span></div></div>
              <div style="display:grid;grid-template-columns:1fr 1fr;gap:9px"><div style="background:#fff;border-radius:12px;padding:13px;text-align:center"><b style="font-size:16px;color:#2FBF9B">3,321.81</b><p style="color:#99a;font-size:10px;margin:2px 0 0">活期余额 · 0.010%</p></div><div style="background:#fff;border-radius:12px;padding:13px;text-align:center"><b style="font-size:16px">0</b><p style="color:#99a;font-size:10px;margin:2px 0 0">在投定期</p></div><div style="background:#fff;border-radius:12px;padding:13px;text-align:center"><b style="font-size:16px;color:#FF7A59">0</b><p style="color:#99a;font-size:10px;margin:2px 0 0">贷款负债</p></div><div style="background:#fff;border-radius:12px;padding:13px;text-align:center"><b style="font-size:16px;color:#2FA8FF">46,924.38</b><p style="color:#99a;font-size:10px;margin:2px 0 0">最大可贷</p></div></div>
            </div>
            <div style="display:grid;gap:10px">
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px">📈 定期利率</b><div style="display:grid;gap:6px;margin-top:9px;font-size:11px"><span style="display:flex;justify-content:space-between"><span style="color:#667">7 天</span><b>0.02%</b></span><span style="display:flex;justify-content:space-between"><span style="color:#667">30 天</span><b>0.05%</b></span><span style="display:flex;justify-content:space-between"><span style="color:#667">90 天</span><b>0.08%</b></span><span style="display:flex;justify-content:space-between"><span style="color:#667">180 天</span><b>0.10%</b></span><span style="display:flex;justify-content:space-between"><span style="color:#667">365 天</span><b>0.12%</b></span></div></div>
              <div style="display:grid;grid-template-columns:1fr 1fr;gap:9px"><button class="f2-btn" style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);color:#fff;border:none;padding:12px;border-radius:11px;font-size:13px">💰 存入定期</button><button class="f2-btn" style="background:#FF7A59;color:#fff;border:none;padding:12px;border-radius:11px;font-size:13px">🏦 申请贷款</button></div>
              <div style="background:#FFF8E8;border-radius:12px;padding:13px;font-size:11px;color:#667;line-height:1.9">💡 最小存款 10,000 火花；提前支取收 1% 手续费；贷款逾期影响信誉</div>
            </div>
          </div>
''', "E539 · 三折叠 银行-主页"))
L(f3_shell("银行-存单详情", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="display:grid;gap:10px"><div style="background:linear-gradient(135deg,#FFC93C,#FF7A59);border-radius:14px;padding:17px;color:#fff"><p style="font-size:11px;opacity:.9;margin:0">存单收益试算</p><b style="font-size:22px">100,000 火花</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">存 30 天 · 利率 0.05% · 到期 +50 火花</p></div>
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px">📋 存单信息</b><div style="display:grid;gap:7px;margin-top:9px;font-size:12px"><span style="display:flex;justify-content:space-between"><span style="color:#99a">存入金额</span><b>100,000</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">期限</span><b>30 天</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">利率</span><b>0.05%</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">到期日</span><b>2026-10-07</b></span></div></div>
            </div>
            <div style="display:grid;gap:10px">
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px">💰 收益预览</b><div style="display:grid;gap:7px;margin-top:9px;font-size:12px"><span style="display:flex;justify-content:space-between"><span style="color:#99a">到期本息</span><b style="color:#2FBF9B">100,050</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">提前支取手续费</span><b style="color:#FF7A59">-1,000 (1%)</b></span></div><div style="background:#E8F8F2;border-radius:10px;padding:10px;margin-top:10px;font-size:11px;color:#2FBF9B">✓ 到期自动转回站内余额</div></div>
              <button class="f2-btn" style="background:linear-gradient(135deg,#FFC93C,#FF7A59);color:#fff;border:none;padding:13px;border-radius:12px;font-size:15px">💰 确认存入</button>
            </div>
          </div>
''', "E540 · 三折叠 银行-存单"))
L(f3_shell("站免池-主页", '''
          <div style="display:grid;grid-template-columns:1fr 1.3fr;gap:12px">
            <div style="display:grid;gap:10px"><div style="background:linear-gradient(135deg,#FF7A59,#FFC93C);border-radius:14px;padding:17px;color:#fff;text-align:center"><p style="font-size:24px;margin:0">💛</p><b style="font-size:17px;display:block;margin-top:4px">站免池</b><p style="font-size:12px;opacity:.9;margin:5px 0 0">当月达 200 万，下月 1-3 号<br>自动开启全局双倍免费促销</p><div style="height:10px;background:rgba(255,255,255,.3);border-radius:5px;overflow:hidden;margin-top:10px"><div style="width:52.3%;height:100%;background:#fff;border-radius:5px"></div></div><p style="font-size:11px;margin-top:5px">1,046,000 / 2,000,000 (52.3%)</p></div>
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px">🏆 捐赠榜 TOP3</b><div style="display:grid;gap:7px;margin-top:9px;font-size:12px"><span style="display:flex;justify-content:space-between"><span>🥇 gntv</span><b>+1,000,000</b></span><span style="display:flex;justify-content:space-between"><span>🥈 dgvge</span><b>+500,000</b></span><span style="display:flex;justify-content:space-between"><span>🥉 kiririn</span><b>+100,000</b></span></div></div>
            </div>
            <div style="display:grid;gap:10px">
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px">🎁 捐赠档位</b><div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-top:10px"><div style="background:#F0F4F8;border-radius:10px;padding:10px;text-align:center"><b style="font-size:14px">1,000</b><p style="color:#99a;font-size:9px;margin:2px 0 0">火花</p></div><div style="background:#F0F4F8;border-radius:10px;padding:10px;text-align:center"><b style="font-size:14px">5,000</b><p style="color:#99a;font-size:9px;margin:2px 0 0">火花</p></div><div style="background:#F0F4F8;border-radius:10px;padding:10px;text-align:center"><b style="font-size:14px">10,000</b><p style="color:#99a;font-size:9px;margin:2px 0 0">火花</p></div><div style="background:#F0F4F8;border-radius:10px;padding:10px;text-align:center"><b style="font-size:14px">50,000</b><p style="color:#99a;font-size:9px;margin:2px 0 0">火花</p></div><div style="background:#F0F4F8;border-radius:10px;padding:10px;text-align:center"><b style="font-size:14px">100,000</b><p style="color:#99a;font-size:9px;margin:2px 0 0">火花</p></div><div style="background:#F0F4F8;border-radius:10px;padding:10px;text-align:center"><b style="font-size:14px">500,000</b><p style="color:#99a;font-size:9px;margin:2px 0 0">火花</p></div></div></div>
              <div style="display:grid;grid-template-columns:1fr 1fr;gap:9px"><button class="f2-btn" style="background:#FF7A59;color:#fff;border:none;padding:12px;border-radius:11px;font-size:14px">💛 我要捐赠</button><button class="f2-btn" style="background:#fff;color:#2FA8FF;border:1px solid #2FA8FF;padding:12px;border-radius:11px;font-size:14px">📜 捐赠记录</button></div>
              <div style="background:#FFF8E8;border-radius:12px;padding:12px;font-size:11px;color:#667;line-height:1.9">💡 当月达 200 万火花后，次月 1-3 号全站种子开启双倍免费下载！</div>
            </div>
          </div>
''', "E541 · 三折叠 站免池-主页"))
L(f3_shell("站免池-捐赠记录", '''
          <div style="display:grid;gap:10px">
            <div style="background:#fff;border-radius:13px;padding:14px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:14px">📜 我的捐赠记录</b><span style="background:#FF7A59;color:#fff;border-radius:10px;padding:6px 14px;font-size:12px">💛 继续捐赠</span></div>
            <div style="display:grid;gap:8px"><div style="background:#fff;border-radius:11px;padding:13px 15px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:13px">站免池捐赠</b><p style="color:#99a;font-size:10px;margin:2px 0 0">2026-09-01 10:23</p></div><b style="font-size:15px;color:#FF7A59">-1,000,000</b></div><div style="background:#fff;border-radius:11px;padding:13px 15px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:13px">站免池捐赠</b><p style="color:#99a;font-size:10px;margin:2px 0 0">2026-08-15 09:12</p></div><b style="font-size:15px;color:#FF7A59">-100,000</b></div><div style="background:#fff;border-radius:11px;padding:13px 15px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:13px">站免池捐赠</b><p style="color:#99a;font-size:10px;margin:2px 0 0">2026-07-20 14:30</p></div><b style="font-size:15px;color:#FF7A59">-50,000</b></div></div>
            <div style="background:linear-gradient(135deg,#FFF8E8,#FFE8F5);border-radius:13px;padding:14px;font-size:12px;color:#B8860B;text-align:center">🏅 累计捐赠 1,150,000 火花 · 全站排名 #1</div>
          </div>
''', "E542 · 三折叠 站免池-记录"))

# ===== 9. 论坛帖子 ×4 =====
for pi, (fn, posts) in enumerate([
    ("本站事务区", [("📢 站免池进度通报：52.3%","管理组 · 09-07 · 89阅"),("📢 中秋双倍免费活动预告","管理组 · 09-15 · 156阅"),("📌 新手必读：站规与常见问题","管理组 · 置顶 · 1,203阅")]),
    ("小学部交流", [("📚 小学语文预习资料分享","study_mom · 09-06 · 45阅"),("✏️ 小学数学思维训练求助","妈妈爱学习 · 09-06 · 32阅"),("🎨 小学美术手工资源合集","手工达人 · 09-05 · 28阅")]),
    ("发邀专区", [("🎁 新手友好发邀（长期）","kiririn · 09-06 · 210阅"),("🎁 高中资源贡献者优先发邀","alan5914 · 09-05 · 178阅")]),
    ("技术交流", [("💻 IYUU辅种保姆级教程","gntv · 09-06 · 356阅"),("💻 qBittorrent 优化设置","rick · 09-05 · 289阅"),("💻 NAS 保种指南","nas玩家 · 09-04 · 198阅")]),
]):
    p_rows = ""
    for pt,au in posts:
        p_rows += f'<div style="background:#fff;border-radius:12px;padding:13px 15px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:13px;flex:1">{pt}</b><span style="color:#99a;font-size:10px;flex-shrink:0">{au}</span></div>'
    L(f3_shell(f"论坛-{fn}", f'''
          <div style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);border-radius:14px;padding:15px;color:#fff;margin-bottom:12px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:16px">{fn}</b><p style="font-size:11px;opacity:.9;margin:2px 0 0">共 120 帖 · 今日 +3</p></div><span style="background:rgba(255,255,255,.25);border-radius:10px;padding:6px 14px;font-size:12px">📝 发帖</span></div>
          <div style="display:grid;gap:9px">{p_rows}</div>
''', f"E54{3+pi} · 三折叠 论坛-{fn}"))

# ===== 10. 农场 ×2 =====
L(f3_shell("农场-全景", '''
          <div style="display:grid;grid-template-columns:1.3fr 1fr;gap:12px">
            <div style="display:grid;gap:10px">
              <div style="background:#fff;border-radius:13px;padding:15px"><b style="font-size:13px">🌾 农作物</b><div style="display:grid;grid-template-columns:repeat(4,1fr);gap:8px;margin-top:10px"><div style="background:#fff;border-radius:11px;padding:12px;text-align:center;border:2px solid #FFF4E0"><span style="font-size:26px">🌾</span><b style="display:block;font-size:11px;margin-top:3px">小麦</b><p style="color:#FF7A59;font-size:10px;margin:2px 0 0">生长中 2h</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🌽</span><b style="display:block;font-size:11px;margin-top:3px">玉米</b><p style="color:#99a;font-size:10px;margin:2px 0 0">空闲</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🥜</span><b style="display:block;font-size:11px;margin-top:3px">花生</b><p style="color:#99a;font-size:10px;margin:2px 0 0">空闲</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🥔</span><b style="display:block;font-size:11px;margin-top:3px">土豆</b><p style="color:#99a;font-size:10px;margin:2px 0 0">空闲</p></div></div></div>
              <div style="background:#fff;border-radius:13px;padding:15px"><b style="font-size:13px">🐔 动物</b><div style="display:grid;grid-template-columns:repeat(4,1fr);gap:8px;margin-top:10px"><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🐔</span><b style="display:block;font-size:11px;margin-top:3px">鸡</b><p style="color:#99a;font-size:10px;margin:2px 0 0">24h · 1,000</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🐷</span><b style="display:block;font-size:11px;margin-top:3px">猪</b><p style="color:#99a;font-size:10px;margin:2px 0 0">48h · 2,000</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🐑</span><b style="display:block;font-size:11px;margin-top:3px">羊</b><p style="color:#99a;font-size:10px;margin:2px 0 0">72h · 5,000</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🐮</span><b style="display:block;font-size:11px;margin-top:3px">牛</b><p style="color:#99a;font-size:10px;margin:2px 0 0">96h · 10,000</p></div></div></div>
            </div>
            <div style="display:grid;gap:10px">
              <div style="background:linear-gradient(135deg,#2FBF9B,#FFC93C);border-radius:14px;padding:16px;color:#fff"><b style="font-size:14px">🥬 菜市场</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">每日 0/4/8/12/16/20 点刷新 ±50%</p><div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-top:10px"><div style="background:rgba(255,255,255,.2);border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px">620</b><p style="font-size:10px;opacity:.9;margin:2px 0 0">小麦 · +24%</p></div><div style="background:rgba(255,255,255,.2);border-radius:10px;padding:10px;text-align:center"><b style="font-size:15px">880</b><p style="font-size:10px;opacity:.9;margin:2px 0 0">玉米 · -12%</p></div></div></div>
              <div style="background:#fff;border-radius:13px;padding:14px"><b style="font-size:13px">📦 我的仓库</b><div style="display:grid;gap:7px;margin-top:9px;font-size:12px"><span style="display:flex;justify-content:space-between"><span style="color:#99a">小麦</span><b>×12</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">玉米</span><b>×8</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">鸡蛋</span><b>×5</b></span></div></div>
              <button class="f2-btn" style="background:#2FBF9B;color:#fff;border:none;padding:12px;border-radius:11px;font-size:14px">🌾 去种植</button>
            </div>
          </div>
''', "E547 · 三折叠 农场-全景"))
L(f3_shell("农场-收获", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:12px">
            <div style="display:grid;gap:10px"><div style="background:#fff;border-radius:13px;padding:15px"><b style="font-size:13px">🌾 可收获</b><div style="display:grid;grid-template-columns:1fr 1fr;gap:9px;margin-top:10px"><div style="background:#E8F8F2;border-radius:12px;padding:13px;text-align:center;border:2px solid #2FBF9B"><span style="font-size:30px">🥔</span><b style="display:block;font-size:12px;margin-top:4px">土豆 · 已成熟</b><p style="color:#2FBF9B;font-size:11px;margin:3px 0 0">🎉 双倍收获 +4,000</p></div><div style="background:#E8F8F2;border-radius:12px;padding:13px;text-align:center;border:2px solid #2FBF9B"><span style="font-size:30px">🌾</span><b style="display:block;font-size:12px;margin-top:4px">小麦 · 已成熟</b><p style="color:#2FBF9B;font-size:11px;margin:3px 0 0">+500</p></div></div></div>
              <div style="background:#fff;border-radius:13px;padding:15px"><b style="font-size:13px">🐔 可收获</b><div style="display:grid;grid-template-columns:1fr 1fr;gap:9px;margin-top:10px"><div style="background:#E8F8F2;border-radius:12px;padding:13px;text-align:center"><span style="font-size:30px">🐔</span><b style="display:block;font-size:12px;margin-top:4px">鸡 · 已成熟</b><p style="color:#2FBF9B;font-size:11px;margin:3px 0 0">+1,000</p></div><div style="background:#fff;border-radius:12px;padding:13px;text-align:center;opacity:.5"><span style="font-size:30px">🐷</span><b style="display:block;font-size:12px;margin-top:4px">猪 · 16h</b></div></div></div>
            </div>
            <div style="display:grid;gap:10px">
              <div style="background:linear-gradient(135deg,#2FBF9B,#FFC93C);border-radius:14px;padding:16px;color:#fff"><b style="font-size:14px">💰 本次收获</b><div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-top:10px"><div style="background:rgba(255,255,255,.2);border-radius:10px;padding:11px;text-align:center"><b style="font-size:18px">+4,500</b><p style="font-size:10px;opacity:.9;margin:2px 0 0">农作物</p></div><div style="background:rgba(255,255,255,.2);border-radius:10px;padding:11px;text-align:center"><b style="font-size:18px">+1,000</b><p style="font-size:10px;opacity:.9;margin:2px 0 0">动物</p></div></div></div>
              <button class="f2-btn" style="background:linear-gradient(135deg,#2FBF9B,#FFC93C);color:#fff;border:none;padding:13px;border-radius:12px;font-size:15px">🌾 全部收获 (+5,500)</button>
              <div style="background:#FFF8E8;border-radius:12px;padding:12px;font-size:11px;color:#667;line-height:1.9">💡 作物有效期 5 天；收获 20% 概率双倍；去菜市场出售可赚差价</div>
            </div>
          </div>
''', "E548 · 三折叠 农场-收获"))

# ===== 11. 其他 ×4 =====
info3("火花收益", "E549 · 三折叠 火花收益", [("💰","基本奖励","74.2 / 小时"),("🏅","勋章加成","69.215 / 小时 · 1.03x"),("📜","官种加成","325.750 / 小时 · 5x"),("🏰","后宫加成","93.680 / 小时 · 0.1x"),("📊","合计","562.845 / 小时"),("📅","今日获得","+13,507")], 3, "#FFF8E8")
info3("勋章详情", "E550 · 三折叠 勋章详情", [("🌸","立春 · 已收集","二十四节气 · 2026-02-04"),("📚","初出茅庐 · 已收集","学习课堂 · 2026-07-16"),("🎓","金榜题名 · 未收集","学习课堂 · 达成条件 1/3"),("🏅","签到100天 · 已收集","学习课堂 · 2026-08-15")], 2, "#E8F4FF")
info3("装饰品中心", "E551 · 三折叠 装饰品", [("🖼️","星光头像框","5,000 火花 · 已拥有"),("🌈","ID 彩虹特效","200,000 火花 · 未购买"),("✨","昵称发光","10,000 火花 · 未购买"),("🎀","学习风背景","3,000 火花 · 未购买"),("🎉","动态头像特效","50,000 火花 · 未购买"),("📛","专属勋章挂件","30,000 火花 · 未购买")], 3, "#FFE8F5")
info3("帮助中心", "E552 · 三折叠 帮助", [("❓","如何获得邀请？","查看发邀规则"),("📥","下载速度慢？","检查做种数与连接"),("💾","如何保种？","保种区教程"),("🔥","分享率怎么算？","上传/下载比值"),("🛡️","账号被禁？","违规申诉渠道"),("📚","课本如何提交？","课本中心规范")], 3, "#E8F4FF")

# ===== 12. 半展开 ×6 =====
def f3_half(lt, lv, rt, rv, label):
    aid = next_id()
    lc = "".join(f'<div style="background:#fff;border-radius:10px;padding:9px 12px;font-size:11px;margin-bottom:7px">{it}</div>' for it in lv)
    rc = "".join(f'<div style="background:#fff;border-radius:10px;padding:9px 12px;font-size:11px;margin-bottom:7px">{it}</div>' for it in rv)
    return f'''    <dc-artboard id="{aid}-f3h" label="{label}" width="900" height="620">
      <div class="fold3-screen" style="flex-direction:row">
        <div style="flex:1.1;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:10px 13px;font-size:13px;border-bottom:1px solid #EEF2F7"><b>{lt}</b></div><div style="padding:11px;flex:1;overflow:hidden">{lc}</div></div>
        <div style="width:12px;background:linear-gradient(180deg,#2FA8FF,#5B6BF5);border-radius:6px;margin:5px 3px"></div>
        <div style="flex:1;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:10px 13px;font-size:13px;border-bottom:1px solid #EEF2F7"><b>{rt}</b></div><div style="padding:11px;flex:1;overflow:hidden">{rc}</div></div>
      </div>
    </dc-artboard>
'''
half_pages = [
    ("游戏战绩", ["<b>⚫ 五子棋</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>18 胜 12 负 · 积分 2,480</p>", "<b>🎫 刮刮乐</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>中奖 8 次 · 今日 7 次</p>", "<b>🎯 九宫格</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>大奖 ×1 · 本月 +50,000</p>", "<b>🎲 猜大小</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>8 胜 2 负 · 本月 +23,400</p>"], "卡牌背包", ["<b>🦉 猫头鹰·N ×4</b>", "<b>📘 古籍·R ×2</b>", "<b>🌈 彩虹·SR ×1</b>", "<b>💎 银河·SSR ×0</b>", "<b>🔄 碎片 18/30</b>"], "E553 · 三折叠 半开-游戏+卡牌"),
    ("课本进度", ["<b>📖 语文 四年级上册</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>已提交 · 审核通过</p>", "<b>📗 数学 三年级上册</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>已提交 · 审核通过</p>", "<b>🔬 科学 五年级上册</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>审核中</p>"], "消息", ["<b>🌟 管理组</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>种子已设为官种 🎉</p>", "<b>💬 alan5914</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>求种回复：已发布 ✓</p>", "<b>🎖️ 系统</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>签到 100 天奖励已发放</p>"], "E554 · 三折叠 半开-课本+消息"),
    ("保种区", ["".join(f'<b style="font-size:10px">{"".join(hc(nm, f"{sz} · {seeds}做种 · {tm}") for nm,ic,c,cat,sz,seeds,tm in PRESERVE[:4])}</b>')], "官种区", ["".join(f'<b style="font-size:10px">{"".join(hc(nm, f"{sz} · {dt}") for nm,sz,dt in OFFICIALS[:5])}</b>')], "E555 · 三折叠 半开-保种+官种"),
    ("浏览", ["<b>识典古籍 六書正譌</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>384MB · 12 做种 · 剩2天</p>", "<b>窗外是蓝星</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>45.6GB · 56 做种 · 剩5天</p>", "<b>人教版语文四年级</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>2.3GB · 34 做种 · 剩1天</p>"], "筛选", ["<b>分类：高中</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>2,100 个种子</p>", "<b>标签：免费</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>1,203 个种子</p>", "<b>排序：做种数</b>"], "E556 · 三折叠 半开-浏览+筛选"),
    ("银行", ["<b>总资产</b><p style='font-size:13px;margin:2px 0 0'>85,814,940.71</p>", "<b>站内余额</b><p style='font-size:13px;margin:2px 0 0'>85,811,618.90</p>", "<b>活期</b><p style='font-size:13px;margin:2px 0 0'>3,321.81</p>", "<b>时魔</b><p style='font-size:13px;margin:2px 0 0'>+469.144/h</p>"], "站免池", ["<b>进度 52.3%</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>1,046,000 / 2,000,000</p>", "<b>距双倍免费</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>还差 954,000</p>", "<b>我的捐赠</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>1,150,000 · #1</p>"], "E557 · 三折叠 半开-银行+站免池"),
    ("任务", ["<b>📥 下载 5 个种子</b><p style='color:#2FBF9B;font-size:10px;margin:2px 0 0'>已完成 ✓</p>", "<b>📤 上传 10 个种子</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>3/10</p>", "<b>💬 论坛发 1 帖</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>未开始</p>", "<b>🎖️ 收集 5 枚勋章</b><p style='color:#99a;font-size:10px;margin:2px 0 0'>已完成 ✓</p>"], "今日活动", ["<b>📊 今日访问 2,226</b>", "<b>🔥 新种子 +36</b>", "<b>💬 新评论 +128</b>", "<b>🎁 待领取奖励 +2</b>"], "E558 · 三折叠 半开-任务+活动"),
]
for lt, lv, rt, rv, lb in half_pages:
    L(f3_half(lt, lv, rt, rv, lb))

# ===== 13. 折叠态 ×6 =====
def f3_closed(title, items, label):
    aid = next_id()
    rows = "".join(f'<div style="background:#fff;border-radius:10px;padding:10px 12px;margin-bottom:7px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:11px">{a}</b><span style="color:#99a;font-size:9px">{b}</span></div>' for a,b in items)
    return f'''    <dc-artboard id="{aid}-f3c" label="{label}" width="600" height="400">
      <div class="fold3-closed">
        <div class="fc3-statusbar"><span>9:41</span><span>📶 🔋</span></div>
        <div class="fc3-topbar"><div class="fc3-logo"><img src="{OWL}" alt=""><b>{title}</b></div><span>🔔</span></div>
        <div style="padding:12px;flex:1;overflow:hidden">{rows}</div>
        <div class="fc3-dock"><span>🏠</span><span>🔍</span><span>📥</span><span>👤</span></div>
      </div>
    </dc-artboard>
'''
closed_pages = [
    ("游戏速览", [("⚫ 五子棋","18胜12负"),("🎫 刮刮乐","今日7次"),("🎯 九宫格","大奖×1"),("🎲 猜大小","+23,400")]),
    ("课本进度", [("📖 语文四上","审核通过"),("📗 数学三上","审核通过"),("🔬 科学五上","审核中")]),
    ("后台待办", [("⏳ 待审种子","12 个"),("⚠️ 待处理举报","2 个"),("👥 新增用户","+18")]),
    ("银行速览", [("💰 总资产","85.8M"),("📈 时魔","+469/h"),("🏦 站免池","52.3%")]),
    ("消息", [("🌟 管理组","官种通知"),("💬 alan5914","求种回复"),("🎖️ 系统","签到奖励")]),
    ("今日运势", [("🍀 学习运势","⭐⭐⭐⭐"),("📚 推荐资源","小学数学思维"),("🎁 待领取","2 项")]),
]
for ci, (ct, items) in enumerate(closed_pages):
    L(f3_closed(ct, items, f"E55{9+ci} · 三折叠 折叠态-{ct}"))

# ===== 14. 分屏 ×4 =====
def f3_split(p1t, p1v, p2t, p2v, p3t, p3v, label):
    aid = next_id()
    def pane(t, v):
        rows = "".join(f'<div style="background:#fff;border-radius:9px;padding:8px 10px;font-size:10px;margin-bottom:6px">{it}</div>' for it in v)
        return f'<div style="flex:1;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:9px 11px;font-size:12px;border-bottom:1px solid #EEF2F7"><b>{t}</b></div><div style="padding:9px;flex:1;overflow:hidden">{rows}</div></div>'
    return f'''    <dc-artboard id="{aid}-f3s" label="{label}" width="1200" height="520">
      <div class="fold3-screen" style="flex-direction:row">
        {pane(p1t,p1v)}<div style="width:8px;background:linear-gradient(180deg,#2FA8FF,#5B6BF5);border-radius:4px;margin:4px 2px"></div>{pane(p2t,p2v)}<div style="width:8px;background:linear-gradient(180deg,#FF7A59,#FFC93C);border-radius:4px;margin:4px 2px"></div>{pane(p3t,p3v)}
      </div>
    </dc-artboard>
'''
split_pages = [
    ("游戏", ["<b>⚫ 五子棋</b><p style='color:#99a;font-size:9px;margin:1px 0 0'>18胜12负</p>", "<b>🎫 刮刮乐</b>", "<b>🎯 九宫格</b>", "<b>🎲 猜大小</b>"], "卡牌", ["<b>🦉 猫头鹰·N ×4</b>", "<b>📘 古籍·R ×2</b>", "<b>🌈 彩虹·SR ×1</b>", "<b>💎 银河·SSR ×0</b>"], "积分", ["<b>总积分</b><p style='font-size:12px;margin:1px 0 0'>12,480</p>", "<b>本月游戏</b><p style='font-size:12px;margin:1px 0 0'>+23,400</p>", "<b>卡牌碎片</b><p style='font-size:12px;margin:1px 0 0'>18/30</p>"], "E565 · 三折叠 分屏-游戏+卡牌+积分"),
    ("银行", ["<b>总资产</b><p style='font-size:12px;margin:1px 0 0'>85.8M</p>", "<b>活期</b><p style='font-size:12px;margin:1px 0 0'>3,321.81</p>", "<b>时魔</b><p style='font-size:12px;margin:1px 0 0'>+469/h</p>"], "站免池", ["<b>进度 52.3%</b>", "<b>距双倍</b><p style='font-size:12px;margin:1px 0 0'>954,000</p>", "<b>我的捐赠 #1</b>"], "商店", ["<b>补签卡</b><p style='font-size:12px;margin:1px 0 0'>1,000/张</p>", "<b>邀请</b><p style='font-size:12px;margin:1px 0 0'>500,000</p>", "<b>改名卡</b><p style='font-size:12px;margin:1px 0 0'>500,000</p>"], "E566 · 三折叠 分屏-银行+站免池+商店"),
    ("课本", ["<b>📖 语文四上</b><p style='color:#99a;font-size:9px;margin:1px 0 0'>审核通过</p>", "<b>📗 数学三上</b><p style='color:#99a;font-size:9px;margin:1px 0 0'>审核通过</p>", "<b>🔬 科学五上</b><p style='color:#99a;font-size:9px;margin:1px 0 0'>审核中</p>"], "论坛", ["<b>📢 站免池通报</b>", "<b>📚 语文预习资料</b>", "<b>🎁 发邀专区</b>"], "消息", ["<b>🌟 管理组</b><p style='color:#99a;font-size:9px;margin:1px 0 0'>官种通知</p>", "<b>💬 alan5914</b>", "<b>🎖️ 系统</b>"], "E567 · 三折叠 分屏-课本+论坛+消息"),
    ("绩校", ["<b>保种员 5T</b><p style='color:#99a;font-size:9px;margin:1px 0 0'>月领 200,000</p>", "<b>操作 28/30</b>", "<b>审核 25/30</b>"], "任务", ["<b>📥 下载5种子 ✓</b>", "<b>📤 上传10种子</b><p style='color:#99a;font-size:9px;margin:1px 0 0'>3/10</p>", "<b>💬 发1帖</b>"], "勋章", ["<b>🌸 立春 ✓</b>", "<b>📚 初出茅庐 ✓</b>", "<b>🎓 金榜题名</b><p style='color:#99a;font-size:9px;margin:1px 0 0'>1/3</p>"], "E568 · 三折叠 分屏-绩校+任务+勋章"),
]
for p1t, p1v, p2t, p2v, p3t, p3v, lb in split_pages:
    L(f3_split(p1t, p1v, p2t, p2v, p3t, p3v, lb))

html = "\n".join(out)
open('/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen/fold3c_boards.html','w',encoding='utf-8').write(html)
print("fold3c boards:", len(out), "last id:", n[0])
