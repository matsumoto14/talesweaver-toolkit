/**
 * 管理画面の静的 HTML(ビルド無し。インライン CSS/JS のみ、外部読み込み無し)。
 * 目的は「wiki に聞く」を良くすること。上から: いまの状態(指標)→ 日別の結果と費用のグラフ →
 * 直すべき質問の一覧 → 1 件の処理の流れ(どこで落ちたか・どこを直すか)。
 * 注意: 全体が TS のテンプレート文字列なので、中の JS にバッククォート・ドル波括弧・バックスラッシュを書かない。
 */
export const ADMIN_HTML = `<!doctype html>
<html lang="ja">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>wiki に聞く — 監視</title>
<style>
  :root {
    color-scheme: light dark;
    --bg: #f4f5f7; --panel: #ffffff; --inset: #f4f5f7; --border: #dfe1e6; --text: #1c1d21; --muted: #6b6f7a;
    --accent: #3457d5;
    --ok: #2e9e5b; --partial: #8fb838; --notfound: #e8912d; --off: #a3a8b3; --error: #d64545; --skip: #d5d8de;
  }
  @media (prefers-color-scheme: dark) {
    :root {
      --bg: #141518; --panel: #1d1f23; --inset: #16171a; --border: #30333a; --text: #ececef; --muted: #9a9ea8;
      --accent: #7f9bff; --skip: #3a3d45; --off: #6f7480;
    }
  }
  * { box-sizing: border-box; }
  body { margin: 0; background: var(--bg); color: var(--text); font: 13px/1.55 -apple-system, "Segoe UI", "Hiragino Sans", "Yu Gothic UI", sans-serif; }
  header { padding: 12px 16px; display: flex; align-items: center; gap: 12px; flex-wrap: wrap; }
  header h1 { font-size: 16px; margin: 0; }
  header .spacer { flex: 1; }
  .seg { display: inline-flex; border: 1px solid var(--border); border-radius: 8px; overflow: hidden; }
  .seg button { border: 0; background: var(--panel); color: var(--muted); padding: 4px 10px; font: inherit; cursor: pointer; }
  .seg button.on { background: var(--accent); color: #fff; }
  main { display: grid; grid-template-columns: minmax(0, 1fr); gap: 12px; padding: 0 12px 16px; }
  @media (min-width: 1100px) { main.with-detail { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); } #detail { grid-column: 2; grid-row: 1 / span 3; } }
  .panel { background: var(--panel); border: 1px solid var(--border); border-radius: 10px; padding: 14px; min-width: 0; }
  h2 { font-size: 12px; color: var(--muted); font-weight: 600; margin: 0 0 10px; }
  .num { text-align: right; font-variant-numeric: tabular-nums; }
  .muted { color: var(--muted); }
  a { color: var(--accent); }

  /* 指標 */
  .kpis { display: grid; grid-template-columns: repeat(auto-fit, minmax(130px, 1fr)); gap: 10px; }
  .kpi { background: var(--inset); border-radius: 8px; padding: 10px 12px; border: 1px solid transparent; }
  .kpi.click { cursor: pointer; } .kpi.click:hover { border-color: var(--accent); }
  .kpi .v { font-size: 24px; font-weight: 700; font-variant-numeric: tabular-nums; line-height: 1.2; }
  .kpi .l { color: var(--muted); font-size: 12px; }
  .kpi .bar { height: 6px; border-radius: 3px; background: var(--skip); margin-top: 6px; overflow: hidden; display: flex; }
  .kpi .bar i { display: block; height: 100%; }

  /* グラフ */
  .legend { display: flex; flex-wrap: wrap; gap: 12px; margin-bottom: 6px; font-size: 12px; }
  .legend span::before { content: ""; display: inline-block; width: 10px; height: 10px; border-radius: 2px; margin-right: 4px; vertical-align: -1px; background: var(--c); }
  svg.chart { width: 100%; height: auto; display: block; }
  svg.chart text { fill: var(--muted); font-size: 10px; }
  svg.chart .grid { stroke: var(--border); stroke-width: 1; }
  svg.chart rect.hit:hover { fill: rgba(127,127,127,.12); }

  /* 一覧 */
  .tabs { display: flex; gap: 6px; margin-bottom: 10px; flex-wrap: wrap; align-items: center; }
  .tabs button { border: 1px solid var(--border); background: var(--panel); color: var(--text); border-radius: 999px; padding: 3px 12px; font: inherit; cursor: pointer; }
  .tabs button.on { background: var(--text); color: var(--panel); border-color: var(--text); }
  .list { display: flex; flex-direction: column; }
  .item { display: grid; grid-template-columns: 92px minmax(0, 1fr) auto; gap: 4px 10px; align-items: center; padding: 8px 6px; border-bottom: 1px solid var(--border); cursor: pointer; border-radius: 6px; }
  .item:hover { background: var(--inset); }
  .item.sel { background: var(--inset); box-shadow: inset 3px 0 0 var(--accent); }
  .item .q { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .item .meta { color: var(--muted); font-size: 12px; font-variant-numeric: tabular-nums; text-align: right; white-space: nowrap; }
  .item .sub { grid-column: 2 / span 2; color: var(--muted); font-size: 12px; }
  .chip { display: inline-block; font-size: 11px; font-weight: 600; padding: 1px 8px; border-radius: 999px; color: #fff; background: var(--c); text-align: center; white-space: nowrap; }
  #more { display: block; margin: 10px auto 0; border: 1px solid var(--border); background: var(--panel); color: var(--text); border-radius: 8px; padding: 4px 14px; font: inherit; cursor: pointer; }
  .empty { color: var(--muted); padding: 12px 4px; }

  /* 詳細 */
  #detail .question { font-size: 17px; font-weight: 700; margin: 0 0 4px; }
  .flow { display: flex; align-items: stretch; gap: 0; margin: 14px 0; overflow-x: auto; padding-bottom: 4px; }
  .node { flex: 1 0 84px; text-align: center; position: relative; padding: 0 2px; }
  .node .dot { width: 30px; height: 30px; border-radius: 50%; margin: 0 auto 4px; display: flex; align-items: center; justify-content: center; color: #fff; font-weight: 700; background: var(--c); }
  .node.skip .dot { color: var(--muted); }
  .node .n { font-weight: 600; font-size: 12px; }
  .node .s { color: var(--muted); font-size: 11px; line-height: 1.35; }
  .node:not(:last-child)::after { content: ""; position: absolute; top: 15px; left: calc(50% + 17px); right: calc(-50% + 17px); height: 2px; background: var(--border); }
  .node.fail .n { color: var(--error); }
  .hint { border-left: 4px solid var(--c); background: var(--inset); border-radius: 6px; padding: 10px 12px; margin-bottom: 14px; }
  .hint b { display: block; margin-bottom: 2px; }
  .lead { font-size: 15px; font-weight: 600; margin: 0 0 10px; }
  .step { background: var(--inset); border-radius: 8px; padding: 10px; margin-bottom: 8px; }
  .step h3 { font-size: 13px; margin: 0 0 2px; }
  .step .src { color: var(--muted); font-size: 12px; margin-bottom: 6px; }
  .step p { margin: 4px 0; white-space: pre-wrap; }
  table { width: 100%; border-collapse: collapse; }
  td, th { padding: 3px 6px; text-align: left; border-bottom: 1px solid var(--border); font-variant-numeric: tabular-nums; vertical-align: top; }
  th { color: var(--muted); font-weight: 500; }
  details { border-top: 1px solid var(--border); padding: 8px 0; }
  details summary { cursor: pointer; font-weight: 600; color: var(--muted); }
  details[open] summary { margin-bottom: 8px; }
  pre { white-space: pre-wrap; word-break: break-word; background: var(--inset); border-radius: 6px; padding: 8px; max-height: 300px; overflow: auto; font-size: 12px; }
  .kv { display: grid; grid-template-columns: auto 1fr; gap: 3px 12px; margin: 0; }
  .kv dt { color: var(--muted); } .kv dd { margin: 0; }
  .conv { list-style: none; padding: 0; margin: 0; }
  .conv li { padding: 3px 0; }
  .conv li.cur { font-weight: 700; }
</style>
</head>
<body>
<header>
  <h1>wiki に聞く — 監視</h1>
  <span class="muted" id="status"></span>
  <span class="spacer"></span>
  <span class="seg" id="range"><button data-d="7">7 日</button><button data-d="30" class="on">30 日</button><button data-d="90">90 日</button></span>
</header>
<main id="main">
  <section class="panel">
    <h2>いまの状態</h2>
    <div class="kpis" id="kpis"></div>
  </section>

  <section class="panel">
    <h2>日別の結果</h2>
    <div class="legend" id="legend"></div>
    <div id="chart-outcome"></div>
    <h2 style="margin-top:14px">日別の費用(USD)</h2>
    <div id="chart-cost"></div>
  </section>

  <section class="panel">
    <div class="tabs" id="tabs">
      <button data-t="problem" class="on">直すべきもの</button>
      <button data-t="all">すべて</button>
      <span class="muted" style="font-size:12px">行を押すと、どこで落ちたかが右に出ます</span>
    </div>
    <div class="list" id="list"></div>
    <button id="more">さらに読む</button>
  </section>
</main>

<script>
(function () {
  const OUTCOME = {
    answered: { label: "答えた", color: "var(--ok)" },
    partial: { label: "一部だけ", color: "var(--partial)" },
    not_found: { label: "見つからない", color: "var(--notfound)" },
    off: { label: "雑談・範囲外", color: "var(--off)" },
    error: { label: "エラー", color: "var(--error)" },
  };
  const ORDER = ["answered", "partial", "not_found", "off", "error"];
  const state = { days: 30, tab: "problem", before: null, selectedId: null };

  function qs(id) { return document.getElementById(id); }
  function esc(s) { return String(s ?? "").replace(/[&<>"']/g, (c) => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c])); }
  function usd(v, digits) { return v === null || v === undefined ? "?" : "$" + v.toFixed(digits ?? 4); }
  function num(v) { return v === null || v === undefined ? "?" : Number(v).toLocaleString("ja-JP"); }
  function sec(ms) { return ms === null || ms === undefined ? "?" : (ms / 1000).toFixed(1) + " 秒"; }
  function pct(a, b) { return b > 0 ? Math.round((a / b) * 100) + "%" : "-"; }
  function chip(outcome) { const o = OUTCOME[outcome]; return "<span class='chip' style='--c:" + o.color + "'>" + o.label + "</span>"; }

  function outcomeOf(kind, reason, missingN) {
    if (kind === "error") return "error";
    if (kind === "answer") return (missingN ?? 0) > 0 ? "partial" : "answered";
    if (reason === "smalltalk" || reason === "other") return "off";
    return "not_found";
  }

  async function api(path) {
    const res = await fetch(path, { headers: { accept: "application/json" } });
    if (!res.ok) throw new Error("HTTP " + res.status);
    return res.json();
  }

  // ---- いまの状態 + グラフ -------------------------------------------------------

  function lastDates(n) {
    const out = [];
    const today = new Date();
    for (let i = n - 1; i >= 0; i -= 1) {
      const d = new Date(Date.UTC(today.getUTCFullYear(), today.getUTCMonth(), today.getUTCDate() - i));
      out.push(d.toISOString().slice(0, 10));
    }
    return out;
  }

  async function loadSummary() {
    try {
      const data = await api("/api/summary?days=" + state.days);
      const byDate = new Map(data.days.map((d) => [d.date, d]));
      const dates = lastDates(state.days);
      const t = { q: 0, ms: 0, react: { helpful: 0, wrong: 0, value_wrong: 0 } };
      ORDER.forEach((k) => { t[k] = 0; });
      for (const d of data.days) {
        t.q += d.questions;
        t.ms += (d.avg_ms ?? 0) * d.questions;
        ORDER.forEach((k) => { t[k] += d.outcomes[k]; });
        t.react.helpful += d.reactions.helpful; t.react.wrong += d.reactions.wrong; t.react.value_wrong += d.reactions.value_wrong;
      }
      const asked = t.q - t.off;
      const answered = t.answered + t.partial;
      const bad = t.react.wrong + t.react.value_wrong;
      const problems = t.not_found + t.error + t.partial;
      const bar = ORDER.map((k) => "<i style='width:" + (t.q > 0 ? (t[k] / t.q) * 100 : 0) + "%;background:" + OUTCOME[k].color + "'></i>").join("");
      qs("kpis").innerHTML =
        "<div class='kpi'><div class='v'>" + num(t.q) + "</div><div class='l'>質問</div><div class='bar'>" + bar + "</div></div>" +
        "<div class='kpi'><div class='v' style='color:var(--ok)'>" + pct(answered, asked) + "</div><div class='l'>答えられた(雑談を除く " + num(asked) + " 問中 " + num(answered) + ")</div></div>" +
        "<div class='kpi click' id="+"'kpi-problem'"+"><div class='v' style='color:var(--notfound)'>" + num(problems) + "</div><div class='l'>直すべきもの(見つからない・一部・エラー)</div></div>" +
        "<div class='kpi'><div class='v' style='color:" + (bad > 0 ? "var(--error)" : "inherit") + "'>👎 " + num(bad) + "</div><div class='l'>違うと言われた答え(👍 " + num(t.react.helpful) + "。値の誤り報告を含む)</div></div>" +
        "<div class='kpi'><div class='v'>" + usd(data.total_cost_usd, 3) + "</div><div class='l'>費用(1 問 " + usd(data.total_cost_usd !== null && t.q > 0 ? data.total_cost_usd / t.q : null, 4) + ")</div></div>" +
        "<div class='kpi'><div class='v'>" + sec(t.q > 0 ? t.ms / t.q : null) + "</div><div class='l'>平均の待ち時間</div></div>";
      const kp = qs("kpi-problem");
      if (kp) kp.addEventListener("click", () => setTab("problem"));

      qs("legend").innerHTML = ORDER.map((k) => "<span style='--c:" + OUTCOME[k].color + "'>" + OUTCOME[k].label + "</span>").join("");
      qs("chart-outcome").innerHTML = stackedChart(dates, byDate);
      qs("chart-cost").innerHTML = costChart(dates, byDate);
      qs("status").textContent = "";
    } catch (e) {
      qs("status").textContent = "集計の読み込みに失敗: " + e.message;
    }
  }

  function axisLabels(dates, x, bw, h) {
    const step = Math.max(1, Math.ceil(dates.length / 10));
    return dates.map((d, i) => (i % step === 0 || i === dates.length - 1)
      ? "<text x='" + (x(i) + bw / 2) + "' y='" + h + "' text-anchor='middle'>" + d.slice(5).replace("-", "/") + "</text>" : "").join("");
  }

  function stackedChart(dates, byDate) {
    const W = 720, H = 180, top = 10, bottom = 20, left = 28;
    const max = Math.max(1, ...dates.map((d) => (byDate.get(d) ? byDate.get(d).questions : 0)));
    const plotH = H - top - bottom;
    const bw = (W - left) / dates.length;
    const x = (i) => left + i * bw;
    let svg = "<svg class='chart' viewBox='0 0 " + W + " " + H + "'>";
    [0, 0.5, 1].forEach((f) => {
      const y = top + plotH * (1 - f);
      svg += "<line class='grid' x1='" + left + "' x2='" + W + "' y1='" + y + "' y2='" + y + "'/><text x='" + (left - 4) + "' y='" + (y + 3) + "' text-anchor='end'>" + Math.round(max * f) + "</text>";
    });
    dates.forEach((d, i) => {
      const day = byDate.get(d);
      let y = top + plotH;
      const tip = [d + " 質問 " + (day ? day.questions : 0)];
      if (day) {
        ORDER.forEach((k) => {
          const v = day.outcomes[k];
          if (!v) return;
          const h = (v / max) * plotH;
          y -= h;
          svg += "<rect x='" + (x(i) + bw * 0.15) + "' y='" + y + "' width='" + bw * 0.7 + "' height='" + h + "' fill='" + OUTCOME[k].color + "' rx='1.5'/>";
          tip.push(OUTCOME[k].label + " " + v);
        });
        const bad = day.reactions.wrong + day.reactions.value_wrong;
        if (bad) tip.push("👎 " + bad);
      }
      svg += "<rect class='hit' x='" + x(i) + "' y='" + top + "' width='" + bw + "' height='" + plotH + "' fill='transparent'><title>" + esc(tip.join(" / ")) + "</title></rect>";
    });
    svg += axisLabels(dates, x, bw, H - 4) + "</svg>";
    return svg;
  }

  function costChart(dates, byDate) {
    const W = 720, H = 90, top = 8, bottom = 20, left = 28;
    const vals = dates.map((d) => (byDate.get(d) ? byDate.get(d).cost_usd ?? 0 : 0));
    const max = Math.max(0.0001, ...vals);
    const plotH = H - top - bottom;
    const bw = (W - left) / dates.length;
    const x = (i) => left + i * bw;
    let svg = "<svg class='chart' viewBox='0 0 " + W + " " + H + "'>";
    svg += "<line class='grid' x1='" + left + "' x2='" + W + "' y1='" + (top + plotH) + "' y2='" + (top + plotH) + "'/>";
    svg += "<text x='" + (left - 4) + "' y='" + (top + 6) + "' text-anchor='end'>" + max.toFixed(max < 0.1 ? 3 : 2) + "</text>";
    vals.forEach((v, i) => {
      const h = (v / max) * plotH;
      const day = byDate.get(dates[i]);
      svg += "<rect x='" + (x(i) + bw * 0.2) + "' y='" + (top + plotH - h) + "' width='" + bw * 0.6 + "' height='" + h + "' fill='var(--accent)' opacity='.75' rx='1.5'/>";
      svg += "<rect class='hit' x='" + x(i) + "' y='" + top + "' width='" + bw + "' height='" + plotH + "' fill='transparent'><title>" +
        esc(dates[i] + " " + usd(day ? day.cost_usd : 0, 4) + (day && day.avg_cost_usd !== null ? "(1 問 " + usd(day.avg_cost_usd, 4) + ")" : "")) + "</title></rect>";
    });
    svg += axisLabels(dates, x, bw, H - 4) + "</svg>";
    return svg;
  }

  // ---- 一覧 ---------------------------------------------------------------------

  function setTab(tab) {
    state.tab = tab;
    document.querySelectorAll("#tabs button").forEach((b) => b.classList.toggle("on", b.dataset.t === tab));
    loadLogs(true);
  }

  function reactionIcons(r) {
    const parts = [];
    if (r.helpful) parts.push("👍" + (r.helpful > 1 ? r.helpful : ""));
    if (r.wrong) parts.push("👎" + (r.wrong > 1 ? r.wrong : ""));
    if (r.value_wrong) parts.push("値✗" + (r.value_wrong > 1 ? r.value_wrong : ""));
    return parts.join(" ");
  }

  const WHY_SHORT = {
    llm_none: "候補を見て LLM が「答えなし」",
    verification_failed: "LLM の選択が検証で落ちた",
    no_terms: "検索語が取れない",
    smalltalk: "雑談と判断",
    other: "範囲外と判断",
  };
  const ROUTE = { cheap: "安い道", loop: "回す道", cheap_then_loop: "安い道 → 回す道", cached: "キャッシュ" };

  async function loadLogs(reset) {
    const list = qs("list");
    if (reset) { state.before = null; list.innerHTML = ""; }
    const p = new URLSearchParams();
    if (state.tab === "problem") p.set("problem", "1");
    if (state.before) p.set("before", state.before);
    try {
      const data = await api("/api/logs?" + p.toString());
      for (const row of data.logs) {
        const outcome = outcomeOf(row.kind, row.reason, row.missing_n);
        const el = document.createElement("div");
        el.className = "item" + (row.id === state.selectedId ? " sel" : "");
        el.dataset.id = row.id;
        const why = row.kind === "error" ? (row.reason ?? "") : row.kind === "none" ? (WHY_SHORT[row.reason] ?? row.reason ?? "") :
          outcome === "partial" ? "一部の観点が見つからない" : "";
        el.innerHTML =
          chip(outcome) +
          "<div class='q'>" + esc(row.question) + "</div>" +
          "<div class='meta'>" + esc(reactionIcons(row.reactions)) + " " + sec(row.ms) + " · " + usd(row.cost_usd, 4) + "</div>" +
          "<div class='sub'>" + esc(row.at.replace("T", " ").slice(5, 16)) + (row.route ? " · " + esc(ROUTE[row.route] ?? row.route) : "") + (why ? " · " + esc(why) : "") + "</div>";
        el.addEventListener("click", () => selectLog(row.id));
        list.appendChild(el);
      }
      state.before = data.next;
      qs("more").style.display = data.next ? "" : "none";
      if (reset && data.logs.length === 0) {
        list.innerHTML = "<div class='empty'>" + (state.tab === "problem" ? "直すべきものはありません 🎉" : "まだ質問がありません") + "</div>";
      }
    } catch (e) {
      qs("status").textContent = "一覧の読み込みに失敗: " + e.message;
    }
  }

  // ---- 詳細: 処理の流れ ------------------------------------------------------------

  const STATE_COLOR = { ok: "var(--ok)", warn: "var(--notfound)", fail: "var(--error)", skip: "var(--skip)" };
  const STATE_MARK = { ok: "✓", warn: "!", fail: "✗", skip: "–" };

  function sumCalls(calls, kinds) {
    const list = calls.filter((c) => kinds.includes(c.kind));
    if (list.length === 0) return null;
    const cost = list.some((c) => c.cost_usd === null) ? null : list.reduce((a, c) => a + c.cost_usd, 0);
    return { n: list.length, cost, ms: list.reduce((a, c) => a + c.ms, 0) };
  }

  /** 各段の状態と、落ちた段の説明(どこを直すか)を決める */
  function analyze(log, calls, reactions) {
    const body = log.body || {};
    const u = log.understanding;
    const cands = (log.candidates || []).length;
    const und = sumCalls(calls, ["understand"]);
    const sel = sumCalls(calls, ["select", "loop"]);
    const loopN = calls.filter((c) => c.kind === "loop").length;
    const dropped = (body.dropped || []).filter((d) => d.what !== "route" && d.what !== "kind");
    const kindLabel = u ? ({ wiki: "wiki の質問", smalltalk: "雑談", other: "範囲外" }[u.kind] ?? u.kind) : "記録なし";
    const selLabel = log.route === "cached" ? "キャッシュ" : loopN > 0 ? "回す道 " + loopN + " 往復" : sel ? "安い道" : "";
    const S = {
      understand: { name: "理解", st: u ? "ok" : "warn", sub: kindLabel + (und ? "<br>" + usd(und.cost, 4) : "") },
      search: { name: "検索", st: "ok", sub: "候補 " + cands + " 件" },
      select: { name: "選択", st: "ok", sub: selLabel + (sel ? "<br>" + usd(sel.cost, 4) : "") },
      verify: { name: "検証", st: "ok", sub: dropped.length > 0 ? "落とした " + dropped.length + " 件" : "全部通過" },
      answer: { name: "答え", st: "ok", sub: body.steps ? "手順 " + body.steps.length : "" },
      reaction: { name: "反応", st: "skip", sub: "なし" },
    };
    const skipFrom = (keys) => keys.forEach((k) => { S[k].st = "skip"; S[k].sub = ""; });
    let hint = { st: "ok", title: "問題なし", text: "答えを返しています。" };

    if (log.kind === "error") {
      const at = !u ? "understand" : cands === 0 ? "search" : "select";
      S[at].st = "fail";
      const order = ["understand", "search", "select", "verify", "answer", "reaction"];
      skipFrom(order.slice(order.indexOf(at) + 1));
      hint = { st: "fail", title: "エラー: " + (log.reason ?? ""), text: "Worker 側で失敗しました。Anthropic のクレジット残高・タイムアウト・D1 の障害を確認。" };
    } else if (log.kind === "none" && (log.reason === "smalltalk" || log.reason === "other")) {
      S.understand.st = "warn";
      skipFrom(["search", "select", "verify", "answer", "reaction"]);
      hint = { st: "warn", title: (log.reason === "smalltalk" ? "雑談" : "wiki の範囲外") + "と判断して検索していません",
        text: "wiki の質問だったなら誤分類です → 理解の指示(prompt.ts の kind の定義)か、別名辞書に語を足して索引に当たるようにする。" };
    } else if (log.kind === "none" && log.reason === "no_terms") {
      S.search.st = "fail"; S.search.sub = "検索語なし";
      skipFrom(["select", "verify", "answer", "reaction"]);
      hint = { st: "fail", title: "検索語が取れませんでした", text: "質問の語が辞書に無い → 分かち書きの辞書・別名に語を足す。" };
    } else if (log.kind === "none" && log.reason === "llm_none") {
      S.select.st = "fail";
      skipFrom(["verify", "answer", "reaction"]);
      hint = { st: "fail", title: "候補を見た LLM が「答えが無い」と判断",
        text: "下の「LLM に見せた候補」に正解のページがあるか見る。無ければ検索の取りこぼし(別名・ページ加点・検索語)、あれば選択の指示か候補の見せ方を直す。" };
    } else if (log.kind === "none" && log.reason === "verification_failed") {
      S.verify.st = "fail";
      skipFrom(["answer", "reaction"]);
      hint = { st: "fail", title: "LLM の選んだ根拠が検証で全部落ちた", text: "「落としたもの」の理由を見る。検証が厳しすぎるのか、LLM が候補に無い値を書いたのか。" };
    } else if (log.kind === "answer" && (body.missing || []).length > 0) {
      S.answer.st = "warn"; S.answer.sub = "一部だけ";
      hint = { st: "warn", title: "答えたが一部の観点が見つからない(" + body.missing.join("、") + ")",
        text: "wiki に本当に無いのか、候補の取りこぼしか。候補一覧に該当ページがあるかを見る。" };
    }

    const bad = reactions.filter((r) => r.kind === "wrong" || r.kind === "value_wrong");
    const good = reactions.filter((r) => r.kind === "helpful");
    if (bad.length > 0) {
      S.reaction.st = "fail"; S.reaction.sub = "👎 " + bad.length;
      const r = bad[0];
      hint = r.kind === "value_wrong"
        ? { st: "fail", title: "値の誤り報告: " + (r.col ?? "") + (r.claim ? " → 正しくは「" + r.claim + "」" : ""), text: "根拠の行と見比べ、正しければ訂正を登録する。" + (r.note ? " メモ: " + r.note : "") }
        : { st: "fail", title: "ユーザーが 👎" + (r.reason ? "(" + ({ off_topic: "聞いたことと違う", outdated: "古い", unclear: "分かりにくい" }[r.reason] ?? r.reason) + ")" : ""),
            text: "答えと根拠を見比べる。聞いたことと違うなら選択、古いなら wiki の同期、分かりにくいなら結論文。" };
    } else if (good.length > 0) {
      S.reaction.st = "ok"; S.reaction.sub = "👍 " + good.length;
    }
    return { stages: ["understand", "search", "select", "verify", "answer", "reaction"].map((k) => S[k]), hint };
  }

  function renderFlow(stages) {
    return "<div class='flow'>" + stages.map((s) =>
      "<div class='node " + s.st + "' style='--c:" + STATE_COLOR[s.st] + "'><div class='dot'>" + STATE_MARK[s.st] + "</div><div class='n'>" + s.name + "</div><div class='s'>" + s.sub + "</div></div>"
    ).join("") + "</div>";
  }

  function srcLink(page, section, url) {
    const label = esc(page) + (section ? " › " + esc(section) : "");
    return url ? "<a href='" + esc(url) + "' target='_blank' rel='noopener'>" + label + "</a>" : label;
  }

  /** 結論文の {{unit.列}} を、手順に載った値で埋める(端末と同じ見え方に寄せる) */
  function leadText(body) {
    if (!body.lead) return null;
    const cells = new Map();
    for (const st of body.steps || []) for (const u of st.units || []) cells.set(u.id, u.cells || {});
    return body.lead.map((seg) => ("t" in seg ? esc(seg.t) : "<b>" + esc((cells.get(seg.ref) || {})[seg.col] ?? "?") + "</b>")).join("");
  }

  function renderUnits(step) {
    const rows = (step.units || []).filter((u) => u.kind === "row");
    const paras = (step.units || []).filter((u) => u.kind !== "row");
    let html = paras.map((u) => "<p>" + esc(u.text) + (u.truncated ? " …" : "") + "</p>").join("");
    if (rows.length > 0) {
      const cols = step.columns && step.columns.length > 0 ? step.columns : Object.keys(rows[0].cells || {});
      html += "<table><thead><tr><th></th>" + cols.map((c) => "<th>" + esc(c) + "</th>").join("") + "</tr></thead><tbody>" +
        rows.map((u) => "<tr><td>" + esc(u.key ?? "") + "</td>" + cols.map((c) => "<td>" + esc((u.cells || {})[c] ?? "") + "</td>").join("") + "</tr>").join("") +
        "</tbody></table>";
    }
    return html;
  }

  function renderAnswer(body) {
    if (!body) return "<p class='muted'>(なし)</p>";
    if (body.kind === "none") {
      if (!body.search || body.search.length === 0) return "<p class='muted'>ユーザーには「見つからない」系の定型文だけが出ました。</p>";
      return "<p class='muted'>ユーザーには近い候補として次を出しました:</p><ul>" +
        body.search.map((s) => "<li>" + srcLink(s.page, s.section, s.url) + "</li>").join("") + "</ul>";
    }
    if (body.kind !== "answer") return "<p>" + esc(body.error ?? "") + "</p>";
    const lead = leadText(body);
    let html = lead ? "<p class='lead'>" + lead + "</p>" : "<p class='muted'>(結論文なし — 手順だけ)</p>";
    html += (body.steps || []).map((st) =>
      "<div class='step'><h3>" + esc(st.title) + "</h3><div class='src'>" + srcLink(st.source.page, st.source.section, st.source.url) + "</div>" + renderUnits(st) + "</div>"
    ).join("");
    if (body.corrections && body.corrections.length > 0) {
      html += "<p class='muted'>訂正を重ねた値: " + body.corrections.map((c) => esc(c.col) + " " + esc(c.wiki) + " → " + esc(c.value)).join("、") + "</p>";
    }
    if (body.next && body.next.length > 0) html += "<p class='muted'>次の一手: " + body.next.map((n) => esc(n.question)).join(" / ") + "</p>";
    return html;
  }

  function renderUnderstanding(u) {
    if (!u) return "<p class='muted'>記録なし(LLM が落ちてコードの経路で進んだか、記録を足す前の質問)</p>";
    return "<dl class='kv'>" +
      "<dt>種類</dt><dd>" + esc({ wiki: "wiki の質問", smalltalk: "雑談", other: "範囲外" }[u.kind] ?? u.kind) + "</dd>" +
      "<dt>ページ</dt><dd>" + (u.pages && u.pages.length > 0 ? u.pages.map(esc).join("、") : "-") + "</dd>" +
      "<dt>検索語</dt><dd>" + (u.terms && u.terms.length > 0 ? u.terms.map(esc).join("、") : "-") + "</dd>" +
      "<dt>たどり</dt><dd>" + (u.hops === "multi" ? "複数回(回す道へ)" : "1 回") + "</dd>" +
      "<dt>続き</dt><dd>" + (u.followup ? "直前の質問の続き" : "-") + "</dd>" +
      "<dt>気分</dt><dd>" + (u.mood === "trouble" ? "困っている" + (u.trouble === "cant_win" ? "(勝てない)" : "") : "-") + "</dd>" +
      "</dl>";
  }

  function renderCandidates(list) {
    if (!list || list.length === 0) return "<p class='muted'>(なし)</p>";
    const byPage = new Map();
    for (const c of list) { const a = byPage.get(c.page) ?? []; a.push(c.section); byPage.set(c.page, a); }
    return "<table><thead><tr><th>ページ</th><th class='num'>件</th><th>節</th></tr></thead><tbody>" +
      [...byPage.entries()].map(([page, secs]) => "<tr><td>" + esc(page) + "</td><td class='num'>" + secs.length + "</td><td class='muted'>" +
        esc([...new Set(secs)].slice(0, 4).join(" / ")) + ([...new Set(secs)].length > 4 ? " …" : "") + "</td></tr>").join("") + "</tbody></table>";
  }

  function renderCalls(calls) {
    if (!calls || calls.length === 0) return "<p class='muted'>(LLM の呼び出しなし)</p>";
    const name = { understand: "理解", select: "選択", loop: "回す道" };
    return "<table><thead><tr><th>#</th><th>段</th><th class='num'>入力</th><th class='num'>キャッシュ読</th><th class='num'>出力</th><th class='num'>時間</th><th class='num'>費用</th></tr></thead><tbody>" +
      calls.map((c) => "<tr><td>" + (c.seq + 1) + "</td><td>" + esc(name[c.kind] ?? c.kind) + "</td><td class='num'>" + num(c.input_tokens + c.cache_creation_tokens) +
        "</td><td class='num'>" + num(c.cache_read_tokens) + "</td><td class='num'>" + num(c.output_tokens) + "</td><td class='num'>" + sec(c.ms) +
        "</td><td class='num'>" + usd(c.cost_usd, 5) + "</td></tr>").join("") + "</tbody></table>";
  }

  function renderDropped(body) {
    const list = (body && body.dropped) || [];
    if (list.length === 0) return "<p class='muted'>(なし)</p>";
    return "<table><thead><tr><th>何を</th><th>id</th><th>理由</th></tr></thead><tbody>" +
      list.map((d) => "<tr><td>" + esc(d.what) + "</td><td>" + esc(d.id ?? "") + "</td><td>" + esc(d.why) + "</td></tr>").join("") + "</tbody></table>";
  }

  function renderConversation(log, neighbors) {
    const item = (n, cur) => "<li class='" + (cur ? "cur" : "") + "'>" + chip(outcomeOf(n.kind, n.reason, n.missing_n)) + " <span class='muted'>" + esc(n.at.slice(11, 16)) + "</span> " + esc(n.question) + "</li>";
    const before = neighbors.before || [], after = neighbors.after || [];
    if (before.length === 0 && after.length === 0) return "<p class='muted'>同じ端末の前後の質問はありません</p>";
    const cur = { at: log.at, question: log.question, kind: log.kind, reason: log.reason, missing_n: ((log.body || {}).missing || []).length };
    return "<ul class='conv'>" + before.map((n) => item(n, false)).join("") + item(cur, true) + after.map((n) => item(n, false)).join("") + "</ul>";
  }

  async function selectLog(id) {
    state.selectedId = id;
    document.querySelectorAll("#list .item").forEach((el) => el.classList.toggle("sel", Number(el.dataset.id) === id));
    qs("main").classList.add("with-detail");
    let detail = qs("detail");
    if (!detail) {
      detail = document.createElement("section");
      detail.className = "panel";
      detail.id = "detail";
      qs("main").appendChild(detail);
    }
    detail.innerHTML = "<p class='muted'>読み込み中…</p>";
    try {
      const data = await api("/api/logs/" + id);
      const log = data.log;
      const a = analyze(log, data.calls || [], data.reactions || []);
      const total = sumCalls(data.calls || [], ["understand", "select", "loop"]);
      detail.innerHTML =
        "<p class='question'>" + esc(log.question) + "</p>" +
        "<div class='muted'>" + esc(log.at.replace("T", " ").slice(0, 16)) + " · " + sec(log.ms) + " · " + usd(total ? total.cost : 0, 5) +
          (log.prev_page ? " · 直前のページ: " + esc(log.prev_page) : "") + "</div>" +
        renderFlow(a.stages) +
        "<div class='hint' style='--c:" + STATE_COLOR[a.hint.st] + "'><b>" + esc(a.hint.title) + "</b>" + esc(a.hint.text) + "</div>" +
        "<h2>ユーザーに返した答え</h2>" + renderAnswer(log.body) +
        "<details" + (a.hint.st !== "ok" ? " open" : "") + "><summary>LLM に見せた候補(" + (log.candidates || []).length + " 件)</summary>" + renderCandidates(log.candidates) + "</details>" +
        "<details><summary>理解(1 回目の LLM)</summary>" + renderUnderstanding(log.understanding) + "</details>" +
        "<details><summary>落としたもの・経路メモ</summary>" + renderDropped(log.body) + "</details>" +
        "<details><summary>LLM の呼び出しと費用</summary>" + renderCalls(data.calls) + "</details>" +
        "<details><summary>前後のやりとり(同じ端末)</summary>" + renderConversation(log, data.neighbors || {}) + "</details>" +
        "<details><summary>応答の JSON そのまま</summary><pre>" + esc(JSON.stringify(log.body, null, 2)) + "</pre></details>";
    } catch (e) {
      detail.innerHTML = "<p>読み込みに失敗: " + esc(e.message) + "</p>";
    }
  }

  // ---- 起動 ---------------------------------------------------------------------

  document.querySelectorAll("#range button").forEach((b) => b.addEventListener("click", () => {
    state.days = Number(b.dataset.d);
    document.querySelectorAll("#range button").forEach((x) => x.classList.toggle("on", x === b));
    loadSummary();
  }));
  document.querySelectorAll("#tabs button").forEach((b) => b.addEventListener("click", () => setTab(b.dataset.t)));
  qs("more").addEventListener("click", () => loadLogs(false));

  loadSummary();
  loadLogs(true);
})();
</script>
</body>
</html>
`;
