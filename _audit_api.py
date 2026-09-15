import re, io, os

out = io.open(r'D:\FluxTorrent\_audit1.txt', 'w', encoding='utf-8')
bad = []
for root, dirs, files in os.walk(r'D:\FluxTorrent\apps\web'):
    dirs[:] = [d for d in dirs if d not in ('node_modules', '.next')]
    for f in files:
        if not f.endswith(('.ts', '.tsx')):
            continue
        p = os.path.join(root, f)
        src = open(p, encoding='utf-8', errors='replace').read()
        for m in re.finditer(r'api\.(get|post|put|del|getBlob)(?:<[^>]*>)?\(\s*([`"\x27])(/[^`"\x27]*)\2', src):
            method, path = m.group(1), m.group(3)
            if not path.startswith('/api/v1'):
                line = src[:m.start()].count('\n') + 1
                rel = p.replace(r'D:\FluxTorrent\apps\web', 'apps/web').replace('\\', '/')
                bad.append('%s:%d  api.%s(%s)' % (rel, line, method, path))

for x in bad:
    out.write(x + '\n')
out.write('TOTAL: %d\n' % len(bad))
out.close()
print('done', len(bad))
