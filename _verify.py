import io, re

h = open(r'D:\FluxTorrent\_pg.html', encoding='utf-8', errors='replace').read()
o = io.open(r'D:\FluxTorrent\_pg11.txt', 'w', encoding='utf-8')
kinds = sorted(set(re.findall(r'name=.(sec_[a-z_]+).', h)))
cats = re.findall(r'category_id" value=.[0-9]+./>(.{0,8})', h)
o.write('len: %d\n' % len(h))
o.write('imdb_option: %s\n' % ('>IMDb<' in h))
o.write('game_cat: %s\n' % ('PC游戏' in h))
o.write('dims: %s\n' % ','.join(kinds))
o.write('cats: %s\n' % ','.join(cats))
o.close()
print('ok')
