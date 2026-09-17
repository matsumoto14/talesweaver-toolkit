r"""インクリシミュレータの画面素材(UI 部品・結果演出・効果音)をクライアント展開データから同梱する。

出どころ(2026-09-17 特定):
- 装備システムウィンドウ(exe の CImproveWin)。部品のテクスチャはアニメ 64113(9157 = タブ)と
  64456(9164 = インクリ/エンチャントのパネル・ボタン・ラジオ)が束ねている。アニメ内の座標は全部 0 で、
  画面上の配置は exe のコードにしか無い → 配置はゲーム内ガイド画像(0165__16557 #7)から測って Svelte 側に書く。
- 結果演出はアニメ 53576: 3 = SUCCESS(6800、SE 806)、4 = FAIL(7990、SE 829)。
  結果は押した瞬間に出て、演出は本体の説明文の中央に通常の重ね方で出る(ユーザーの録画 2026-09-17)。
  コマの表示 tick・dx/dy はアニメから、フレームの描画オフセットは .dtx のフレームヘッダから取る。
- ボタン SE 0080 / タブ SE 0077 はアニメ 64113 の音コマンド。

`.dtx` の読み方は tw_tool_v2 の `tools/dat_sprite_decode.py`、`.d2a` は `tools/dat_anim_dump.py` をそのまま使う
(形式の知見はそちらが正)。

出力: apps/desktop/src/assets/inkri/
  ui/<name>.png     静的な部品(状態違いは _0,_1,.. の連番)
  fx/<name>.webp    演出のコマを横に並べたシート
  fx/<name>.json    {"frames":[{x,y,w,h,ox,oy}], "timeline":[{track,frame,start,hold,dx,dy}],
                     "fades":[{track,start,ticks}]}
                    start/hold/ticks は tick 単位。描画左上 = 基準点 + (ox,oy) + (dx,dy)
  se/<name>.wav     効果音(RIFF PCM のまま)

使い方:
    python tools/gamedata/import_inkri_ui.py [--assets PATH] [--tool PATH]
`--assets` 省略時は環境変数 TW_ASSETS → C:\github\private\tw_assets、
`--tool` 省略時は環境変数 TW_TOOL_V2 → C:\github\private\tw_tool_v2。
"""

from __future__ import annotations

import argparse
import glob
import json
import os
import shutil
import sys
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "apps/desktop/src/assets/inkri"

# 静的部品: 出力名 → (テクスチャ, [アトラスのフレーム番号 = アニメの frame 値])
UI_PARTS: dict[str, tuple[int, list[int]]] = {
    # タブ(アニメ 64113 の 3〜9。状態は 通常 / 選択 / 使用不可)
    "tab_improve": (9157, [3, 4, 4]),
    "tab_evolve": (9157, [5, 6, 7]),
    "tab_element": (9157, [8, 9, 10]),
    "tab_ability": (9157, [11, 12, 13]),
    "tab_mr": (9157, [14, 15, 16]),
    "tab_enchant": (9157, [17, 18, 19]),
    "tab_inkri": (9157, [20, 21, 22]),
    # オーラのタブ(アニメ 64113 の 26。2 状態)
    "tab_aura": (15888, [0, 1, 1]),
    # インクリのパネル(スロット + インクリ選択の一覧)。アニメ 64456 の 22 番の土台
    "panel": (9164, [57]),
    # 「インクリ」ボタン(アニメ 64456 の 25: 通常 / ホバー / 押下 / 使用不可)
    "button_inkri": (9164, [59, 60, 61, 62]),
    # ラジオ(アニメ 64456 の 5: 未選択 / 選択)
    "radio": (9164, [13, 14]),
}

# 演出: 出力名 → (アニメファイル, アニメ番号)
FX_ANIMS: dict[str, tuple[str, int]] = {
    "success": ("dt_00017/AnimationFile/0535/53576.d2a", 3),
    "fail": ("dt_00017/AnimationFile/0535/53576.d2a", 4),
}

SHEET_MAX = 4096

SOUNDS: dict[str, str] = {
    "success": "dk_00010/SEFile/0806.wav",
    "fail": "dk_00011/SEFile/0829.wav",
    "button": "dk_00000/SEFile/0080.wav",
    "tab": "dk_00000/SEFile/0077.wav",
}


def env_path(value: str | None, env: str, default: str) -> Path:
    return Path(value or os.environ.get(env) or default)


class Textures:
    def __init__(self, assets: Path, decoder) -> None:
        self.assets = assets
        self.decoder = decoder
        self.cache: dict[int, tuple[object, Image.Image]] = {}

    def get(self, tex: int):
        if tex not in self.cache:
            bucket = f"{tex // 100:04d}"
            hits = glob.glob(str(self.assets / "raw" / "da_*" / "TextureFile" / bucket / f"{tex}.dtx"))
            if not hits:
                raise SystemExit(f"テクスチャ {tex}.dtx が raw/ に無い")
            spr = self.decoder.parse_atlas_sprite(Path(hits[0]).read_bytes())
            self.cache[tex] = (spr, self.decoder.atlas_texture_to_image(spr))
        return self.cache[tex]

    def frame(self, tex: int, index: int):
        spr, img = self.get(tex)
        fr = spr.frames[index]
        crop = img.crop((fr.x, fr.y, fr.x + fr.w, fr.y + fr.h))
        # 描画オフセットは「次のフレーム」の ObjHdr に入っている(最後のフレームはテクスチャのヘッダ)。
        # dat_sprite_decode は各フレームの手前の ObjHdr をそのフレームのものとして読むが、そのまま使うと
        # 演出のコマごとに位置が 30px 跳ぶ。1 つずらすと 7990(FAIL)/6800(SUCCESS)の中心がなめらかに揃う
        hdr = spr.frames[index + 1].hdr if index + 1 < len(spr.frames) else spr.tex_hdr
        return crop, int(hdr.offx), int(hdr.offy)


def export_ui(tex: Textures) -> None:
    out = OUT / "ui"
    out.mkdir(parents=True, exist_ok=True)
    for name, (t, frames) in UI_PARTS.items():
        for i, f in enumerate(frames):
            crop, _, _ = tex.frame(t, f)
            path = out / (f"{name}_{i}.png" if len(frames) > 1 else f"{name}.png")
            crop.save(path, optimize=True)


def export_fx(tex: Textures, anim_dump, assets: Path) -> None:
    out = OUT / "fx"
    out.mkdir(parents=True, exist_ok=True)
    for name, (rel, anim_no) in FX_ANIMS.items():
        doc = anim_dump.parse_d2a((assets / "raw" / rel).read_bytes())
        anim = doc["anims"][anim_no]
        sprs: list[dict] = []
        fades: list[dict] = []
        for track_no, track in enumerate(anim["dirs"][0]["frames"]):
            cmds = track["cmds"]
            for i, c in enumerate(cmds):
                if c["type"] == 0:
                    hold = cmds[i + 1]["value"] + 1 if i + 1 < len(cmds) and cmds[i + 1]["type"] == 1 else 1
                    sprs.append({"track": track_no, "tex": c["sprite"], "frame": c["frame"], "start": c["time"],
                                 "hold": hold, "dx": c.get("dx", 0), "dy": c.get("dy", 0)})
                elif c["type"] == 9:
                    # AlphaFade: b3 = [かける tick, ?, 対象トラック](SUCCESS/FAIL の見え方からの推定)
                    fades.append({"track": c["b3"][2], "start": c["time"], "ticks": c["b3"][0]})
        # トラックは同時に重なって再生される(FAIL は爆発の残像と文字が別トラック)
        sprs.sort(key=lambda s: (s["start"], s["frame"]))
        unique = sorted({(s["tex"], s["frame"]) for s in sprs}, key=lambda k: k[1])
        crops = [tex.frame(t, f) for t, f in unique]
        # 横に並べて SHEET_MAX で折り返す。1 枚が GPU のテクスチャ上限(多くは 8192)を超えると
        # canvas の drawImage が遅い経路に落ちて演出がカクつく
        places, x, y, row_h = [], 0, 0, 0
        for img, _, _ in crops:
            if x > 0 and x + img.width > SHEET_MAX:
                x, y, row_h = 0, y + row_h + 2, 0
            places.append((x, y))
            x += img.width + 2
            row_h = max(row_h, img.height)
        width = max(px + img.width for (px, _), (img, _, _) in zip(places, crops))
        sheet = Image.new("RGBA", (width, y + row_h), (0, 0, 0, 0))
        frames = []
        for (px, py), (img, ox, oy) in zip(places, crops):
            sheet.paste(img, (px, py))
            frames.append({"x": px, "y": py, "w": img.width, "h": img.height, "ox": ox, "oy": oy})
        index = {k: i for i, k in enumerate(unique)}
        timeline = [{"track": s["track"], "frame": index[(s["tex"], s["frame"])], "start": s["start"], "hold": s["hold"],
                     "dx": s["dx"], "dy": s["dy"]} for s in sprs]
        sheet.save(out / f"{name}.webp", lossless=False, quality=88, method=6)
        (out / f"{name}.json").write_text(
            json.dumps({"frames": frames, "timeline": timeline, "fades": fades}, ensure_ascii=False, indent=1),
            encoding="utf-8")


def export_tab_icon(assets: Path) -> None:
    # アプリ上部のタブに出すアイコン。「ビアヌのインクリスクロール」(ItemId 1047313)の絵
    hits = glob.glob(str(assets / "item_icons" / "1047313_*.png"))
    if not hits:
        raise SystemExit("item_icons に 1047313(ビアヌのインクリスクロール)が無い")
    shutil.copyfile(hits[0], OUT / "ui" / "app_tab.png")


def export_se(assets: Path) -> None:
    out = OUT / "se"
    out.mkdir(parents=True, exist_ok=True)
    for name, rel in SOUNDS.items():
        shutil.copyfile(assets / "raw" / rel, out / f"{name}.wav")


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    ap.add_argument("--assets")
    ap.add_argument("--tool")
    args = ap.parse_args()
    assets = env_path(args.assets, "TW_ASSETS", r"C:\github\private\tw_assets")
    tool = env_path(args.tool, "TW_TOOL_V2", r"C:\github\private\tw_tool_v2")
    sys.path.insert(0, str(tool / "tools"))
    import dat_anim_dump  # noqa: E402
    import dat_sprite_decode  # noqa: E402

    tex = Textures(assets, dat_sprite_decode)
    export_ui(tex)
    export_fx(tex, dat_anim_dump, assets)
    export_se(assets)
    export_tab_icon(assets)
    print(f"wrote {OUT}")


if __name__ == "__main__":
    main()
