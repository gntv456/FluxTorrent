# -*- coding: utf-8 -*-
"""双折叠第三批: 用户详情页签/消息会话/游戏状态/课本科目/后台详情/银行存单/论坛帖子/外屏 ~100页"""
import sys
sys.path.insert(0, '/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen')
from data import *

n = [433]
def next_id():
    n[0] += 1
    return f"e{n[0]}"

def f2_shell(title, content, label):
    aid = next_id()
    return f'''    <dc-artboard id="{aid}-f2p" label="{label}" width="720" height="900">
      <div class="fold2-screen">
        <div class="f2-statusbar"><span>9:41</span><span>📶 🔋 100%</span></div>
        <div class="f2-topbar"><div class="f2-logo"><img src="{OWL}" alt=""><b>{title}</b></div><div class="f2-search">🔍 搜索…</div><div class="f2-actions"><img class="f2-avatar" src="{OWL}" alt=""></div></div>
        <div class="f2-content" style="padding:14px">{content}</div>
        <div class="f2-dock"><span>🏠</span><span>🔍</span><span>📥</span><span>💬</span><span>👤</span></div>
      </div>
    </dc-artboard>
'''

def card(icon, bg, name, sub, right=''):
    return f'<div style="display:flex;gap:9px;align-items:center;background:#fff;border-radius:11px;padding:11px 12px"><span style="width:34px;height:34px;border-radius:9px;background:{bg};color:#fff;display:flex;align-items:center;justify-content:center;font-size:13px;flex-shrink:0">{icon}</span><div style="flex:1;min-width:0"><b style="font-size:12px;display:block;white-space:nowrap;overflow:hidden;text-overflow:ellipsis">{name}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">{sub}</p></div>{f"<div style=text-align:right;flex-shrink:0>{right}</div>" if right else ""}</div>'

def chip(t, on=False):
    return f'<span style="background:{"#2FA8FF" if on else "#F0F4F8"};color:{"#fff" if on else "#667"};border-radius:14px;padding:5px 12px;font-size:11px">{t}</span>'

def tag(t, c="#2FA8FF", bg="#E8F4FF"):
    return f'<span style="background:{bg};color:{c};border-radius:8px;padding:1px 6px;font-size:9px">{t}</span>'

out = []
L = out.append

def info_page(title, label, items, icon_bg="#E8F4FF"):
    L(f2_shell(title, f'<div style="display:grid;gap:9px">{"".join(card(ic, icon_bg, a, b) for ic,a,b in items)}</div>', label))

# ===== 1. 用户详情页签 ×8 =====
user_tabs = ["概览","上传种子","做种种子","下载历史","评论","收藏","好友","勋章"]
for ti, tab in enumerate(user_tabs):
    if tab == "概览":
        body = f'''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px;display:flex;align-items:center;gap:10px"><img src="{OWL}" style="width:42px;height:42px;border-radius:50%"><div><b style="font-size:14px">gntv</b><p style="color:#99a;font-size:10px;margin:2px 0 0">发布员 · 加入 2025-07-16</p></div><div style="flex:1;text-align:right"><span style="background:#FFE8F5;color:#FF8FC7;border-radius:9px;padding:3px 10px;font-size:10px">🎖️ 30/81</span></div></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-bottom:10px"><div style="background:#F0F7FF;border-radius:10px;padding:11px;text-align:center"><b style="font-size:16px;color:#2FA8FF">108.97T</b><p style="color:#99a;font-size:9px;margin:1px 0 0">上传</p></div><div style="background:#F0F7FF;border-radius:10px;padding:11px;text-align:center"><b style="font-size:16px;color:#2FBF9B">2.83T</b><p style="color:#99a;font-size:9px;margin:1px 0 0">下载</p></div><div style="background:#F0F7FF;border-radius:10px;padding:11px;text-align:center"><b style="font-size:16px">38.5</b><p style="color:#99a;font-size:9px;margin:1px 0 0">分享率</p></div></div>
          <div style="display:grid;gap:6px;font-size:11px"><div style="display:flex;justify-content:space-between;padding:7px 0;border-bottom:1px solid #F0F4F8"><span style="color:#99a">邮箱</span><b>95836184@qq.com</b></div><div style="display:flex;justify-content:space-between;padding:7px 0;border-bottom:1px solid #F0F4F8"><span style="color:#99a">当前活动</span><b>14,247</b></div><div style="display:flex;justify-content:space-between;padding:7px 0"><span style="color:#99a">连接数</span><b>无限制</b></div></div>'''
    elif tab == "上传种子":
        body = f'<div style="display:grid;gap:8px">{"".join(card(t[1], t[2], t[0][:24]+"…" if len(t[0])>24 else t[0], f"{t[4]} · {t[5]}做种") for t in TORRENTS[:6])}<div style="padding:10px;text-align:center;color:#99a;font-size:11px">共 16,348 个上传种子 · 第 1/2724 页</div></div>'
    elif tab == "做种种子":
        body = f'<div style="display:grid;gap:8px">{"".join(card(t[1], t[2], t[0][:24]+"…" if len(t[0])>24 else t[0], f"做种中 · {t[5]}做种 · 上传 1.2MB/s") for t in TORRENTS[:6])}<div style="padding:10px;text-align:center;color:#99a;font-size:11px">共 14,247 个做种种子</div></div>'
    elif tab == "下载历史":
        body = f'<div style="display:grid;gap:8px">{"".join(card(t[1], t[2], t[0][:24]+"…" if len(t[0])>24 else t[0], f"完成 · 剩余 0 · {t[4]}") for t in TORRENTS[:5])}<div style="padding:10px;text-align:center;color:#99a;font-size:11px">共 282 条下载记录</div></div>'
    elif tab == "评论":
        body = '''
          <div style="display:grid;gap:8px"><div style="background:#fff;border-radius:11px;padding:11px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>识典古籍 六書正譌</b><span style="color:#99a;font-size:9px">09-06</span></div><p style="font-size:11px;color:#667;margin:5px 0 0">资源制作精良，扫描页清晰！</p></div><div style="background:#fff;border-radius:11px;padding:11px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>蓝色星球 第三季</b><span style="color:#99a;font-size:9px">09-03</span></div><p style="font-size:11px;color:#667;margin:5px 0 0">纪录片画质一流，孩子最爱！</p></div><div style="background:#fff;border-radius:11px;padding:11px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>人教版语文四年级</b><span style="color:#99a;font-size:9px">08-28</span></div><p style="font-size:11px;color:#667;margin:5px 0 0">课本齐全，排版精美。</p></div></div>'''
    elif tab == "收藏":
        body = f'<div style="display:grid;gap:8px">{"".join(card("⭐", "#FFF8E8", t[0][:24]+"…" if len(t[0])>24 else t[0], f"收藏于 09-0{6-i}") for i,t in enumerate(TORRENTS[:6]))}</div>'
    elif tab == "好友":
        body = '''
          <div style="display:grid;gap:8px"><div style="background:#fff;border-radius:11px;padding:11px;display:flex;gap:9px;align-items:center"><img src="''' + OWL + '''" style="width:32px;height:32px;border-radius:50%"><div style="flex:1"><b style="font-size:12px">alan5914</b><p style="color:#99a;font-size:9px;margin:1px 0 0">上传 16,348 · 在线</p></div><span style="background:#E8F4FF;color:#2FA8FF;border-radius:8px;padding:3px 9px;font-size:9px">发消息</span></div><div style="background:#fff;border-radius:11px;padding:11px;display:flex;gap:9px;align-items:center"><img src="''' + OWL + '''" style="width:32px;height:32px;border-radius:50%"><div style="flex:1"><b style="font-size:12px">study_mom</b><p style="color:#99a;font-size:9px;margin:1px 0 0">上传 892 · 离线</p></div><span style="background:#E8F4FF;color:#2FA8FF;border-radius:8px;padding:3px 9px;font-size:9px">发消息</span></div><div style="background:#fff;border-radius:11px;padding:11px;display:flex;gap:9px;align-items:center"><img src="''' + OWL + '''" style="width:32px;height:32px;border-radius:50%"><div style="flex:1"><b style="font-size:12px">kiririn</b><p style="color:#99a;font-size:9px;margin:1px 0 0">上传 3,204 · 在线</p></div><span style="background:#E8F4FF;color:#2FA8FF;border-radius:8px;padding:3px 9px;font-size:9px">发消息</span></div></div>'''
    else:  # 勋章
        cards = ""
        for i,(m,nm,ds) in enumerate(MEDALS):
            brd = 'border:2px solid #E8F4FF' if i<4 else 'opacity:.45'
            cards += f'<div style="background:#fff;border-radius:11px;padding:11px;text-align:center;{brd}"><span style="font-size:24px">{m}</span><b style="display:block;font-size:10px;margin-top:3px">{nm}</b></div>'
        body = f'<div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px">{cards}</div><div style="padding:10px;text-align:center;color:#99a;font-size:11px">已收集 30/81 枚勋章</div>'''
    L(f2_shell("用户详情", f'''
          <div style="display:flex;gap:5px;margin-bottom:10px;flex-wrap:wrap">{"".join(chip(x, on=(x==tab)) for x in user_tabs)}</div>
          {body}
''', f"E43{4+ti} · 双折叠 用户详情-{tab}"))

# ===== 2. 消息中心 ×7 =====
L(f2_shell("收件箱", '''
          <div style="display:grid;gap:8px"><div style="background:#fff;border-radius:11px;padding:11px 12px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>🌟 管理组</b><span style="color:#99a;font-size:9px">10:30</span></div><p style="font-size:11px;color:#667;margin:5px 0 0">您的种子「识典古籍 六書正譌」已设为官种，获得 5x 收益加成！</p></div><div style="background:#fff;border-radius:11px;padding:11px 12px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>💬 alan5914</b><span style="color:#99a;font-size:9px">昨天</span></div><p style="font-size:11px;color:#667;margin:5px 0 0">求种回复：已发布 ✓ 快来下载吧</p></div><div style="background:#fff;border-radius:11px;padding:11px 12px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>🎖️ 系统</b><span style="color:#99a;font-size:9px">09-05</span></div><p style="font-size:11px;color:#667;margin:5px 0 0">签到 100 天奖励已发放：10,000 火花 + 勋章</p></div></div>
''', "E442 · 双折叠 收件箱"))
for ci, (un, msgs) in enumerate([
    ("管理组", [("管理组","您的种子已设为官种 🎉", "10:30"), ("我","感谢支持！继续努力 🌱", "10:32"), ("管理组","继续保持高质量发布", "10:35")]),
    ("alan5914", [("alan5914","求种回复：已发布 ✓", "昨天 21:15"), ("我","太感谢啦！马上去下载", "昨天 21:20"), ("alan5914","记得做种哦，保种区福利多", "昨天 21:22")]),
]):
    bubbles = "".join(f'<div style="display:flex;{"justify-content:flex-end" if u=="我" else "justify-content:flex-start"};margin-bottom:8px"><div style="max-width:75%;background:{"linear-gradient(135deg,#2FA8FF,#5B6BF5)" if u=="我" else "#fff"};color:{"#fff" if u=="我" else "#1F2A44"};border-radius:12px;padding:9px 12px;font-size:11px;line-height:1.6"><b style="display:block;font-size:9px;opacity:.7;margin-bottom:2px">{u} · {t}</b>{m}</div></div>' for u,m,t in msgs)
    L(f2_shell(f"会话-{un}", f'<div style="background:#F5FAFF;border-radius:12px;padding:12px;margin-bottom:10px;min-height:340px">{bubbles}</div><div style="background:#fff;border-radius:11px;padding:10px;display:flex;gap:8px"><input class="gi" placeholder="输入消息…" style="flex:1;height:36px;font-size:12px"><span style="background:#2FA8FF;color:#fff;border-radius:9px;padding:0 16px;display:flex;align-items:center;font-size:12px">发送</span></div>', f"E44{3+ci} · 双折叠 会话-{un}"))
info_page("发消息", "E445 · 双折叠 发消息", [("📨","收件人","输入用户名或选择好友"),("📝","主题","输入消息主题"),("💬","内容","输入消息内容…"),("📤","发送","点击发送消息")], "#E8F4FF")
info_page("发件箱", "E446 · 双折叠 发件箱", [("📤","致 alan5914","谢谢资源 · 昨天"),("📤","致 管理组","申请官种 · 09-05"),("📤","致 study_mom","求种讨论 · 09-03")], "#E8F0FF")
info_page("回收站", "E447 · 双折叠 回收站", [("🗑️","系统通知 09-01","已删除 · 可恢复"),("🗑️","系统通知 08-28","已删除 · 可恢复"),("💡","提示","回收站 30 天后自动清空")], "#FFE8E8")
info_page("草稿箱", "E448 · 双折叠 草稿箱", [("📝","致 管理组","关于官种申请… · 草稿 09-06"),("📝","致 kiririn","关于蓝色星球字幕… · 草稿 09-04")], "#FFF8E8")
info_page("消息设置", "E449 · 双折叠 消息设置", [("🔔","站内通知","开启"),("📧","邮件通知","开启"),("📲","手机推送","关闭"),("🔄","提醒频率","即时"),("🔕","免打扰时段","23:00-07:00")], "#E8F4FF")

# ===== 3. 游戏状态变体 ×6 =====
L(f2_shell("五子棋-胜利", '''
          <div style="background:#fff;border-radius:12px;padding:14px;text-align:center;margin-bottom:10px"><p style="font-size:40px;margin:0">🏆</p><b style="font-size:16px;display:block;margin-top:4px">恭喜获胜！</b><p style="color:#99a;font-size:11px;margin:4px 0 0">+120 积分 · 连胜 +1</p></div>
          <div style="background:#F5E8C8;border-radius:8px;padding:14px;margin-bottom:10px"><div style="display:grid;grid-template-columns:repeat(5,1fr);gap:0;max-width:300px;margin:0 auto"><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A;border-radius:50%;box-shadow:inset 0 0 0 3px #2FA8FF"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A;border-radius:50%"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A;border-radius:50%;box-shadow:inset 0 0 0 3px #1F2A44"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A;border-radius:50%;box-shadow:inset 0 0 0 3px #2FA8FF;background:#2FA8FF"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div><div style="aspect-ratio:1;background:#D9B36C;border:1px solid #B8914A"></div></div></div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">⚔️ 再来一局</button>
''', "E450 · 双折叠 五子棋-胜利"))
L(f2_shell("刮刮乐-未中", '''
          <div style="background:linear-gradient(135deg,#FF7A59,#FFC93C);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px;text-align:center"><b style="font-size:16px">🎫 刮刮乐</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">2,000 火花 / 次</p></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-bottom:10px"><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px;color:#fff">🎁</div><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px;color:#fff">🎁</div><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px;color:#fff">🎁</div><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px;color:#fff">🎁</div><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px;color:#fff">🎁</div><div style="background:linear-gradient(135deg,#B8C4D9,#8B9BB8);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:22px;color:#fff">🎁</div></div>
          <div style="background:#FFE8E8;border-radius:10px;padding:10px;font-size:11px;color:#FF5A5A;margin-bottom:10px">😢 很遗憾，没有刮中… 再接再厉！</div>
          <button class="f2-btn" style="background:#FF7A59;color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">🎫 再试一次</button>
''', "E451 · 双折叠 刮刮乐-未中"))
L(f2_shell("刮刮乐-大奖", '''
          <div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px;text-align:center"><b style="font-size:18px">🎉 恭喜中大奖！</b><p style="font-size:12px;opacity:.9;margin:4px 0 0">+50,000 火花</p></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-bottom:10px"><div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:26px">💎</div><div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:26px">💰</div><div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:26px">🎊</div><div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:26px">🎁</div><div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:26px">👑</div><div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:10px;height:70px;display:flex;align-items:center;justify-content:center;font-size:26px">✨</div></div>
          <div style="background:#FFF8E8;border-radius:10px;padding:12px;font-size:12px;color:#B8860B;margin-bottom:10px;text-align:center">🎊 欧气爆棚！刮出「头奖」获得 50,000 火花</div>
          <button class="f2-btn" style="background:#FFB300;color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">🎫 趁热再刮</button>
''', "E452 · 双折叠 刮刮乐-大奖"))
L(f2_shell("猜大小-押注", '''
          <div style="background:linear-gradient(135deg,#2FBF9B,#2FA8FF);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px;text-align:center"><b style="font-size:16px">🎲 猜大小 · 押注中</b></div>
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:12px;color:#8895aa">💰 押注金额</b><input class="gi" value="2,000" style="width:100%;height:42px;font-size:16px;margin-top:6px"><div style="display:flex;gap:6px;margin-top:8px;flex-wrap:wrap">{"".join(chip(x, on=(i==1)) for i,x in enumerate(["500","1,000","2,000","5,000","10,000"]))}</div></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:8px"><div style="background:#E8F4FF;border-radius:11px;padding:13px;text-align:center"><b style="font-size:15px;color:#2FA8FF">押 大</b><p style="color:#99a;font-size:10px;margin:2px 0 0">4-6 点 · 1.95x</p></div><div style="background:#FFE8F5;border-radius:11px;padding:13px;text-align:center"><b style="font-size:15px;color:#FF8FC7">押 小</b><p style="color:#99a;font-size:10px;margin:2px 0 0">1-3 点 · 1.95x</p></div></div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#2FBF9B,#2FA8FF);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">🎲 确认押注 2,000 火花</button>
''', "E453 · 双折叠 猜大小-押注"))
L(f2_shell("卡牌背包", '''
          <div style="background:linear-gradient(135deg,#8B5CF6,#5B6BF5);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:16px">🃏 我的卡牌背包</b><p style="font-size:11px;opacity:.9;margin:3px 0 0">共 12 张 · 碎片 18/30</p></div><span style="background:rgba(255,255,255,.2);border-radius:9px;padding:5px 12px;font-size:11px">🔄 合成</span></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:8px;margin-bottom:10px"><div style="background:linear-gradient(135deg,#E8F4FF,#D6E9FF);border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🦉</span><b style="display:block;font-size:10px;margin-top:3px">猫头鹰·N ×4</b></div><div style="background:linear-gradient(135deg,#E8F8F2,#D6F5EC);border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">📘</span><b style="display:block;font-size:10px;margin-top:3px">古籍·R ×2</b></div><div style="background:linear-gradient(135deg,#FFF8E8,#FFF0D0);border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🌟</span><b style="display:block;font-size:10px;margin-top:3px">星光·R ×1</b></div><div style="background:linear-gradient(135deg,#FFE8F5,#FFD6EC);border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🌈</span><b style="display:block;font-size:10px;margin-top:3px">彩虹·SR ×1</b></div><div style="background:linear-gradient(135deg,#F0E8FF,#E0D0FF);border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">💎</span><b style="display:block;font-size:10px;margin-top:3px">银河·SSR ×0</b></div><div style="background:#fff;border:2px dashed #C8D2E0;border-radius:11px;padding:12px;text-align:center;color:#99a"><span style="font-size:26px">?</span><b style="display:block;font-size:10px;margin-top:3px">未知卡</b></div></div>
          <div style="background:#fff;border-radius:10px;padding:10px;font-size:11px;color:#99a">💡 3 张同阶可合成 1 张高阶卡，合成失败返还碎片</div>
''', "E454 · 双折叠 卡牌背包"))
L(f2_shell("九宫格-结果", '''
          <div style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px;text-align:center"><b style="font-size:16px">🎯 抽奖结果</b></div>
          <div style="display:grid;grid-template-columns:repeat(3,1fr);gap:6px;max-width:320px;margin:0 auto 10px"><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px;opacity:.5">📚</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px;opacity:.5">💰</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px;opacity:.5">🎖️</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px;opacity:.5">🎁</div><div style="background:#FFC93C;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:30px;box-shadow:0 0 20px rgba(255,201,60,.8)">💎</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px;opacity:.5">⭐</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px;opacity:.5">🌈</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px;opacity:.5">🔥</div><div style="background:#fff;border-radius:10px;height:80px;display:flex;align-items:center;justify-content:center;font-size:24px;opacity:.5">🎯</div></div>
          <div style="background:#FFF8E8;border-radius:10px;padding:12px;text-align:center;font-size:12px;color:#B8860B;margin-bottom:10px">💎 指针停在「钻石」！获得 10,000 火花</div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">🎯 再抽一次</button>
''', "E455 · 双折叠 九宫格-结果"))

# ===== 4. 课本科目细分 ×8 =====
sub_pages = [
    ("语文", "📖", "#E8F4FF", [("部编版 一年级上册","32MB · 09-01"),("部编版 二年级上册","34MB · 09-01"),("部编版 三年级上册","38MB · 08-30"),("部编版 四年级上册","41MB · 08-28")]),
    ("数学", "📗", "#E8F8F2", [("人教版 一年级上册","28MB · 09-01"),("人教版 三年级上册","30MB · 09-01"),("人教版 五年级上册","35MB · 08-30")]),
    ("英语", "🔤", "#FFE8F5", [("人教PEP 三年级上册","26MB · 09-01"),("人教PEP 五年级上册","29MB · 09-01"),("外研版 七年级上册","33MB · 08-29")]),
    ("物理", "⚛️", "#E8F0FF", [("人教版 八年级上册","42MB · 09-01"),("人教版 九年级全册","58MB · 08-30")]),
    ("化学", "🧪", "#FFE8E8", [("人教版 九年级上册","45MB · 09-01"),("人教版 九年级下册","47MB · 08-31")]),
    ("历史", "🏛️", "#FFF8E8", [("部编版 七年级上册","38MB · 09-01"),("部编版 八年级上册","40MB · 09-01")]),
    ("地理", "🌍", "#E8F8F2", [("人教版 七年级上册","36MB · 09-01"),("人教版 八年级上册","39MB · 08-30")]),
    ("科学", "🔬", "#E8F4FF", [("教科版 三年级上册","22MB · 09-01"),("教科版 五年级上册","25MB · 08-31")]),
]
for si, (sn, ic, bg, books) in enumerate(sub_pages):
    L(f2_shell(f"课本-{sn}", f'''
          <div style="background:{bg};border-radius:12px;padding:13px;margin-bottom:10px;display:flex;align-items:center;gap:10px"><span style="font-size:26px">{ic}</span><div><b style="font-size:15px">{sn}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">共 {len(books)} 本 · 356 次下载</p></div><span style="margin-left:auto;background:#2FA8FF;color:#fff;border-radius:9px;padding:6px 13px;font-size:11px">📋 我的提交</span></div>
          <div style="display:grid;gap:8px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:11px 12px;display:flex;justify-content:space-between;align-items:center"><div style="display:flex;gap:9px;align-items:center"><span style="font-size:18px">{ic}</span><div><b style="font-size:12px">{nm}</b><p style="color:#99a;font-size:9px;margin:1px 0 0">{sz}</p></div></div><span style="background:#E8F4FF;color:#2FA8FF;border-radius:8px;padding:3px 10px;font-size:10px">下载</span></div>' for nm,sz in books)}</div>
''', f"E45{6+si} · 双折叠 课本-{sn}"))

# ===== 5. 浏览分类×排序组合 ×4 =====
for bi, (cn, sort, col) in enumerate([("小学","热门","#FF8FC7"),("初中","做种数","#2FBF9B"),("高中","大小","#5B6BF5"),("纪录片","最新","#FF7A59")]):
    L(f2_shell("浏览", f'''
          <div style="display:flex;gap:6px;margin-bottom:10px;flex-wrap:wrap">{"".join(chip(x, on=(x==cn)) for x in ["全部","学前教育","小学","初中","职高","高中","教育影音","纪录片"])}</div>
          <div style="display:flex;gap:6px;margin-bottom:10px"><span style="background:#2FA8FF;color:#fff;border-radius:12px;padding:4px 11px;font-size:10px">{sort}</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:4px 11px;font-size:10px">最新</span><span style="background:#F0F4F8;color:#667;border-radius:12px;padding:4px 11px;font-size:10px">免费</span></div>
          <div style="display:grid;gap:8px">{"".join(card(t[1], col, t[0][:24]+"…" if len(t[0])>24 else t[0], f"{t[3]} · {t[4]} · {t[5]}做种", tag(t[6].split("/")[0], bg="#E8F8F2" if t[6].startswith("Free") else "#FFF4E0")) for t in TORRENTS[bi*4:(bi+1)*4])}</div>
          <div style="padding:10px;text-align:center;color:#99a;font-size:11px">共 {[3200,2800,2100,3402][bi]} 个种子</div>
''', f"E46{4+bi} · 双折叠 浏览-{cn}-{sort}"))

# ===== 6. 更多种子详情 ×6 =====
for ti, t in enumerate(TORRENTS[12:18]):
    L(f2_shell("种子详情", f'''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><div style="display:flex;gap:8px;align-items:flex-start"><b style="font-size:14px;flex:1;line-height:1.5">{t[0]}</b><span class="tag tag-free">Free</span></div><p style="color:#99a;font-size:10px;margin:6px 0 0">发布者 gntv · 发布员 · 2026-09-06</p><div style="display:flex;gap:6px;margin-top:8px;flex-wrap:wrap">{"".join(tag(x) for x in t[6].split("/"))}</div></div>
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px"><b style="font-size:12px;color:#8895aa">基本信息</b><div style="display:grid;gap:6px;margin-top:8px;font-size:12px"><span style="display:flex;justify-content:space-between"><span style="color:#99a">大小</span><b>{t[4]}</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">类型</span><b>{t[3]}</b></span><span style="display:flex;justify-content:space-between"><span style="color:#99a">做种/下载</span><b>{t[5]} / 3</b></span></div></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px"><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:16px;color:#2FA8FF">{t[5]}</b><p style="color:#99a;font-size:10px;margin:2px 0 0">做种</p></div><div style="background:#F0F7FF;border-radius:10px;padding:10px;text-align:center"><b style="font-size:16px;color:#2FBF9B">3</b><p style="color:#99a;font-size:10px;margin:2px 0 0">下载</p></div></div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px;margin-bottom:8px">⬇️ 下载种子</button>
          <div style="background:#fff;border-radius:10px;padding:11px;font-size:11px;color:#667">📎 {t[0]}，本站官方制作组整理，欢迎下载做种！</div>
''', f"E46{8+ti} · 双折叠 详情-{t[1]}"))

# ===== 7. 管理后台详情 ×4 =====
L(f2_shell("后台-待审种子", '''
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:14px">⏳ 待审核种子</b><p style="color:#99a;font-size:10px;margin:2px 0 0">12 个待处理</p></div><span style="background:#2FA8FF;color:#fff;border-radius:9px;padding:5px 12px;font-size:11px">批量通过</span></div>
          <div style="display:grid;gap:8px"><div style="background:#fff;border-radius:11px;padding:11px 12px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>高中英语词汇 3500</b><span style="color:#99a;font-size:9px">gntv · 09-07</span></div><div style="display:flex;gap:6px;margin-top:7px"><span style="background:#2FBF9B;color:#fff;border-radius:8px;padding:3px 11px;font-size:10px">✓ 通过</span><span style="background:#FF5A5A;color:#fff;border-radius:8px;padding:3px 11px;font-size:10px">✗ 拒绝</span></div></div><div style="background:#fff;border-radius:11px;padding:11px 12px"><div style="display:flex;justify-content:space-between;font-size:12px"><b>小学科学实验合集</b><span style="color:#99a;font-size:9px">alan5914 · 09-07</span></div><div style="display:flex;gap:6px;margin-top:7px"><span style="background:#2FBF9B;color:#fff;border-radius:8px;padding:3px 11px;font-size:10px">✓ 通过</span><span style="background:#FF5A5A;color:#fff;border-radius:8px;padding:3px 11px;font-size:10px">✗ 拒绝</span></div></div></div>
''', "E474 · 双折叠 后台-待审"))
L(f2_shell("后台-用户列表", '''
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:14px">👥 用户管理</b><input class="gi" placeholder="搜索用户…" style="width:180px;height:32px;font-size:11px"></div>
          <div style="display:grid;gap:8px"><div style="background:#fff;border-radius:11px;padding:10px 12px;display:flex;gap:9px;align-items:center"><img src="''' + OWL + '''" style="width:30px;height:30px;border-radius:50%"><div style="flex:1"><b style="font-size:12px">gntv</b><p style="color:#99a;font-size:9px;margin:1px 0 0">发布员 · 108.97T</p></div><span style="color:#2FBF9B;font-size:10px">正常</span></div><div style="background:#fff;border-radius:11px;padding:10px 12px;display:flex;gap:9px;align-items:center"><img src="''' + OWL + '''" style="width:30px;height:30px;border-radius:50%"><div style="flex:1"><b style="font-size:12px">dgvge</b><p style="color:#99a;font-size:9px;margin:1px 0 0">贵宾 · 109.697T</p></div><span style="color:#2FBF9B;font-size:10px">正常</span></div><div style="background:#fff;border-radius:11px;padding:10px 12px;display:flex;gap:9px;align-items:center"><img src="''' + OWL + '''" style="width:30px;height:30px;border-radius:50%"><div style="flex:1"><b style="font-size:12px">user_cheat</b><p style="color:#99a;font-size:9px;margin:1px 0 0">User · 0.12分享率</p></div><span style="color:#FF5A5A;font-size:10px">已封禁</span></div></div>
''', "E475 · 双折叠 后台-用户"))
L(f2_shell("后台-公告编辑", '''
          <div style="background:#fff;border-radius:12px;padding:14px;display:grid;gap:10px"><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">公告标题 *</label><input class="gi" value="站免池月度进度通报（9月）" style="width:100%;height:38px;font-size:12px"></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">内容 *</label><textarea class="gi" style="width:100%;height:110px;font-size:12px;resize:none">当前站免池进度 52.3%（1,046,000/2,000,000 火花）。当月达 200 万，下月 1-3 号自动开启全局双倍免费促销！大家一起冲鸭 🌱</textarea></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">置顶</label><div style="display:flex;gap:7px">{"".join(chip(x, on=(i==0)) for i,x in enumerate(["置顶显示","普通")))}</div></div></div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px;margin-top:10px">📤 发布公告</button>
''', "E476 · 双折叠 后台-公告"))
L(f2_shell("后台-勋章发放", '''
          <div style="background:#fff;border-radius:12px;padding:14px;display:grid;gap:10px"><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">选择勋章 *</label><select class="gi" style="width:100%;height:38px;font-size:12px"><option>开学季限定 · 已上架</option><option>中秋节限定 · 待上架</option><option>签到100天</option></select></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">发放对象</label><textarea class="gi" placeholder="每行一个用户名" style="width:100%;height:70px;font-size:12px;resize:none"></textarea></div><div><label style="font-size:11px;font-weight:600;display:block;margin-bottom:4px">备注</label><input class="gi" placeholder="发放原因…" style="width:100%;height:36px;font-size:12px"></div></div>
          <button class="f2-btn" style="background:#8B5CF6;color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px;margin-top:10px">🎖️ 发放勋章</button>
''', "E477 · 双折叠 后台-勋章"))

# ===== 8. 银行 ×3 =====
L(f2_shell("银行-存单", '''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:14px">📈 我的存单</b><p style="color:#99a;font-size:10px;margin:3px 0 0">暂无定期存款 · 建议存入获取更高利息</p></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px"><div style="background:#E8F4FF;border-radius:11px;padding:13px;text-align:center"><b style="font-size:18px;color:#2FA8FF">3,321.81</b><p style="color:#99a;font-size:10px;margin:2px 0 0">活期余额</p></div><div style="background:#FFF8E8;border-radius:11px;padding:13px;text-align:center"><b style="font-size:18px;color:#FFC93C">0.010%</b><p style="color:#99a;font-size:10px;margin:2px 0 0">活期利率</p></div></div>
          <div style="background:#fff;border-radius:11px;padding:12px;font-size:11px;color:#667;margin-bottom:10px">💡 最小存款 10,000 火花；定期 7 天利率 0.02%，30 天 0.05%，90 天 0.08%，180 天 0.1%，365 天 0.12%</div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">💰 存入定期</button>
''', "E478 · 双折叠 银行-存单"))
L(f2_shell("银行-贷款", '''
          <div style="background:#fff;border-radius:12px;padding:14px;margin-bottom:10px"><b style="font-size:14px">🏦 申请贷款</b><p style="color:#99a;font-size:10px;margin:3px 0 0">最大可贷 46,924.38 火花</p></div>
          <div style="display:grid;gap:8px;margin-bottom:10px"><div style="background:#fff;border-radius:11px;padding:11px;display:flex;justify-content:space-between;font-size:12px"><span>贷款额度</span><b>0 / 46,924.38</b></div><div style="background:#fff;border-radius:11px;padding:11px;display:flex;justify-content:space-between;font-size:12px"><span>当前负债</span><b style="color:#2FBF9B">0</b></div></div>
          <div style="background:#FFE8E8;border-radius:11px;padding:12px;font-size:11px;color:#C0392B;line-height:1.9;margin-bottom:10px"><b>⚠️ 贷款利率：</b>7天0.08% / 30天0.12% / 90天0.18% / 180天0.2% / 365天0.22%<br>逾期将影响信誉，请按时还款！</div>
          <button class="f2-btn" style="background:#FF7A59;color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">🏦 申请贷款</button>
''', "E479 · 双折叠 银行-贷款"))
L(f2_shell("银行-交易记录", '''
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:14px">📋 交易记录</b><span style="color:#99a;font-size:10px">本月</span></div>
          <div style="display:grid;gap:8px"><div style="background:#fff;border-radius:11px;padding:10px 12px;display:flex;justify-content:space-between;font-size:12px"><div><b>活期结息</b><p style="color:#99a;font-size:9px;margin:2px 0 0">09-07 00:00</p></div><b style="color:#2FBF9B">+4.69</b></div><div style="background:#fff;border-radius:11px;padding:10px 12px;display:flex;justify-content:space-between;font-size:12px"><div><b>火花收益结算</b><p style="color:#99a;font-size:9px;margin:2px 0 0">09-06</p></div><b style="color:#2FBF9B">+13,507</b></div><div style="background:#fff;border-radius:11px;padding:10px 12px;display:flex;justify-content:space-between;font-size:12px"><div><b>商店消费-补签卡</b><p style="color:#99a;font-size:9px;margin:2px 0 0">09-05</p></div><b style="color:#FF5A5A">-1,000</b></div><div style="background:#fff;border-radius:11px;padding:10px 12px;display:flex;justify-content:space-between;font-size:12px"><div><b>站免池捐赠</b><p style="color:#99a;font-size:9px;margin:2px 0 0">09-01</p></div><b style="color:#FF7A59">-1,000,000</b></div></div>
''', "E480 · 双折叠 银行-记录"))

# ===== 9. 站免池 ×2 =====
L(f2_shell("站免池-捐赠", '''
          <div style="background:linear-gradient(135deg,#FF7A59,#FFC93C);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px;text-align:center"><b style="font-size:17px">💛 捐赠站免池</b><p style="font-size:11px;opacity:.9;margin:4px 0 0">当前进度 52.3% · 距双倍免费还差 954,000</p><div style="height:9px;background:rgba(255,255,255,.3);border-radius:5px;overflow:hidden;margin-top:8px"><div style="width:52.3%;height:100%;background:#fff;border-radius:5px"></div></div></div>
          <div style="display:grid;gap:8px;margin-bottom:10px"><div style="background:#fff;border-radius:11px;padding:11px;display:flex;justify-content:space-between;align-items:center;font-size:12px"><b>捐赠金额</b><input class="gi" value="10,000" style="width:140px;height:34px;font-size:13px"></div><div style="display:flex;gap:6px;flex-wrap:wrap">{"".join(chip(x, on=(i==2)) for i,x in enumerate(["1,000","5,000","10,000","50,000","100,000"]))}</div></div>
          <button class="f2-btn" style="background:#FF7A59;color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">💛 确认捐赠 10,000 火花</button>
          <div style="background:#FFF8E8;border-radius:10px;padding:10px;margin-top:10px;font-size:11px;color:#667">🏆 捐赠榜：gntv +1,000,000 · dgvge +500,000 · kiririn +100,000</div>
''', "E481 · 双折叠 站免池-捐赠"))
L(f2_shell("站免池-完成", '''
          <div style="background:linear-gradient(135deg,#FFD54F,#FFB300);border-radius:14px;padding:20px;color:#fff;margin-bottom:10px;text-align:center"><p style="font-size:36px;margin:0">🎉</p><b style="font-size:17px;display:block;margin-top:5px">捐赠成功！</b><p style="font-size:12px;opacity:.9;margin:5px 0 0">感谢你为全站双倍免费助力</p></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:8px;margin-bottom:10px"><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><b style="font-size:16px;color:#FF7A59">10,000</b><p style="color:#99a;font-size:10px;margin:2px 0 0">本次捐赠</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><b style="font-size:16px;color:#2FA8FF">53.3%</b><p style="color:#99a;font-size:10px;margin:2px 0 0">最新进度</p></div></div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#2FA8FF,#5B6BF5);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">🔙 返回站免池</button>
''', "E482 · 双折叠 站免池-完成"))

# ===== 10. 论坛帖子列表 ×4 =====
forum_posts = [
    ("本站事务区", [("📢 站点新公告：站免池进度通报","管理组 · 09-07 · 89阅"),("📢 中秋双倍免费活动预告","管理组 · 09-15 · 156阅")]),
    ("小学部交流", [("📚 小学语文预习资料分享","study_mom · 09-06 · 45阅"),("✏️ 小学数学思维训练求助","妈妈爱学习 · 09-06 · 32阅")]),
    ("发邀专区", [("🎁 新手友好发邀（长期）","kiririn · 09-06 · 210阅"),("🎁 高中资源贡献者优先发邀","alan5914 · 09-05 · 178阅")]),
    ("技术交流", [("💻 IYUU辅种保姆级教程","gntv · 09-06 · 356阅"),("💻 qBittorrent 优化设置","rick · 09-05 · 289阅")]),
]
for pi, (fn, posts) in enumerate(forum_posts):
    L(f2_shell(f"论坛-{fn}", f'''
          <div style="background:#fff;border-radius:12px;padding:12px;margin-bottom:10px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:14px">{fn}</b><span style="color:#99a;font-size:10px">共 120 帖</span></div>
          <div style="display:grid;gap:8px">{"".join(f'<div style="background:#fff;border-radius:11px;padding:11px 12px"><b style="font-size:12px">{pt}</b><p style="color:#99a;font-size:10px;margin:4px 0 0">{au}</p></div>' for pt,au in posts)}</div>
''', f"E48{3+pi} · 双折叠 论坛-{fn}"))

# ===== 11. 农场状态 ×3 =====
L(f2_shell("农场-种植中", '''
          <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:8px;margin-bottom:10px"><div style="background:#fff;border-radius:11px;padding:12px;text-align:center;border:2px solid #FFF4E0"><span style="font-size:26px">🌾</span><b style="display:block;font-size:11px;margin-top:3px">小麦</b><p style="color:#FF7A59;font-size:10px;margin:2px 0 0">生长中 2h</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🌽</span><b style="display:block;font-size:11px;margin-top:3px">玉米</b><p style="color:#99a;font-size:10px;margin:2px 0 0">空闲</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🥜</span><b style="display:block;font-size:11px;margin-top:3px">花生</b><p style="color:#99a;font-size:10px;margin:2px 0 0">空闲</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🥔</span><b style="display:block;font-size:11px;margin-top:3px">土豆</b><p style="color:#99a;font-size:10px;margin:2px 0 0">空闲</p></div></div>
          <div style="background:#FFF8E8;border-radius:10px;padding:11px;font-size:11px;color:#667;margin-bottom:10px">⏳ 小麦将于 2 小时后成熟，成熟后可收获 500 火花（20% 概率双倍）</div>
          <button class="f2-btn" style="background:#2FBF9B;color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">🌾 种植下一块地</button>
''', "E487 · 双折叠 农场-种植中"))
L(f2_shell("农场-收获", '''
          <div style="display:grid;grid-template-columns:repeat(4,1fr);gap:8px;margin-bottom:10px"><div style="background:#fff;border-radius:11px;padding:12px;text-align:center;border:2px solid #E8F8F2"><span style="font-size:26px">🥔</span><b style="display:block;font-size:11px;margin-top:3px">土豆</b><p style="color:#2FBF9B;font-size:10px;margin:2px 0 0">已成熟🎉</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center;border:2px solid #E8F8F2"><span style="font-size:26px">🌾</span><b style="display:block;font-size:11px;margin-top:3px">小麦</b><p style="color:#2FBF9B;font-size:10px;margin:2px 0 0">已成熟🎉</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🌽</span><b style="display:block;font-size:11px;margin-top:3px">玉米</b><p style="color:#99a;font-size:10px;margin:2px 0 0">生长中 5h</p></div><div style="background:#fff;border-radius:11px;padding:12px;text-align:center"><span style="font-size:26px">🥜</span><b style="display:block;font-size:11px;margin-top:3px">花生</b><p style="color:#99a;font-size:10px;margin:2px 0 0">生长中 9h</p></div></div>
          <div style="background:#E8F8F2;border-radius:10px;padding:11px;font-size:11px;color:#2FBF9B;margin-bottom:10px">🎉 土豆双倍收获！+4,000 火花 · 小麦收获 +500 火花</div>
          <button class="f2-btn" style="background:linear-gradient(135deg,#2FBF9B,#FFC93C);color:#fff;border:none;width:100%;padding:12px;border-radius:10px;font-size:14px">🌾 收获全部</button>
''', "E488 · 双折叠 农场-收获"))
L(f2_shell("菜市场-出售", '''
          <div style="background:linear-gradient(135deg,#2FBF9B,#FFC93C);border-radius:14px;padding:16px;color:#fff;margin-bottom:10px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:16px">🥬 菜市场出售</b><p style="font-size:11px;opacity:.9;margin:3px 0 0">下次刷新 12:00 · 波动 ±50%</p></div><span style="background:rgba(255,255,255,.25);border-radius:9px;padding:5px 12px;font-size:11px">🔄 刷新</span></div>
          <div style="display:grid;gap:8px;margin-bottom:10px"><div style="background:#fff;border-radius:11px;padding:11px 12px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:12px">🌾 小麦 ×12</b><p style="color:#99a;font-size:9px;margin:2px 0 0">今日收购价 620（+24%）</p></div><span style="background:#2FBF9B;color:#fff;border-radius:8px;padding:4px 12px;font-size:10px">出售</span></div><div style="background:#fff;border-radius:11px;padding:11px 12px;display:flex;justify-content:space-between;align-items:center"><div><b style="font-size:12px">🌽 玉米 ×8</b><p style="color:#99a;font-size:9px;margin:2px 0 0">今日收购价 880（-12%）</p></div><span style="background:#2FBF9B;color:#fff;border-radius:8px;padding:4px 12px;font-size:10px">出售</span></div></div>
          <div style="background:#FFF8E8;border-radius:10px;padding:10px;font-size:11px;color:#667">💰 预计收入：7,440 + 7,040 = 14,480 火花</div>
''', "E489 · 双折叠 菜市场-出售"))

# ===== 12. 其他 ×4 =====
info_page("火花收益明细", "E490 · 双折叠 收益明细", [("💰","基本奖励","74.2 / 小时"),("🏅","勋章加成","69.215 / 小时 · 1.03x"),("📜","官种加成","325.750 / 小时 · 5x"),("🏰","后宫加成","93.680 / 小时 · 0.1x"),("📊","合计","562.845 / 小时")], "#FFF8E8")
info_page("勋章详情", "E491 · 双折叠 勋章详情", [("🏅","签到 100 天勋章","条件：连续签到 100 天"),("🎁","奖励","10,000 火花"),("📅","获得时间","2026-08-15"),("🎨","分类","学习课堂")], "#E8F4FF")
info_page("卡牌详情", "E492 · 双折叠 卡牌详情", [("📘","古籍·R","2 张 · 可用于合成"),("🎯","获取方式","开卡包/合成/活动"),("💡","用途","收藏与合成 SSR"),("🔄","碎片","18/30 · 可兑换卡包")], "#FFE8F5")
info_page("装饰品购买", "E493 · 双折叠 装饰购买", [("🖼️","星光头像框","5,000 火花 · 已有"),("🌈","ID 彩虹特效","200,000 火花 · 未购买"),("✨","昵称发光","10,000 火花 · 未购买"),("🎀","学习风背景","3,000 火花 · 未购买")], "#FFE8F5")

# ===== 13. 外屏补充 ×8 =====
def fc_shell(title, content, label, dock_icon="🔍"):
    aid = next_id()
    return f'''    <dc-artboard id="{aid}-f2c" label="{label}" width="340" height="720">
      <div class="fold2-closed">
        <div class="fc-statusbar"><span>9:41</span><span>📶 🔋</span></div>
        <div class="fc-topbar"><div class="fc-logo"><img src="{OWL}" alt=""><b>{title}</b></div><span>🔔</span></div>
        <div class="fc-scroll">{content}</div>
        <div class="fc-dock"><span>🏠</span><span style="background:#2FA8FF;color:#fff;border-radius:10px">{dock_icon}</span><span>📥</span><span>👤</span></div>
      </div>
    </dc-artboard>
'''
L(fc_shell("绩校", '''
          <div style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);border-radius:12px;padding:14px;color:#fff;margin-bottom:8px"><p style="font-size:10px;opacity:.9;margin:0">保种员 5T 版</p><b style="font-size:20px">200,000</b><p style="font-size:9px;opacity:.9;margin:2px 0 0">火花/月</p></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:7px;margin-bottom:8px"><div style="background:#fff;border-radius:10px;padding:9px;text-align:center"><b style="font-size:14px;color:#2FA8FF">28/30</b><p style="color:#99a;font-size:9px;margin:1px 0 0">操作总数</p></div><div style="background:#fff;border-radius:10px;padding:9px;text-align:center"><b style="font-size:14px;color:#2FBF9B">25/30</b><p style="color:#99a;font-size:9px;margin:1px 0 0">通过审核</p></div></div>
''', "E494 · 双折叠 外屏-绩效", "📊"))
L(fc_shell("任务", '''
          <div style="display:grid;gap:7px"><div style="background:#fff;border-radius:10px;padding:10px 11px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:11px">📥 下载 5 个种子</b><span style="background:#2FBF9B;color:#fff;border-radius:8px;padding:2px 8px;font-size:9px">✓</span></div><div style="background:#fff;border-radius:10px;padding:10px 11px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:11px">📤 上传 10 个种子</b><span style="background:#FFC93C;color:#fff;border-radius:8px;padding:2px 8px;font-size:9px">3/10</span></div><div style="background:#fff;border-radius:10px;padding:10px 11px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:11px">💬 论坛发 1 帖</b><span style="background:#F0F4F8;color:#99a;border-radius:8px;padding:2px 8px;font-size:9px">未开始</span></div></div>
''', "E495 · 双折叠 外屏-任务", "📋"))
L(fc_shell("消息", '''
          <div style="display:grid;gap:7px"><div style="background:#fff;border-radius:10px;padding:10px 11px"><b style="font-size:11px">🌟 管理组</b><p style="color:#99a;font-size:9px;margin:3px 0 0">您的种子已设为官种 🎉</p></div><div style="background:#fff;border-radius:10px;padding:10px 11px"><b style="font-size:11px">💬 alan5914</b><p style="color:#99a;font-size:9px;margin:3px 0 0">求种回复：已发布 ✓</p></div><div style="background:#fff;border-radius:10px;padding:10px 11px"><b style="font-size:11px">🎖️ 系统</b><p style="color:#99a;font-size:9px;margin:3px 0 0">签到 100 天奖励已发放</p></div></div>
''', "E496 · 双折叠 外屏-消息", "💬"))
L(fc_shell("农场", '''
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:7px;margin-bottom:7px"><div style="background:#fff;border-radius:10px;padding:10px;text-align:center"><span style="font-size:20px">🌾</span><b style="display:block;font-size:10px;margin-top:2px">小麦</b><p style="color:#FF7A59;font-size:9px;margin:1px 0 0">2h成熟</p></div><div style="background:#fff;border-radius:10px;padding:10px;text-align:center"><span style="font-size:20px">🥔</span><b style="display:block;font-size:10px;margin-top:2px">土豆</b><p style="color:#2FBF9B;font-size:9px;margin:1px 0 0">已成熟</p></div></div>
          <div style="background:#E8F8F2;border-radius:10px;padding:9px;font-size:9px;color:#2FBF9B">🎉 土豆双倍 +4,000 火花，快去收获！</div>
''', "E497 · 双折叠 外屏-农场", "🌾"))
L(fc_shell("游戏", '''
          <div style="display:grid;gap:7px"><div style="background:#fff;border-radius:10px;padding:10px 11px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:11px">⚫ 五子棋</b><span style="color:#99a;font-size:9px">18胜12负</span></div><div style="background:#fff;border-radius:10px;padding:10px 11px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:11px">🎫 刮刮乐</b><span style="color:#99a;font-size:9px">中奖8次</span></div><div style="background:#fff;border-radius:10px;padding:10px 11px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:11px">🎯 九宫格</b><span style="color:#99a;font-size:9px">大奖×1</span></div><div style="background:#fff;border-radius:10px;padding:10px 11px;display:flex;justify-content:space-between;align-items:center"><b style="font-size:11px">🎲 猜大小</b><span style="color:#99a;font-size:9px">+3,200</span></div></div>
''', "E498 · 双折叠 外屏-游戏", "🎮"))
L(fc_shell("勋章", '''
          <div style="display:grid;grid-template-columns:1fr 1fr 1fr;gap:7px"><div style="background:#fff;border-radius:10px;padding:9px;text-align:center;border:2px solid #E8F4FF"><span style="font-size:18px">🌸</span><b style="display:block;font-size:9px;margin-top:1px">立春</b></div><div style="background:#fff;border-radius:10px;padding:9px;text-align:center;border:2px solid #E8F4FF"><span style="font-size:18px">📚</span><b style="display:block;font-size:9px;margin-top:1px">初出</b></div><div style="background:#fff;border-radius:10px;padding:9px;text-align:center;opacity:.4"><span style="font-size:18px">🎓</span><b style="display:block;font-size:9px;margin-top:1px">学霸</b></div></div>
          <div style="background:#fff;border-radius:10px;padding:9px;margin-top:7px;font-size:10px;text-align:center">🎖️ 30/81 已收集 · 37%</div>
''', "E499 · 双折叠 外屏-勋章", "🎖️"))
L(fc_shell("银行", '''
          <div style="background:linear-gradient(135deg,#5B6BF5,#2FA8FF);border-radius:12px;padding:13px;color:#fff;margin-bottom:8px"><p style="font-size:10px;opacity:.9;margin:0">总资产</p><b style="font-size:20px">85,814,940</b><p style="font-size:9px;opacity:.9;margin:2px 0 0">时魔 +469.1/h</p></div>
          <div style="display:grid;grid-template-columns:1fr 1fr;gap:7px"><div style="background:#fff;border-radius:10px;padding:9px;text-align:center"><b style="font-size:13px">3,321.81</b><p style="color:#99a;font-size:9px;margin:1px 0 0">活期</p></div><div style="background:#fff;border-radius:10px;padding:9px;text-align:center"><b style="font-size:13px">0</b><p style="color:#99a;font-size:9px;margin:1px 0 0">定期</p></div></div>
''', "E500 · 双折叠 外屏-银行", "🏦"))

# ===== 14. 半开补充 ×4 =====
def f2_split(left_title, left_items, right_title, right_items, label):
    aid = next_id()
    lc = "".join(f'<div style="background:#fff;border-radius:9px;padding:8px 10px;font-size:10px;margin-bottom:6px">{it}</div>' for it in left_items)
    rc = "".join(f'<div style="background:#fff;border-radius:9px;padding:8px 10px;font-size:10px;margin-bottom:6px">{it}</div>' for it in right_items)
    return f'''    <dc-artboard id="{aid}-f2s" label="{label}" width="720" height="900">
      <div class="fold2-screen" style="flex-direction:row">
        <div style="flex:1;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:9px 12px;font-size:12px;border-bottom:1px solid #EEF2F7"><b>{left_title}</b></div><div style="padding:10px;flex:1;overflow:hidden">{lc}</div></div>
        <div style="width:10px;background:linear-gradient(180deg,#2FA8FF,#5B6BF5);border-radius:5px;margin:4px 2px"></div>
        <div style="flex:1;background:#F5FAFF;display:flex;flex-direction:column;overflow:hidden"><div style="background:#fff;padding:9px 12px;font-size:12px;border-bottom:1px solid #EEF2F7"><b>{right_title}</b></div><div style="padding:10px;flex:1;overflow:hidden">{rc}</div></div>
      </div>
    </dc-artboard>
'''
split_pages = [
    ("绩校", ["<b>保种员 5T 版</b><p style='color:#FF7A59;font-size:10px;margin:2px 0 0'>月领 200,000</p>", "<b>操作总数</b><p style='color:#2FA8FF;font-size:11px;margin:2px 0 0'>28/30</p>", "<b>通过审核</b><p style='color:#2FBF9B;font-size:11px;margin:2px 0 0'>25/30</p>"], "任务", ["<b>📥 下载5种子</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>已完成 ✓</p>", "<b>📤 上传10种子</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>3/10</p>", "<b>💬 发1帖</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>未开始</p>"], "E501 · 双折叠 半开-绩效+任务"),
    ("勋章", ["<b>🌸 立春</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>已收集</p>", "<b>📚 初出茅庐</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>已收集</p>", "<b>🎓 金榜题名</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>未收集</p>"], "卡牌", ["<b>🦉 猫头鹰·N ×4</b>", "<b>📘 古籍·R ×2</b>", "<b>🌈 彩虹·SR ×1</b>"], "E502 · 双折叠 半开-勋章+卡牌"),
    ("浏览", ["<b>识典古籍 六書正譌</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>384MB · 12做种</p>", "<b>窗外是蓝星</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>45.6GB · 56做种</p>"], "筛选", ["<b>分类：初中</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>3,421 个种子</p>", "<b>标签：免费</b><p style='color:#99a;font-size:9px;margin:2px 0 0'>1,203 个种子</p>"], "E503 · 双折叠 半开-浏览+筛选"),
    ("我的数据", ["<b>上传</b><p style='font-size:12px;margin:2px 0 0'>108.97T</p>", "<b>下载</b><p style='font-size:12px;margin:2px 0 0'>2.83T</p>", "<b>分享率</b><p style='font-size:12px;margin:2px 0 0'>38.5</p>"], "收益", ["<b>当前火花</b><p style='font-size:12px;margin:2px 0 0'>85,815,181</p>", "<b>每小时</b><p style='font-size:12px;margin:2px 0 0'>+562.845</p>", "<b>今日获得</b><p style='font-size:12px;margin:2px 0 0'>+13,507</p>"], "E504 · 双折叠 半开-数据+收益"),
]
for lt, lv, rt, rv, lb in split_pages:
    L(f2_split(lt, lv, rt, rv, lb))

html = "\n".join(out)
open('/home/user/.super_doubao/super-doubao-runtime/workspace/hxpt-theme-redesign/gen/fold2c_boards.html','w',encoding='utf-8').write(html)
print("fold2c boards:", len(out), "last id:", n[0])
