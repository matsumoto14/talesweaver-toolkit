/** 管理画面の静的 HTML(ビルド無し。インライン CSS/JS のみ、外部読み込み無し)。 */
export const ADMIN_HTML = `<!doctype html>
<html lang="ja">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>wiki に聞く — 監視</title>
<style>
  :root {
    color-scheme: light dark;
    --bg: #f7f7f8; --panel: #ffffff; --border: #dcdce0; --text: #1c1c1f; --muted: #6b6b74;
    --accent: #3457d5; --good: #1f8a4c; --bad: #c0392b; --warn: #b8860b;
  }
  @media (prefers-color-scheme: dark) {
    :root { --bg: #17171a; --panel: #202024; --border: #34343a; --text: #ececef; --muted: #9a9aa2; --accent: #7f9bff; }
  }
  * { box-sizing: border-box; }
  body { margin: 0; background: var(--bg); color: var(--text); font: 13px/1.5 -apple-system, "Segoe UI", sans-serif; }
  header { padding: 12px 16px; border-bottom: 1px solid var(--border); display: flex; align-items: center; gap: 12px; }
  header h1 { font-size: 15px; margin: 0; }
  main { display: grid; grid-template-columns: 1fr; gap: 12px; padding: 12px; }
  @media (min-width: 900px) { main.with-detail { grid-template-columns: minmax(0, 1.4fr) minmax(0, 1fr); } }
  section.panel { background: var(--panel); border: 1px solid var(--border); border-radius: 8px; padding: 12px; }
  h2 { font-size: 12px; text-transform: uppercase; letter-spacing: .04em; color: var(--muted); margin: 0 0 8px; }
  table { width: 100%; border-collapse: collapse; }
  td, th { padding: 4px 6px; text-align: left; border-bottom: 1px solid var(--border); font-variant-numeric: tabular-nums; }
  th { color: var(--muted); font-weight: 500; }
  tbody tr:hover { background: rgba(127,127,127,.08); cursor: pointer; }
  tbody tr.selected { background: rgba(52,87,213,.12); }
  .num { text-align: right; font-variant-numeric: tabular-nums; }
  .filters { display: flex; flex-wrap: wrap; gap: 8px; margin-bottom: 10px; }
  .filters input, .filters select { background: var(--bg); color: var(--text); border: 1px solid var(--border); border-radius: 6px; padding: 4px 8px; font: inherit; }
  .filters button { border: 1px solid var(--border); background: var(--panel); color: var(--text); border-radius: 6px; padding: 4px 10px; font: inherit; cursor: pointer; }
  .muted { color: var(--muted); }
  .tag { display: inline-block; padding: 1px 6px; border-radius: 999px; font-size: 11px; background: rgba(127,127,127,.15); }
  .tag.answer { color: var(--good); } .tag.error { color: var(--bad); } .tag.none { color: var(--warn); }
  pre { white-space: pre-wrap; word-break: break-word; background: var(--bg); border: 1px solid var(--border); border-radius: 6px; padding: 8px; max-height: 240px; overflow: auto; }
  .row-detail { display: flex; flex-direction: column; gap: 12px; }
  .kv { display: grid; grid-template-columns: auto 1fr; gap: 4px 10px; }
  .kv dt { color: var(--muted); }
  .kv dd { margin: 0; }
  .totals { display: flex; gap: 16px; margin-bottom: 8px; flex-wrap: wrap; }
  .totals div b { display: block; font-size: 16px; }
  #pager { margin-top: 8px; text-align: center; }
  .empty { color: var(--muted); padding: 8px 0; }
</style>
</head>
<body>
<header>
  <h1>wiki に聞く — 監視</h1>
  <span class="muted" id="status"></span>
</header>
<main id="main">
  <section class="panel">
    <h2>日別の費用</h2>
    <div class="totals" id="cost-totals"></div>
    <table id="cost-table"><thead><tr>
      <th>日付</th><th class="num">質問</th><th class="num">呼び出し</th><th class="num">トークン</th><th class="num">費用</th><th class="num">1 問あたり</th>
    </tr></thead><tbody></tbody></table>
  </section>

  <section class="panel">
    <h2>ログ</h2>
    <div class="filters">
      <input type="date" id="f-from" title="from">
      <input type="date" id="f-to" title="to">
      <select id="f-kind"><option value="">kind: すべて</option><option value="answer">answer</option><option value="none">none</option><option value="error">error</option></select>
      <select id="f-route"><option value="">route: すべて</option><option value="cheap">cheap</option><option value="loop">loop</option><option value="cheap_then_loop">cheap_then_loop</option><option value="cached">cached</option></select>
      <select id="f-reaction"><option value="">reaction: すべて</option><option value="helpful">helpful</option><option value="wrong">wrong</option><option value="value_wrong">value_wrong</option></select>
      <button id="f-apply">絞る</button>
    </div>
    <table id="log-table"><thead><tr>
      <th>at</th><th>question</th><th>kind</th><th>route</th><th class="num">ms</th><th class="num">呼び出し</th><th class="num">費用</th><th>反応</th>
    </tr></thead><tbody></tbody></table>
    <div id="pager"><button id="more">さらに読む</button></div>
  </section>
</main>

<script>
(function () {
  const state = { before: null, filters: {}, selectedId: null };

  function qs(id) { return document.getElementById(id); }
  function esc(s) { return String(s ?? "").replace(/[&<>"']/g, (c) => ({"&":"&amp;","<":"&lt;",">":"&gt;",'"':"&quot;","'":"&#39;"}[c])); }
  function fmtUsd(v) { return v === null || v === undefined ? "?" : "$" + v.toFixed(4); }
  function fmtNum(v) { return v === null || v === undefined ? "?" : Number(v).toLocaleString("ja-JP"); }

  async function api(path) {
    const res = await fetch(path, { headers: { accept: "application/json" } });
    if (!res.ok) throw new Error("HTTP " + res.status);
    return res.json();
  }

  async function loadCosts() {
    try {
      const data = await api("/api/costs?days=30");
      qs("cost-totals").innerHTML =
        "<div>直近30日の費用<b>" + fmtUsd(data.total_cost_usd) + "</b></div>" +
        "<div>上位ユーザー<b>" + (data.top_users[0] ? esc(data.top_users[0].user.slice(0, 8)) + " " + fmtUsd(data.top_users[0].cost_usd) : "-") + "</b></div>";
      const tbody = document.querySelector("#cost-table tbody");
      tbody.innerHTML = data.days.map((d) =>
        "<tr><td>" + esc(d.date) + "</td><td class='num'>" + fmtNum(d.questions) + "</td><td class='num'>" + fmtNum(d.calls) +
        "</td><td class='num'>" + fmtNum(d.input_tokens + d.cache_read_tokens + d.cache_creation_tokens + d.output_tokens) +
        "</td><td class='num'>" + fmtUsd(d.cost_usd) + "</td><td class='num'>" + fmtUsd(d.avg_cost_usd) + "</td></tr>"
      ).join("") || "<tr><td colspan=6 class='empty'>データがありません</td></tr>";
    } catch (e) {
      qs("status").textContent = "費用の読み込みに失敗: " + e.message;
    }
  }

  function reactionBadges(r) {
    const parts = [];
    if (r.helpful) parts.push("helpful " + r.helpful);
    if (r.wrong) parts.push("wrong " + r.wrong);
    if (r.value_wrong) parts.push("value_wrong " + r.value_wrong);
    return parts.join(" / ");
  }

  function buildQuery(before) {
    const p = new URLSearchParams();
    if (state.filters.from) p.set("from", state.filters.from);
    if (state.filters.to) p.set("to", state.filters.to + "T23:59:59Z");
    if (state.filters.kind) p.set("kind", state.filters.kind);
    if (state.filters.route) p.set("route", state.filters.route);
    if (state.filters.reaction) p.set("reaction", state.filters.reaction);
    if (before) p.set("before", before);
    return p.toString();
  }

  async function loadLogs(reset) {
    if (reset) { state.before = null; document.querySelector("#log-table tbody").innerHTML = ""; }
    try {
      const data = await api("/api/logs?" + buildQuery(state.before));
      const tbody = document.querySelector("#log-table tbody");
      for (const row of data.logs) {
        const tr = document.createElement("tr");
        tr.dataset.id = row.id;
        tr.innerHTML =
          "<td>" + esc(row.at.replace("T", " ").slice(0, 19)) + "</td>" +
          "<td>" + esc(row.question) + "</td>" +
          "<td><span class='tag " + esc(row.kind) + "'>" + esc(row.kind) + (row.reason ? " " + esc(row.reason) : "") + "</span></td>" +
          "<td>" + esc(row.route ?? "-") + "</td>" +
          "<td class='num'>" + fmtNum(row.ms) + "</td>" +
          "<td class='num'>" + fmtNum(row.calls) + "</td>" +
          "<td class='num'>" + fmtUsd(row.cost_usd) + "</td>" +
          "<td class='muted'>" + esc(reactionBadges(row.reactions)) + "</td>";
        tr.addEventListener("click", () => selectLog(row.id));
        tbody.appendChild(tr);
      }
      state.before = data.next;
      qs("more").style.display = data.next ? "" : "none";
      if (data.logs.length === 0 && reset) tbody.innerHTML = "<tr><td colspan=8 class='empty'>該当するログがありません</td></tr>";
    } catch (e) {
      qs("status").textContent = "ログの読み込みに失敗: " + e.message;
    }
  }

  function renderCandidates(list) {
    if (!list || list.length === 0) return "<p class='muted'>(なし)</p>";
    return "<table><thead><tr><th>id</th><th>page</th><th>section</th></tr></thead><tbody>" +
      list.map((c) => "<tr><td>" + esc(c.id) + "</td><td>" + esc(c.page) + "</td><td>" + esc(c.section) + "</td></tr>").join("") +
      "</tbody></table>";
  }

  function renderCalls(calls) {
    if (!calls || calls.length === 0) return "<p class='muted'>(呼び出しなし)</p>";
    return "<table><thead><tr><th>#</th><th>kind</th><th>model</th><th class='num'>in</th><th class='num'>cache読</th><th class='num'>cache書</th><th class='num'>out</th><th class='num'>ms</th><th class='num'>費用</th></tr></thead><tbody>" +
      calls.map((c) =>
        "<tr><td>" + c.seq + "</td><td>" + esc(c.kind) + "</td><td>" + esc(c.model) + "</td><td class='num'>" + fmtNum(c.input_tokens) +
        "</td><td class='num'>" + fmtNum(c.cache_read_tokens) + "</td><td class='num'>" + fmtNum(c.cache_creation_tokens) +
        "</td><td class='num'>" + fmtNum(c.output_tokens) + "</td><td class='num'>" + fmtNum(c.ms) + "</td><td class='num'>" + fmtUsd(c.cost_usd) + "</td></tr>"
      ).join("") + "</tbody></table>";
  }

  function renderNeighbors(list) {
    if (!list || list.length === 0) return "<p class='muted'>(なし)</p>";
    return "<ul>" + list.map((n) => "<li>" + esc(n.at.slice(11, 19)) + " [" + esc(n.kind) + "] " + esc(n.question) + "</li>").join("") + "</ul>";
  }

  async function selectLog(id) {
    state.selectedId = id;
    document.querySelectorAll("#log-table tbody tr").forEach((tr) => tr.classList.toggle("selected", Number(tr.dataset.id) === id));
    qs("main").classList.add("with-detail");
    let detail = qs("detail-panel");
    if (!detail) {
      detail = document.createElement("section");
      detail.className = "panel";
      detail.id = "detail-panel";
      qs("main").appendChild(detail);
    }
    detail.innerHTML = "<h2>読み込み中…</h2>";
    try {
      const data = await api("/api/logs/" + id);
      const log = data.log;
      detail.innerHTML =
        "<h2>#" + log.id + " の詳細</h2>" +
        "<div class='row-detail'>" +
        "<dl class='kv'>" +
        "<dt>質問</dt><dd>" + esc(log.question) + "</dd>" +
        "<dt>kind / reason</dt><dd>" + esc(log.kind) + (log.reason ? " / " + esc(log.reason) : "") + "</dd>" +
        "<dt>route</dt><dd>" + esc(log.route ?? "-") + "</dd>" +
        "<dt>lead</dt><dd>" + esc(log.lead ?? "-") + "</dd>" +
        "<dt>ms</dt><dd>" + fmtNum(log.ms) + "</dd>" +
        "<dt>user</dt><dd>" + esc(log.user.slice(0, 12)) + "…</dd>" +
        "</dl>" +
        "<div><h2>理解</h2><pre>" + esc(JSON.stringify(log.understanding, null, 2)) + "</pre></div>" +
        "<div><h2>候補</h2>" + renderCandidates(log.candidates) + "</div>" +
        "<div><h2>呼び出し</h2>" + renderCalls(data.calls) + "</div>" +
        "<div><h2>答え(body)</h2><pre>" + esc(JSON.stringify(log.body, null, 2)) + "</pre></div>" +
        "<div><h2>反応</h2><pre>" + esc(JSON.stringify(data.reactions, null, 2)) + "</pre></div>" +
        "<div><h2>直前</h2>" + renderNeighbors(data.neighbors.before) + "</div>" +
        "<div><h2>直後</h2>" + renderNeighbors(data.neighbors.after) + "</div>" +
        "</div>";
    } catch (e) {
      detail.innerHTML = "<h2>読み込みに失敗</h2><p>" + esc(e.message) + "</p>";
    }
  }

  qs("f-apply").addEventListener("click", () => {
    state.filters = {
      from: qs("f-from").value, to: qs("f-to").value, kind: qs("f-kind").value,
      route: qs("f-route").value, reaction: qs("f-reaction").value,
    };
    loadLogs(true);
  });
  qs("more").addEventListener("click", () => loadLogs(false));

  loadCosts();
  loadLogs(true);
})();
</script>
</body>
</html>
`;
