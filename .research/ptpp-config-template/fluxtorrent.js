// PT-Plugin-Plus 解析脚本模板：把 /compat/nexusphp/torrents.json 的信封
// {code, message, data:{items:[...]}} 翻译为插件统一搜索结果结构。
// 提 PR 时与 config.json 同目录放置（resource/sites/<域名>/fluxtorrent.js）。
(function () {
  'use strict';
  const results = [];
  const items = ((data && data.data && data.data.items) || []);
  for (const t of items) {
    results.push({
      id: t.id,
      title: t.name,
      subTitle: t.small_descr || '',
      time: new Date(t.added * 1000).toISOString().slice(0, 19).replace('T', ' '),
      size: t.size,
      seeders: t.seeders,
      leechers: t.leechers,
      completed: t.completed,
      comments: 0,
      author: '',
      category: String(t.category),
      tags: [],
      // 免费标记（插件促销标签渲染）：free=true 时显示 Free 徽标
      isFree: t.free === true,
      progress: false,
      status: 1,
      entryName: '全部'
    });
  }
  return results;
})();
