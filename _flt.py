import json, io

a = json.load(open(r'D:\FluxTorrent\_t1.json', encoding='utf-8'))['data']
b = json.load(open(r'D:\FluxTorrent\_t2.json', encoding='utf-8'))['data']
o = io.open(r'D:\FluxTorrent\_flt.txt', 'w', encoding='utf-8')
o.write('unfiltered: total=%s rows=%d\n' % (a['total_estimate'], len(a['items'])))
o.write('sec_media=60(PC): total=%s rows=%d names=%s\n' % (
    b['total_estimate'], len(b['items']),
    ','.join(i['name'][:18] for i in b['items'][:3])))
o.close()
print('ok')
