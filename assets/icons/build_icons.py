"""
资产包生成流程：以 logo/logo-master.png 为唯一输入，产出包内全部成品。

母版是「渐变底 + 带明暗的图形」，不是两色图，所以与早先的两色假设不同：
1) defringe  —— 把 alpha=0 像素的 RGB 填成邻近不透明像素的颜色（缩放时不渗进黑 RGB）
2) 缩放走 defringe -> LANCZOS -> 地板回抬 -> defringe，半透明边缘不被污染
3) 抠背景按**饱和度**分离，不是投影到两色连线 —— 图形一律不透明，
   明暗层次不会被翻译成透明度
"""
import io, os, struct
import numpy as np
from PIL import Image
from scipy.ndimage import distance_transform_edt

PACK = os.path.dirname(os.path.abspath(__file__))
ROOT = PACK                              # 输出即资产包自身，不经仓库外的暂存目录
SRC = f'{PACK}/logo/logo-master.png'     # 唯一输入：1024 母版
FG = np.array([252.0, 205.0, 1.0])       # #FCCD01 图形主色
PLATE_LO, PLATE_HI = 38.0, 74.0          # 底板亮度带 #262626 -> #4a4a48
PLATE_MID_HEX = '#383838'                # 亮度带中点，供只能取纯色的层使用
SAT_LO, SAT_HI = 45.0, 150.0             # 分离阈值：底板饱和度 <= 59，图形 >= 159
GLYPH_MIN_LUMA = 100.0                   # 低于此亮度的一定是底板（图形最暗约 127）
LUMA = np.array([0.299, 0.587, 0.114])

# ---------------------------------------------------------------- 工具函数
def defringe(im):
    """
    把非实心像素的 RGB 换成最近的实心像素颜色，alpha 通道不变。
    原图圆角边缘的 RGB 被透明区黑 RGB 拉暗（11.5 vs 应有 34），这是黑线的根源。
    """
    a = np.array(im.convert('RGBA')).astype(np.float32)
    rgb, al = a[..., :3], a[..., 3]
    # 只有 alpha 接近不透明的像素颜色可信；其余（含抗锯齿边缘）的原图颜色
    # 被透明区黑 RGB 污染过，一律用最近的实心像素颜色覆盖
    solid = al >= 250
    if solid.all() or not solid.any():
        return im.convert('RGBA')
    idx = distance_transform_edt(~solid, return_distances=False, return_indices=True)
    filled = rgb[idx[0], idx[1]]
    out_rgb = np.where(solid[..., None], rgb, filled)
    return Image.fromarray(
        np.concatenate([np.clip(out_rgb, 0, 255), al[..., None]], axis=2).astype(np.uint8), 'RGBA')


def purify(im):
    """
    母版就是成品色：渐变底板 + 带明暗的图形都按原样保留，这里只清理边缘。
    （早先的版本会把每个实心像素投影到「旧底板色 -> 主色」这条两色连线上做重建，
    那会把渐变底压成平涂、把图形的明暗一起抹掉。）
    """
    return defringe(im.convert('RGBA'))


def resize(im, size):
    """
    defringe -> LANCZOS -> 地板回抬 -> defringe
    先把透明区的黑 RGB 换成邻近实色，插值时就没有黑色可渗入；缩放后再修一次新产生的透明像素。
    （不用预乘反解：低 alpha 处除以小数会把 LANCZOS 振铃放大成白边，比黑边更明显）
    """
    im = defringe(im.convert('RGBA'))
    if im.size == (size, size):
        return im
    out = im.resize((size, size), Image.LANCZOS)
    a = np.array(out).astype(np.float32)
    a[a[..., 3] < 6, 3] = 0          # 抹掉无意义的极低 alpha 噪点
    a = lift_floor(a)                # 下采样振铃把方底压到地板之下，回抬
    return defringe(Image.fromarray(a.astype(np.uint8), 'RGBA'))


def lift_floor(a):
    """
    LANCZOS 核带负瓣，方底与图形的高对比边缘会下冲，把方底压到地板（#262626）之下。
    按亮度判定方底像素（图形最暗处约 127，远高于 GLYPH_MIN_LUMA），把它们的亮度抬回地板。
    回抬目标取地板 +1 级：转 uint8 是截断，正好抬到 38 会被截成 37。
    """
    lum = a[..., :3] @ LUMA
    fix = (a[..., 3] > 0) & (lum < GLYPH_MIN_LUMA) & (lum < PLATE_LO)
    if fix.any():
        k = np.where(fix, (PLATE_LO + 1.0) / np.maximum(lum, 1e-6), 1.0)
        a[..., :3] = np.clip(a[..., :3] * k[..., None], 0, 255)
    return a


def strip_background(im):
    """
    抠掉底板只留图形：按**饱和度**分离，不做颜色投影。

    图形是饱和的黄（饱和度 >= 159），底板是近中性（<= 59），中间只有一圈抗锯齿过渡带；
    在 [SAT_LO, SAT_HI] 上做软斜坡，过渡带拿到部分 alpha，边缘才不带锯齿。
    图形像素一律不透明 —— 若按「投影到两色连线」算 alpha，带明暗的图形会被翻译成半透明，
    色相偏橙的暗部甚至会掉到描边阈值之下而整块丢失。
    """
    a = np.array(im.convert('RGBA')).astype(np.float32)
    sat = a[..., :3].max(axis=2) - a[..., :3].min(axis=2)
    t = np.clip((sat - SAT_LO) / (SAT_HI - SAT_LO), 0.0, 1.0)
    out = a.copy()
    out[..., 3] = t * (a[..., 3] / 255.0) * 255.0
    out[out[..., 3] < 8, 3] = 0          # 清掉压缩噪声留下的孤立亮点
    return Image.fromarray(np.clip(out, 0, 255).astype(np.uint8), 'RGBA')


def save_png(im, path):
    im.save(path, 'PNG', optimize=True)


def bmp_layer(im, size):
    """
    未压缩的 32bpp 位图层（ICO 里的 BMP 变体）：BITMAPINFOHEADER + 自底向上的 BGRA 行 + 全零 AND mask。

    小尺寸层不用 PNG 压缩：并非所有「按尺寸取层」的调用方都认 PNG 压缩层，
    未压缩层在任何调用路径上都能取到有效位图（256 层仍用 PNG，那是最通用的一档）。
    """
    a = np.array(im.convert('RGBA'))
    bgra = np.ascontiguousarray(a[..., [2, 1, 0, 3]][::-1])          # 自底向上，行内 BGRA
    header = struct.pack('<IiiHHIIiiII', 40, size, size * 2, 1, 32, 0, size * size * 4, 0, 0, 0, 0)
    mask_stride = ((size + 31) // 32) * 4                            # AND mask 每行按 4 字节对齐
    return header + bgra.tobytes() + bytes(mask_stride * size)


def make_ico(img, path, sizes=(16, 24, 32, 48, 64, 128, 256)):
    layers = []
    for s in sizes:
        layer = resize(img, s)
        if s >= 256:
            buf = io.BytesIO()
            layer.save(buf, 'PNG')
            data = buf.getvalue()
        else:
            data = bmp_layer(layer, s)
        layers.append((s, data))
    hdr = struct.pack('<HHH', 0, 1, len(layers))
    offset = 6 + 16 * len(layers)
    entries, datas = b'', b''
    for s, d in layers:
        entries += struct.pack('<BBBBHHII', s % 256, s % 256, 0, 0, 1, 32, len(d), offset)
        offset += len(d)
        datas += d
    open(path, 'wb').write(hdr + entries + datas)


def make_icns(img, path):
    layers = [('icp4', 16), ('icp5', 32), ('icp6', 64), ('ic07', 128), ('ic08', 256),
              ('ic09', 512), ('ic10', 1024), ('ic11', 32), ('ic12', 64), ('ic13', 256), ('ic14', 1024)]
    body = b''
    for t, s in layers:
        buf = io.BytesIO()
        resize(img, s).save(buf, 'PNG')
        d = buf.getvalue()
        body += t.encode('ascii') + struct.pack('>I', len(d) + 8) + d
    open(path, 'wb').write(b'icns' + struct.pack('>I', 8 + len(body)) + body)


# ---------------------------------------------------------------- 主流程
raw = Image.open(SRC).convert('RGBA')
base = purify(raw)                        # 母版即真源，只清边缘，作为一切缩放的真源

# 透明版 logo（按饱和度抠背景 + defringe）
logo_tr = defringe(strip_background(base))

os.makedirs(f'{ROOT}/logo', exist_ok=True)
save_png(logo_tr, f'{ROOT}/logo/logo-transparent.png')   # 母版是输入，不在这里回写

# ---- 桌面 icons ----
OUT = f'{ROOT}/desktop'
os.makedirs(OUT, exist_ok=True)
for s in (32, 64, 128, 256, 512):
    save_png(resize(base, s), f'{OUT}/{s}x{s}.png')
save_png(resize(base, 256), f'{OUT}/128x128@2x.png')
save_png(base, f'{OUT}/icon.png')
make_ico(base, f'{OUT}/icon.ico')
make_icns(base, f'{OUT}/icon.icns')
for s in (30, 44, 71, 89, 107, 142, 150, 284, 310):
    save_png(resize(base, s), f'{OUT}/Square{s}x{s}Logo.png')
save_png(resize(base, 50), f'{OUT}/StoreLogo.png')

# ---- 托盘 ----
TRAY = f'{ROOT}/tray'
os.makedirs(TRAY, exist_ok=True)
save_png(resize(base, 64), f'{TRAY}/tray-64-color.png')
save_png(resize(logo_tr, 64), f'{TRAY}/tray-64-transparent.png')

def recolor(im, rgb):
    a = np.array(im.convert('RGBA'))
    a[..., 0], a[..., 1], a[..., 2] = rgb
    return Image.fromarray(a, 'RGBA')

save_png(defringe(recolor(resize(logo_tr, 64), (255, 255, 255))), f'{TRAY}/tray-64-white.png')
save_png(defringe(recolor(resize(logo_tr, 64), (0, 0, 0))), f'{TRAY}/tray-64-black.png')
save_png(defringe(recolor(resize(logo_tr, 44), (0, 0, 0))), f'{TRAY}/tray-mac-template@2x.png')

# ---- Android ----
AND = f'{ROOT}/android/res'
DENS = {'mdpi': 48, 'hdpi': 72, 'xhdpi': 96, 'xxhdpi': 144, 'xxxhdpi': 192}
for name, s in DENS.items():
    d = f'{AND}/mipmap-{name}'
    os.makedirs(d, exist_ok=True)
    save_png(resize(base, s), f'{d}/ic_launcher.png')
    save_png(resize(base, s), f'{d}/ic_launcher_round.png')
    fg = s * 108 // 48
    safe = int(fg * 66 / 108)
    canvas = Image.new('RGBA', (fg, fg), (0, 0, 0, 0))
    g = resize(logo_tr, safe)
    canvas.paste(g, ((fg - safe) // 2, (fg - safe) // 2), g)
    save_png(defringe(canvas), f'{d}/ic_launcher_foreground.png')
os.makedirs(f'{AND}/values', exist_ok=True)
open(f'{AND}/values/ic_launcher_background.xml', 'w').write(
    '<?xml version="1.0" encoding="utf-8"?>\n<resources>\n'
    f'    <color name="ic_launcher_background">{PLATE_MID_HEX}</color>\n</resources>\n')
os.makedirs(f'{AND}/mipmap-anydpi-v26', exist_ok=True)
adaptive = ('<?xml version="1.0" encoding="utf-8"?>\n'
            '<adaptive-icon xmlns:android="http://schemas.android.com/apk/res/android">\n'
            '    <background android:drawable="@color/ic_launcher_background"/>\n'
            '    <foreground android:drawable="@mipmap/ic_launcher_foreground"/>\n'
            '    <monochrome android:drawable="@mipmap/ic_launcher_foreground"/>\n</adaptive-icon>\n')
open(f'{AND}/mipmap-anydpi-v26/ic_launcher.xml', 'w').write(adaptive)
open(f'{AND}/mipmap-anydpi-v26/ic_launcher_round.xml', 'w').write(adaptive)

# ---- iOS（满幅不透明，圆角遮罩由平台自加）----
def opaque(size):
    """
    iOS 要求图标满幅不透明。母版四角是透明的，因此把四角按邻近底板色外推后整幅置为不透明。
    早先的做法是垫一层纯色（#333333），那恰好等于旧底板色所以衔接无痕；
    渐变底板的四个角亮度各不相同，垫任何单色都会在角上露出与底板不同色的缺口。
    （透明区的 RGB 已由 defringe 填成最近实心像素的颜色，置 alpha=255 即得外推结果。）
    """
    a = np.array(defringe(resize(base, size))).astype(np.float32)
    a[..., 3] = 255
    return Image.fromarray(np.clip(a, 0, 255).astype(np.uint8), 'RGBA').convert('RGB')

IOS = f'{ROOT}/ios'
os.makedirs(IOS, exist_ok=True)
ios_spec = [('20', ['2', '3'], 'iphone'), ('20', ['1', '2'], 'ipad'),
            ('29', ['1', '2', '3'], 'iphone'), ('29', ['1', '2'], 'ipad'),
            ('40', ['2', '3'], 'iphone'), ('40', ['1', '2'], 'ipad'),
            ('60', ['2', '3'], 'iphone'), ('76', ['1', '2'], 'ipad'),
            ('83.5', ['2'], 'ipad')]
items = []
for size, scales, idiom in ios_spec:
    for sc in scales:
        fn = f'AppIcon-{size}x{size}@{sc}x.png'
        px = int(float(size) * int(sc[0]))
        opaque(px).save(f'{IOS}/{fn}')
        items.append({"filename": fn, "idiom": idiom, "scale": f'{sc}x', "size": f'{size}x{size}'})
opaque(1024).save(f'{IOS}/AppIcon-512x512@2x.png')
items.append({"filename": "AppIcon-512x512@2x.png", "idiom": "ios-marketing",
              "scale": "1x", "size": "1024x1024"})
import json
open(f'{IOS}/Contents.json', 'w').write(json.dumps({"images": items, "info": {"author": "xcode", "version": 1}}, indent=2) + '\n')

# ---- 对照图 ----
def checkerboard(w, h, cell=8):
    y, x = np.mgrid[0:h, 0:w]
    t = (((x // cell) + (y // cell)) % 2).astype(np.float32)
    v = (0.94 + 0.06 * t) * 255.0
    a = np.repeat(v[..., None], 4, axis=2)
    return Image.fromarray(a.astype(np.uint8), 'RGBA')

DESK = [(512, '512x512.png'), (256, '256x256.png'), (128, '128x128.png'),
        (64, '64x64.png'), (32, '32x32.png')]
TRAY_STRIP = ['tray-64-color.png', 'tray-64-transparent.png', 'tray-64-white.png',
              'tray-64-black.png', 'tray-mac-template@2x.png']
PAD, GAP = 32, 24
top_w = sum(s for s, _ in DESK) + GAP * (len(DESK) - 1)
top_h = max(s for s, _ in DESK)
bot_w = 64 * len(TRAY_STRIP) + GAP * (len(TRAY_STRIP) - 1)
W, H = max(top_w, bot_w) + 2 * PAD, PAD + top_h + GAP + 64 + PAD
sheet = Image.new('RGBA', (W, H), (255, 255, 255, 255))
x = PAD
for s, fn in DESK:
    ico = resize(base, s)
    sheet.alpha_composite(ico, (x, PAD + (top_h - s) // 2))
    x += s + GAP
y = PAD + top_h + GAP
strip = checkerboard(bot_w, 64, 8)
sheet.alpha_composite(strip, (PAD, y))
x = PAD
for fn in TRAY_STRIP:
    sheet.alpha_composite(Image.open(f'{TRAY}/{fn}').convert('RGBA'), (x, y))
    x += 64 + GAP
save_png(sheet.convert('RGB'), f'{ROOT}/preview.png')

print('regenerated all partitions from', SRC)

