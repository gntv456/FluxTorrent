import json, io

d = json.load(open(r'D:\FluxTorrent\_sd2.json', encoding='utf-8'))['data']
media = d.get('media', [])
o = io.open(r'D:\FluxTorrent\_mid.txt', 'w', encoding='utf-8')
o.write('media rows: %s\n' % json.dumps([(r['id'], r['name']) for r in media], ensure_ascii=False))
o.close()
print('ok')
