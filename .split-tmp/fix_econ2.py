import io, os, re, subprocess

base = os.path.join('D:', os.sep, 'FluxTorrent', 'apps', 'api', 'src', 'economy_http')

def cargo_unused():
    r = subprocess.run(['cargo', 'check', '-p', 'flux-api', '--message-format=short'],
                       capture_output=True, text=True, cwd=os.path.join('D:', os.sep, 'FluxTorrent'))
    out = r.stdout + r.stderr
    fixes = {}
    for m in re.finditer(r'([^\s:]+\.rs):\d+:\d+: warning: unused imports?: `([^`]+)`', out):
        path, syms = m.group(1), m.group(2)
        path = path.replace(chr(92), '/')
        if '/economy_http/' not in path:
            continue
        for sym in [s.strip() for s in syms.split(',')]:
            fixes.setdefault(os.path.basename(path), set()).add(sym)
    return fixes

for _round in range(8):
    fixes = cargo_unused()
    if not fixes:
        print('clean after round', _round)
        break
    for fname, syms in fixes.items():
        p = os.path.join(base, fname)
        if not os.path.exists(p):
            continue
        s = io.open(p, encoding='utf-8').read()
        lines = s.split('\n')
        out = []
        for l in lines:
            t = l.strip()
            handled = False
            m = re.match(r'use ([\w:]+)::\{([^}]*)\};', t)
            if m:
                root = m.group(1)
                names = [x.strip() for x in m.group(2).split(',')]
                keep = [n for n in names if n not in syms and n != 'self']
                if keep:
                    out.append(l[:len(l)-len(t)] + 'use %s::{%s};' % (root, ', '.join(keep)))
                handled = True
            if not handled:
                m2 = re.match(r'use ([\w:]+)::(\w+);', t)
                if m2 and m2.group(2) in syms:
                    handled = True
            if not handled:
                out.append(l)
        io.open(p, 'w', encoding='utf-8', newline='\n').write('\n'.join(out))
    print('round', _round, sum(len(v) for v in fixes.values()))
print('done')
