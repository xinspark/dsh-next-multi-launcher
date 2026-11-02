"""推广封面生成（竖版 3:4 + 横版 4:3）。

用法：python tools/promo/make-cover.py
输出：target/promo/cover-vertical.png（1080x1440）、cover-horizontal.png（1080x810）
素材：target/promo/assets/char.png（角色立绘；纯白背景会被自动抠掉）

注意：标题文字在 title_lines / lines2 里改；字号需与行宽匹配，横版更窄。
"""
import sys
import os
from PIL import Image, ImageDraw, ImageFont, ImageFilter

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

OUT = r"E:\FXS\Data\DSH000\DSH-NEXT Multi-Launcher\target\promo"
CHAR = os.path.join(OUT, "assets", "char.png")
FONT_BOLD = r"C:\Windows\Fonts\msyhbd.ttc"
FONT_REG = r"C:\Windows\Fonts\msyh.ttc"

def font(path, size):
    return ImageFont.truetype(path, size)

def cutout_white(img, threshold=238):
    """白底转透明（卡通图纯白背景）。"""
    img = img.convert("RGBA")
    px = img.load()
    w, h = img.size
    for y in range(h):
        for x in range(w):
            r, g, b, a = px[x, y]
            if r >= threshold and g >= threshold and b >= threshold:
                px[x, y] = (r, g, b, 0)
    return img

def rounded(draw, box, radius, fill):
    draw.rounded_rectangle(box, radius=radius, fill=fill)

char = cutout_white(Image.open(CHAR))
char_w, char_h = char.size

# ---------------- 竖版 1080x1440 ----------------
W, H = 1080, 1440
img = Image.new("RGB", (W, H), (248, 250, 253))
draw = ImageDraw.Draw(img)
# 背景渐变（从白到淡蓝）
for y in range(H):
    t = y / H
    r = int(248 + (232 - 248) * t)
    g = int(250 + (241 - 250) * t)
    b = int(253 + (255 - 253) * t)
    draw.line([(0, y), (W, y)], fill=(r, g, b))
# 顶部强调条
draw.rectangle([0, 0, W, 10], fill=(37, 99, 235))

# 标题（红色，三行）
f_title = font(FONT_BOLD, 82)
title_lines = ["让你在DSH里", "养的大肥鱼，", "有独属于自己的家"]
y = 110
for line in title_lines:
    bbox = draw.textbbox((0, 0), line, font=f_title)
    tw = bbox[2] - bbox[0]
    draw.text(((W - tw) // 2, y), line, font=f_title, fill=(220, 60, 60))
    y += (bbox[3] - bbox[1]) + 26

# 角色（放大）
scale = 2.1
cw, ch = int(char_w * scale), int(char_h * scale)
char_big = char.resize((cw, ch), Image.LANCZOS)
img.paste(char_big, ((W - cw) // 2, y - 10), char_big)

# 底部标签
f_sub = font(FONT_REG, 36)
sub = "DSH-NEXT 多开启动器 · MIT 开源 · Windows"
bbox = draw.textbbox((0, 0), sub, font=f_sub)
draw.text(((W - (bbox[2] - bbox[0])) // 2, H - 90), sub, font=f_sub, fill=(120, 132, 150))
img.save(os.path.join(OUT, "cover-vertical.png"))
print("saved cover-vertical.png")

# ---------------- 横版 1080x810 (4:3) ----------------
W2, H2 = 1080, 810
img2 = Image.new("RGB", (W2, H2), (248, 250, 253))
d2 = ImageDraw.Draw(img2)
for x in range(W2):
    t = x / W2
    r = int(248 + (232 - 248) * t)
    g = int(250 + (241 - 250) * t)
    b = int(253 + (255 - 253) * t)
    d2.line([(x, 0), (x, H2)], fill=(r, g, b))
d2.rectangle([0, 0, W2, 10], fill=(37, 99, 235))

# 左侧文字
f_t2 = font(FONT_BOLD, 54)
f_s2 = font(FONT_REG, 34)
lines2 = ["让你在 DSH 里", "养的大肥鱼", "有独属于自己的家"]
y2 = 130
for line in lines2:
    d2.text((90, y2), line, font=f_t2, fill=(220, 60, 60))
    y2 += 96
d2.text((90, y2 + 20), "多开启动器 · MIT 开源", font=f_s2, fill=(110, 122, 140))

# 右侧角色
scale2 = 1.3
cw2, ch2 = int(char_w * scale2), int(char_h * scale2)
char2 = char.resize((cw2, ch2), Image.LANCZOS)
img2.paste(char2, (W2 - cw2 - 40, (H2 - ch2) // 2), char2)
img2.save(os.path.join(OUT, "cover-horizontal.png"))
print("saved cover-horizontal.png")



