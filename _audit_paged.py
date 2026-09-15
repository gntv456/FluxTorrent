import re, io, os

out = io.open(r'D:\FluxTorrent\_audit3.txt', 'w', encoding='utf-8')
bad_paged = []
other_http = []
for root, dirs, files in os.walk(r'D:\FluxTorrent\apps\web'):
    dirs[:] = [d for d in dirs if d not in ('node_modules', '.next')]
    for f in files:
        if not f.endswith(('.ts', '.tsx')):
            continue
        p = os.path.join(root, f)
        src = open(p, encoding='utf-8', errors='replace').read()
        rel = p.replace(r'D:\FluxTorrent\apps\web', 'apps/web').replace('\\', '/')
        # paged() calls: resource must start with /api/v1
        for m in re.finditer(r'paged<[^>]*>\(\s*[`"\x27]([^`"\x27]+)', src):
            path = m.group(1)
            if not path.startswith('/api/v1'):
                line = src[:m.start()].count('\n') + 1
                bad_paged.append('%s:%d  paged(%s)' % (rel, line, path))
        # other external-looking http calls in server code
        for m in re.finditer(r'https?://(?!localhost|127\.0\.0\.1|api:8080)[^\s"\'`)]+', src):
            line = src[:m.start()].count('\n') + 1
            other_http.append('%s:%d  %s' % (rel, line, m.group(0)[:100]))

for x in bad_paged:
    out.write('PAGED-PREFIX: ' + x + '\n')
for x in other_http:
    out.write('HTTP-URL: ' + x + '\n')
out.write('paged_bad=%d http=%d\n' % (len(bad_paged), len(other_http)))
out.close()
print('done')
