// 纯文本 part 设计稿校验（11 封 × 9 项）。
// 规则：68–72 列折行；URL 整行不折（截断链接会让人复制出错），
// 因此 URL-only 行放宽到 120 列，并在结果里单列计数。
import fs from "node:fs";
import path from "node:path";

const root = path.resolve("design/emails/plain");
const outDir = path.resolve("design/_shots");
if (!fs.existsSync(outDir)) fs.mkdirSync(outDir, { recursive: true });

const expect = {
  invite: ["{{site}}", "{{base}}", "{{inviter}}", "{{invite_code}}"],
  "password-reset": ["{{site}}", "{{base}}", "{{reset_token}}"],
  "email-verify": ["{{site}}", "{{base}}", "{{username}}", "{{verify_token}}"],
  "review-approved": ["{{site}}", "{{base}}", "{{torrent_title}}"],
  "review-rejected": [
    "{{site}}", "{{base}}", "{{torrent_title}}", "{{reject_reason}}",
  ],
  "application-approved": [
    "{{site}}", "{{base}}", "{{username}}", "{{email}}", "{{invite_code}}",
  ],
  "application-rejected": [
    "{{site}}", "{{base}}", "{{username}}", "{{email}}", "{{reject_note}}",
  ],
  "appeal-accepted": ["{{site}}", "{{base}}", "{{appeal_id}}", "{{appeal_note}}"],
  "appeal-rejected": ["{{site}}", "{{base}}", "{{appeal_id}}", "{{appeal_note}}"],
  "account-status": [
    "{{site}}", "{{base}}", "{{username}}", "{{status_label}}",
    "{{status_reason}}",
  ],
  "hr-warning": [
    "{{site}}", "{{base}}", "{{torrent_title}}", "{{deadline}}",
  ],
};

const HTML_TAG = /<\/?[a-z][^>]*>/i;
const MARKDOWN = /^\s*(#{1,6}\s|>|-{3,}|\*\s|-\s|\d+\.\s.*\*\*)/;
const EMOJI = /\p{Extended_Pictographic}/u;

const rows = [];
for (const [name, placeholders] of Object.entries(expect)) {
  const p = path.join(root, `${name}.txt`);
  const checks = {};
  if (!fs.existsSync(p)) {
    checks.exists = false;
    rows.push({ name, checks });
    continue;
  }
  checks.exists = true;
  const raw = fs.readFileSync(p);
  const buf = Buffer.from(fs.readFileSync(p));
  checks.noBom = !(buf[0] === 0xef && buf[1] === 0xbb && buf[2] === 0xbf);
  const text = raw.toString("utf8");
  checks.lfOnly = !text.includes("\r");
  const lines = text.split("\n");
  checks.subjectFirst = lines[0].startsWith("Subject: ");
  checks.hasWhy = text.includes("你收到这封邮件");
  checks.hasContact = text.includes("{{base}}/contactstaff");
  checks.placeholders = placeholders.every((ph) => text.includes(ph));
  checks.noHtml = !HTML_TAG.test(text);
  checks.noMarkdown = lines.every((l) => !MARKDOWN.test(l));
  checks.noEmoji = !EMOJI.test(text);
  const urlLines = lines.filter((l) => /^\s*(https?:|{{base}})/.test(l));
  checks.urlsAbsolute = urlLines.every((l) => l.includes("{{base}}/"));
  const over = [];
  lines.forEach((l, i) => {
    const n = [...l].length;
    const isUrl = /^\s*{{base}}\/\S+$/.test(l);
    if (isUrl ? n > 120 : n > 78) over.push({ line: i + 1, len: n });
  });
  checks.lineWidth = over.length === 0;
  checks.overLong = over;
  checks.urlOnlyLines = urlLines.length;
  checks.endsWithNewline = text.endsWith("\n");
  rows.push({ name, checks });
}

const cols = [
  "exists", "noBom", "lfOnly", "subjectFirst", "hasWhy", "hasContact",
  "placeholders", "noHtml", "noMarkdown", "noEmoji", "urlsAbsolute",
  "lineWidth", "endsWithNewline",
];
const allPass = rows.every((r) => cols.every((c) => r.checks[c] === true));

const table = [
  `| 邮件 | ${cols.join(" | ")} | URL 行 | 超长行 |`,
  `| --- | ${cols.map(() => "---").join(" | ")} | --- | --- |`,
  ...rows.map((r) => {
    const marks = cols.map((c) => (r.checks[c] ? "✓" : "✗"));
    return `| ${r.name} | ${marks.join(" | ")} | ${r.checks.urlOnlyLines ?? 0} | ${(r.checks.overLong ?? []).length} |`;
  }),
];

const summary = {
  generatedAt: new Date().toISOString(),
  files: rows.length,
  allPass,
  urlLinePolicy: "URL-only 行不折行，放宽到 ≤120 列；其余行 ≤78 列",
  rows,
};
fs.writeFileSync(
  path.join(outDir, "plain-verify.json"),
  JSON.stringify(summary, null, 2),
);
console.log(table.join("\n"));
console.log(`\nallPass=${allPass}  files=${rows.length}`);
if (!allPass) process.exit(1);
