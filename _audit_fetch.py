import re, io, os

out = io.open(r'D:\FluxTorrent\_audit2.txt', 'w', encoding='utf-8')
hits = []
for root, dirs, files in os.walk(r'D:\FluxTorrent\apps\web'):
    dirs[:] = [d for d in dirs if d not in ('node_modules', '.next')]
    for f in files:
        if not f.endswith(('.ts', '.tsx')):
            continue
        p = os.path.join(root, f)
        src = open(p, encoding='utf-8', errors='replace').read()
        for m in re.finditer(r'\bfetch\(', src):
            line = src[:m.start()].count('\n') + 1
            seg = src[m.start():m.start() + 220].replace('\n', ' ')
            rel = p.replace(r'D:\FluxTorrent\apps\web', 'apps/web').replace('\\', '/')
            hits.append('%s:%d  %s' % (rel, line, seg))

for x in hits:
    out.write(x + '\n\n')
out.write('TOTAL: %d\n' % len(hits))
out.close()
print('done', len(hits))
