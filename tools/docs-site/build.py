"""人向け docs(README + docs/ の 4 本)を docs/site/ の HTML に変換する。

    python tools/docs-site/build.py

- md が正。HTML は生成物で、md を変えたら再実行して commit する
- 依存: python-markdown(`pip install markdown`)
- 画像は docs/screenshots/ を相対参照する(コピーしない)
- docs/claude/ 配下(Claude 向け)は対象外。リンクは md ファイルへそのまま飛ばす
"""
from __future__ import annotations

import html
import pathlib
import re

import markdown

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "docs" / "site"

# (md パス, 出力名, ナビ表示名, 1 行説明)
PAGES = [
    ("README.md", "index", "概要", "何ができるか・セットアップ"),
    ("docs/architecture.md", "architecture", "アーキテクチャ", "クレート構成・フロント階層・依存の向き"),
    ("docs/ux-guidelines.md", "ux-guidelines", "UX ガイドライン", "UI を作るときの 4 原則"),
    ("docs/damage-formula.md", "damage-formula", "ダメージ計算仕様", "talewiki を整理した計算モデル"),
    ("docs/status.md", "status", "進捗", "goal ごとの到達点"),
]
SLUG_OF = {md: slug for md, slug, _, _ in PAGES}
CLAUDE_DOCS = [
    ("docs/claude/decisions.md", "決定記録"),
    ("docs/claude/goals/", "goal 文書"),
    ("docs/claude/workflow.md", "Claude Code 運用"),
    ("docs/legacy-twtoolkit.md", "旧リポ棚卸し"),
]


def md_to_html(src: str, md_path: str) -> tuple[str, str, str]:
    """-> (h1, toc_html, body_html)"""
    md = markdown.Markdown(
        extensions=["tables", "fenced_code", "toc", "sane_lists"],
        extension_configs={"toc": {"toc_depth": "2-2", "anchorlink": False}},
    )
    body = md.convert(src)

    m = re.search(r"<h1[^>]*>(.*?)</h1>", body, re.S)
    h1 = m.group(1) if m else ""
    body = body[: m.start()] + body[m.end():] if m else body

    # --- リンク解決(docs 相対 → site 相対) -------------------------------
    here = pathlib.PurePosixPath(md_path).parent

    def to_site(target: str) -> str:
        path, _, frag = target.partition("#")
        if not path:
            return target
        norm = pathlib.PurePosixPath(*(p for p in (here / path).parts if p != "."))
        norm_s = str(norm).replace("docs/../", "")
        if norm_s in SLUG_OF:
            return f"{SLUG_OF[norm_s]}.html" + (f"#{frag}" if frag else "")
        # site/ から見た相対パス(docs/ 配下なら ../、ルートなら ../../)
        return "../../" + norm_s if not norm_s.startswith("docs/") else "../" + norm_s[len("docs/"):]

    body = re.sub(
        r'(href|src)="((?!https?://|#|mailto:)[^"]+)"',
        lambda mm: f'{mm.group(1)}="{to_site(mm.group(2))}"',
        body,
    )
    # リンクになっていない "docs/xxx.md" の素テキストもリンクにする
    body = re.sub(
        r'(?<![">/\w])(docs/[\w./-]+\.md)(?![^<]*</a>)',
        lambda mm: f'<a href="{to_site("../" + mm.group(1) if here != pathlib.PurePosixPath(".") else mm.group(1))}">{mm.group(1)}</a>',
        body,
    )

    # --- 共通の整形 -------------------------------------------------------
    body = body.replace("<table>", '<div class="tbl"><table>').replace("</table>", "</table></div>")
    body = body.replace("<code>[仮]</code>", '<span class="tag warm">仮</span>')
    body = re.sub(r"<code>\[更新済 → (.*?)\]</code>", r'<span class="tag">更新済 → \1</span>', body)
    body = re.sub(r"<p><strong>(理由|判断の問い)</strong>: (.*?)</p>",
                  r'<p class="callout \1"><span class="k">\1</span>\2</p>', body, flags=re.S)
    body = body.replace("<li>[ ] ", '<li class="check">')

    toc = md.toc if "<li>" in md.toc else ""
    toc = re.sub(r'^<div class="toc">|</div>\s*$', "", toc.strip())
    return h1, toc, body


# --- ページ固有の整形 ---------------------------------------------------------

def tweak_index(body: str) -> str:
    # 画像をヒーローに
    body = re.sub(r'<p><img alt="([^"]*)" src="([^"]+)"\s*/?></p>',
                  r'<figure class="hero"><img alt="\1" src="\2"><figcaption>\1(1280×840、docs/screenshots/26-damage.png)</figcaption></figure>',
                  body, count=1)
    # 「現在できること」の 3 項目をカードに
    def cards(mm):
        items = re.findall(r"<li><strong>(.*?)</strong> — (.*?)</li>", mm.group(1), re.S)
        return '<div class="cards">' + "".join(f"<div class=\"card\"><h3>{t}</h3><p>{d}</p></div>" for t, d in items) + "</div>"
    body = re.sub(r'(?<=<h2 id="_1">現在できること</h2>\n)<ul>(.*?)</ul>', lambda mm: cards(mm), body, count=1, flags=re.S)
    return body


def tweak_formula(body: str) -> str:
    # §3 の与ダメージ式を主役に
    body = re.sub(r'(<h2 id="3">.*?</h2>\n)<pre><code>(.*?)</code></pre>',
                  r'\1<pre class="formula"><code>\2</code></pre>', body, count=1, flags=re.S)
    # カテゴリ表: 種別をチップに、記号をモノスペースに
    kinds = {"代入": "assign", "固定値": "fixed", "割合": "rate"}
    def chip(mm):
        k = mm.group(1)
        return f'<td><span class="kind {kinds[k]}">{k}</span></td>'
    body = re.sub(r"<td>(代入|固定値|割合)</td>", chip, body)
    body = re.sub(r"<tr>\n<td>([A-Z][A-Za-z0-9]{0,3})</td>", r'<tr>\n<td class="sym">\1</td>', body)
    return body


def tweak_status(body: str) -> str:
    body = re.sub(r"<li>(\d{4}-\d{2}-\d{2}) (.*?)</li>",
                  r'<li class="event"><time>\1</time><div>\2</div></li>', body, flags=re.S)
    return body.replace("<ul>\n<li class=\"event\">", "<ul class=\"timeline\">\n<li class=\"event\">", 1)


TWEAKS = {"index": tweak_index, "damage-formula": tweak_formula, "status": tweak_status}

# --- テンプレート -------------------------------------------------------------

CSS = (pathlib.Path(__file__).parent / "site.css").read_text("utf-8")

TEMPLATE = """<!doctype html>
<html lang="ja">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · TalesWeaver Toolkit</title>
<link rel="preconnect" href="https://fonts.googleapis.com">
<link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=M+PLUS+1+Code:wght@400;600&family=Noto+Sans+JP:wght@400;700&display=swap">
<style>{css}</style>
</head>
<body>
<div class="shell">
<aside class="side">
  <a class="brand" href="index.html"><span class="dot"></span>TalesWeaver Toolkit</a>
  <nav class="site-nav">
    <div class="label">DOCS</div>
    {nav}
    <div class="label">CLAUDE 向け(md)</div>
    {claude_nav}
  </nav>
</aside>
<main class="main">
  <article class="doc">
    <header class="doc-head">
      <div class="crumb">{crumb}</div>
      <h1>{h1}</h1>
    </header>
    {toc}
    {body}
    <footer class="doc-foot">原本: <code>{md}</code>(md が正。HTML は <code>tools/docs-site/build.py</code> の生成物)</footer>
  </article>
</main>
</div>
</body>
</html>
"""


def build() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for md_path, slug, nav_name, desc in PAGES:
        src = (ROOT / md_path).read_text("utf-8")
        h1, toc, body = md_to_html(src, md_path)
        body = TWEAKS.get(slug, lambda b: b)(body)
        nav = "".join(
            f'<a href="{s}.html"{" class=\"current\"" if s == slug else ""}><span>{n}</span><small>{d}</small></a>'
            for _, s, n, d in PAGES
        )
        claude_nav = "".join(
            f'<a href="../../{p}" class="ext"><span>{n}</span><small>{p}</small></a>' for p, n in CLAUDE_DOCS
        )
        toc_html = f'<nav class="toc"><div class="label">この文書</div>{toc}</nav>' if toc else ""
        page = TEMPLATE.format(
            title=html.escape(nav_name), css=CSS, nav=nav, claude_nav=claude_nav,
            crumb=md_path, h1=h1, toc=toc_html, body=body, md=md_path,
        )
        (OUT / f"{slug}.html").write_text(page, "utf-8", newline="\n")
        print(f"{slug:16s} {len(page):6d} bytes")


if __name__ == "__main__":
    build()
