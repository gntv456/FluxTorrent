import re
c=open(r'D:\FluxTorrent\design-system\FluxTorrent-UI设计方案.html',encoding='utf-8').read()
c2=re.sub(r'<!--.*?-->','',c,flags=re.S)
opens=len(re.findall(r'<div\b',c2))
closes=len(re.findall(r'</div>',c2))
print('open',opens,'close',closes,'diff',opens-closes)
for m in re.finditer(r's-num">(\d+)</span><span class="s-title">([^<]+)',c):
    print(m.group(1),m.group(2))
