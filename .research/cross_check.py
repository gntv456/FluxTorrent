import re, os, glob

# ---- 后端路由表 ----
routes = []  # (method, path)
for line in open('.research/routes_table.tsv', encoding='utf-8'):
    m, p, fn, f = line.rstrip('\n').split('\t')
    routes.append((m, p))

def norm_backend(p):
    # {id} -> *
    segs = [s if not (s.startswith('{') and s.endswith('}')) else '*' for s in p.split('?')[0].split('/')]
    return '/'.join(segs)

backend = set()
bymethod = {}
for m, p in routes:
    n = norm_backend(p)
    backend.add((m, n))
    bymethod.setdefault(n, set()).add(m)

# ---- 前端调用点 ----
fe = set()
raw = []
os.chdir('apps/web')
for f in glob.glob('app/**/*.tsx', recursive=True) + glob.glob('app/**/*.ts', recursive=True) + \
         glob.glob('components/*.tsx') + glob.glob('components/*.ts') + glob.glob('lib/*.ts'):
    src = open(f, encoding='utf-8').read()
    # api.get/post/put/del/getBlob + template literals and strings
    for m in re.finditer(r'api\.(get|post|put|del|getBlob)(?:<[^>]*>)?\(\s*([`"\'])(/api/v1[^`"\']*)\2', src):
        meth = {'get':'GET','post':'POST','put':'PUT','del':'DELETE','getBlob':'GET'}[m.group(1)]
        path = m.group(3)
        raw.append((meth, path, f))
    # api.call("METHOD /path")
    for m in re.finditer(r'api\.call(?:<[^>]*>)?\(\s*([`"\'])(GET|POST|PUT|DELETE) (/api/v1[^`"\']*)\1', src):
        raw.append((m.group(2), m.group(3), f))
    # paged<T>("/api/v1/...")
    for m in re.finditer(r'paged(?:<[^>]*>)?\(\s*([`"\'])(/api/v1[^`"\']*)\1', src):
        raw.append(('GET', m.group(2), f))
    # 裸 fetch
    for m in re.finditer(r'fetch\((?:[^)]*)?([`"\'])(\/api\/v1[^`"\']*)\1', src):
        raw.append(('GET', m.group(2), f))

def norm_fe(p):
    p = re.sub(r'\$\{[^}]*\}', '*', p)
    p = p.split('?')[0]
    segs = [('*' if '*' in s else s) for s in p.split('/')]
    return '/'.join(segs)

def backend_matches(fe_path_norm):
    # 前端动态段是整段 *；后端 {id} 也是整段 *
    return fe_path_norm in bymethod

broken = []
ok = 0
for meth, path, f in sorted(set(raw)):
    n = norm_fe(path)
    # 尝试精确匹配；若前端有 * 而后端同位置可能是静态段（例如 ${id} 嵌入静态前缀）
    if n in bymethod and meth in bymethod[n]:
        ok += 1
    elif n in bymethod:
        # 方法不匹配但路径存在 —— 仍算断链(方法错)
        broken.append((meth, path, f, f"PATH-EXISTS methods={bymethod[n]}"))
    else:
        # 段级模糊：逐段比对允许 * 匹配任意后端段
        found = None
        for bp, ms in bymethod.items():
            bs = bp.split('/'); fs = n.split('/')
            if len(bs) != len(fs): continue
            if all(b == s or s == '*' or b == '*' for b, s in zip(bs, fs)):
                if meth in ms:
                    found = bp; break
        if found:
            ok += 1
        else:
            broken.append((meth, path, f, "NO-MATCH"))

print(f"FE calls: {len(set(raw))}, OK: {ok}, BROKEN: {len(broken)}")
for b in broken:
    print("BROKEN", b)
