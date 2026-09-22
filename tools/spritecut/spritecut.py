r"""動き案のシートをコマに切り、アニメ GIF にする。**色は原画のまま。**

原画(1536x1024 の RGBA)は 4x2 のコマ割りで、背景はもともと透過。
この道具がやるのは「切る・並べる・GIF にする」だけで、**減色のパレットを決め打ちしたり、
塊を消したり、穴を埋めたり、孤立ピクセルを直したりはしない**
(ユーザー決定 2026-09-23「加工は色の話。切り抜いて GIF にしてよい」)。

注意 1: 透過部分の RGB には暗い色が焼き込まれている。`convert('RGB')` すると背景が
付いているように見えるので、alpha を落とさないこと。

注意 2: GIF の透過は 1bit(透ける/透けないの 2 値)しかない。原画のふちは半透明なので、
透過のまま GIF にするとふちがギザギザになる。そこで `--matte` で**置き先の地の色に
焼いてから** GIF にする。アプリの「調べる」タブの地は 1 色(`--bg-mid: #DEEBF7`)なので
これで成立する。地が変わる面に置くならこの道具は使えない。

コマの切り出しは**渡した全シートの全コマに共通の外接矩形**で行う。動きが変わっても
キャラの位置と大きさが飛ばないようにするため。

原画はコマごとに絵の位置も大きさも違う(待機でも底が 29px、笑顔では 97px ずれている)。
そのまま回すと足元がふよふよ動くので、`--align` で全コマの足元を揃える。動かすだけで
絵そのものは変えない。

    python tools/spritecut/spritecut.py idle.png joy.png -o out/ \
        --grid 4x2 --height 192 --matte DEEBF7 \
        --ms idle=250,joy=120
"""
from __future__ import annotations

import argparse
import os
import sys

try:
    import numpy as np
    from PIL import Image
    from scipy import ndimage
except ImportError:  # pragma: no cover
    sys.exit('Pillow・numpy・scipy が要る: pip install pillow numpy scipy')


def cells(src: Image.Image, cols: int, rows: int) -> list[Image.Image]:
    w, h = src.width // cols, src.height // rows
    return [src.crop((c * w, r * h, (c + 1) * w, (r + 1) * h))
            for r in range(rows) for c in range(cols)]


def footing(im: Image.Image) -> tuple[float, int]:
    """足元の位置(横の中心, 底の y)。原画はコマごとに絵の位置も大きさも違うので、
    ここを揃えないと再生したときに足元がふよふよ動く(ユーザー指摘 2026-09-23)。

    測るのは**体だけ**。「!」「?」のような離れた効果や、まわりの淡い光を混ぜると
    位置が飛ぶので、濃い部分(alpha>=200)の最大の塊を体とみなす。
    横の中心は体の下 12px の帯で測る(羽の出方で中心が振れないように)。"""
    solid = np.asarray(im.getchannel('A')) >= 200
    lab, n = ndimage.label(solid)
    if n == 0:
        raise ValueError('コマが空')
    body = lab == (np.bincount(lab.ravel())[1:].argmax() + 1)
    ys = np.where(body.any(axis=1))[0]
    bottom = int(ys.max())
    band = body[max(0, bottom - 11):bottom + 1]
    xs = np.where(band.any(axis=0))[0]
    return (float(xs.min() + xs.max()) / 2, bottom)


def align(frames: list[Image.Image]) -> list[Image.Image]:
    """全コマの足元を、みんなの真ん中の位置に寄せる。動かすだけで絵は変えない"""
    feet = [footing(im) for im in frames]
    tx = float(np.median([f[0] for f in feet]))
    ty = float(np.median([f[1] for f in feet]))
    out = []
    for im, (cx, by) in zip(frames, feet):
        dx, dy = round(tx - cx), round(ty - by)
        moved = Image.new('RGBA', im.size, (0, 0, 0, 0))
        moved.paste(im, (dx, dy))
        out.append(moved)
    print('  足元を (%.1f, %.1f) に揃えた' % (tx, ty))
    return out


def union_box(frames: list[Image.Image], cut: int) -> tuple[int, int, int, int]:
    """全コマに共通の外接矩形。原画のまわりには alpha がごく薄い光が端まで伸びているので、
    `cut` 未満の alpha は無いものとして測る(切る位置が決まらないため)"""
    box: tuple[int, int, int, int] | None = None
    for im in frames:
        bb = im.getchannel('A').point(lambda a: 255 if a >= cut else 0).getbbox()
        if bb is None:
            continue
        box = bb if box is None else (min(box[0], bb[0]), min(box[1], bb[1]),
                                      max(box[2], bb[2]), max(box[3], bb[3]))
    if box is None:
        sys.exit('中身が空')
    return box


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description='シートを切って GIF にする(色は触らない)')
    ap.add_argument('inputs', nargs='+', help='動き 1 つにつきシート 1 枚')
    ap.add_argument('-o', '--out', required=True)
    ap.add_argument('--grid', default='4x2', help='COLSxROWS。既定 4x2')
    ap.add_argument('--height', type=int, help='書き出す高さ(px)。省略で原寸')
    ap.add_argument('--matte', help='この色に焼いてから GIF にする #RRGGBB')
    ap.add_argument('--alpha-cut', type=int, default=16,
                    help='切る位置を測るときに無視する alpha の上限。既定 16')
    ap.add_argument('--align', action='store_true',
                    help='全コマの足元を揃える(位置を動かすだけ。絵は変えない)')
    ap.add_argument('--ms', default='', help='1 コマの長さ。name=ms をカンマ区切り')
    ap.add_argument('--still', action='store_true',
                    help='1 コマ目を webp でも書く(動きを消す設定のとき用)')
    args = ap.parse_args(argv)

    cols, rows = (int(v) for v in args.grid.lower().split('x'))
    per = dict(kv.split('=') for kv in args.ms.split(',') if kv)
    os.makedirs(args.out, exist_ok=True)

    sheets: dict[str, list[Image.Image]] = {}
    for path in args.inputs:
        src = Image.open(path)
        if 'A' not in src.getbands():
            sys.exit('%s に alpha が無い。背景が透過の原画を渡すこと' % path)
        name = os.path.splitext(os.path.basename(path))[0]
        sheets[name] = cells(src.convert('RGBA'), cols, rows)

    if args.align:
        flat = align([f for fs in sheets.values() for f in fs])
        i = 0
        for name, fs in sheets.items():
            sheets[name] = flat[i:i + len(fs)]
            i += len(fs)

    box = union_box([f for fs in sheets.values() for f in fs], args.alpha_cut)
    print('共通の外接矩形: %s' % (box,))

    matte = None
    if args.matte:
        h = args.matte.lstrip('#')
        matte = tuple(int(h[i:i + 2], 16) for i in (0, 2, 4))

    for name, frames in sheets.items():
        out = []
        for im in frames:
            im = im.crop(box)
            if args.height:
                w = round(im.width * args.height / im.height)
                im = im.resize((w, args.height), Image.Resampling.LANCZOS)
            if matte:
                bg = Image.new('RGBA', im.size, matte + (255,))
                bg.alpha_composite(im)
                im = bg.convert('RGB')
            out.append(im)

        ms = int(per.get(name, 200))
        dst = os.path.join(args.out, name + '.gif')
        out[0].save(dst, save_all=True, append_images=out[1:], loop=0, duration=ms,
                    optimize=True)
        print('  %s  (%dx%d ×%d コマ / %dms)  %d KB'
              % (dst, out[0].width, out[0].height, len(out), ms,
                 os.path.getsize(dst) // 1024))

        if args.still:
            sp = os.path.join(args.out, name + '-still.webp')
            out[0].save(sp, lossless=True, quality=100, method=6)
            print('  %s  %d KB' % (sp, os.path.getsize(sp) // 1024))
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
