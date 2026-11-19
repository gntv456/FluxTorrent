import { computeUxReport } from "../apps/web/lib/ux-metrics-report.ts";
import assert from "node:assert/strict";

const now = Date.UTC(2026, 10, 19, 12, 0, 0);
const first = now - 8 * 86400000;
const base = new Date(first).toISOString().slice(0, 10);
const shift = (d) => new Date(Date.parse(d + "T00:00:00Z") + d * 86400000);
const day1 = new Date(Date.parse(base + "T00:00:00Z") + 86400000)
  .toISOString().slice(0, 10);
const day7 = new Date(Date.parse(base + "T00:00:00Z") + 7 * 86400000)
  .toISOString().slice(0, 10);

const profile = {
  firstSeen: first,
  activeDays: [base, day1, day7],
  events: [
    { name: "app_ready", at: first + 1000, value: 1000 },
    { name: "first_torrent_open", at: first + 3000, value: 3000 },
    { name: "checkin", at: first + 5000, value: 5000 },
    { name: "search_submit", at: first + 6000, value: 6000 },
    { name: "search_submit", at: first + 7000, value: 7000 },
    { name: "empty_state", label: "filter", at: first + 8000, value: 8000 },
  ],
};
const r = computeUxReport(profile, now);
assert.equal(r.ageDays, 8);
assert.equal(r.activeDays, 3);
assert.equal(r.d1Return, true);
assert.equal(r.d7Return, true);
assert.equal(r.ttfvMs, 2000);
assert.equal(r.searchCount, 2);
assert.equal(r.emptyByLabel[0].label, "filter");
assert.equal(r.emptyByLabel[0].count, 1);
assert.equal(r.steps.find((s) => s.name === "checkin").count, 1);
assert.equal(r.steps.find((s) => s.name === "signup_complete").count, 0);
assert.equal(r.eventCount, 6);

const empty = computeUxReport(
  { firstSeen: null, activeDays: [], events: [] }, now,
);
assert.equal(empty.ageDays, 0);
assert.equal(empty.d1Return, false);
assert.equal(empty.ttfvMs, null);
assert.equal(empty.steps.length, 7);
console.log("UX_METRICS_SELFTEST_OK", JSON.stringify({
  ageDays: r.ageDays, d1: r.d1Return, d7: r.d7Return,
  ttfvMs: r.ttfvMs, searchCount: r.searchCount, steps: r.steps.length,
}));
