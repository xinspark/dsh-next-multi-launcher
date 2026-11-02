"""推广视频合成（截图/录屏片段 → 字幕 + 卡片 + 转场）。

依赖：Python 的 opencv-python、Pillow、imageio-ffmpeg。
用法：python tools/promo/make-video.py
输出：target/promo/promo-v5.mp4（21 秒，1920x1080）

注意：
- SRC 是原始录屏路径，W/H/FPS_OUT 是输出规格，按环境修改；
- 各段时长/文案在文件末尾的 timeline 段落里调整；
- 中文字体用 C:\\Windows\\Fonts\\msyh.ttc（Windows 自带）。
"""
import cv2
import numpy as np
import os
import json
from PIL import Image, ImageDraw, ImageFont

SRC = r"D:\FXS\Home\Videos\PixPin\PixPin_2026-11-02_16-52-49.mp4"
OUT_DIR = r"E:\FXS\Data\DSH000\DSH-NEXT Multi-Launcher\target\promo"
os.makedirs(OUT_DIR, exist_ok=True)

W, H = 1920, 1080
FPS_OUT = 30
FONT_PATH = r"C:\Windows\Fonts\msyh.ttc"

def font(size):
    return ImageFont.truetype(FONT_PATH, size)

F_BIG = font(64)
F_MID = font(44)
F_SMALL = font(34)
F_TAG = font(30)

def crop_16_9(frame):
    h, w = frame.shape[:2]
    target = w * 9 / 16
    if target <= h:
        y0 = int((h - target) / 2)
        frame = frame[y0:y0 + int(target), :]
    else:
        x0 = int((w - h * 16 / 9) / 2)
        frame = frame[:, x0:x0 + int(h * 16 / 9)]
    return cv2.resize(frame, (W, H), interpolation=cv2.INTER_AREA)

def to_pil(bgr):
    return Image.fromarray(cv2.cvtColor(bgr, cv2.COLOR_BGR2RGB))

def from_pil(img):
    return cv2.cvtColor(np.array(img), cv2.COLOR_RGB2BGR)

def subtitle(frame_bgr, text, font_obj=F_MID, bottom=76):
    img = to_pil(frame_bgr)
    draw = ImageDraw.Draw(img)
    bbox = draw.textbbox((0, 0), text, font=font_obj)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    bar_w, bar_h = tw + 68, th + 36
    x0 = (W - bar_w) // 2
    y0 = H - bottom - bar_h
    overlay = Image.new("RGBA", img.size, (0, 0, 0, 0))
    od = ImageDraw.Draw(overlay)
    od.rounded_rectangle([x0, y0, x0 + bar_w, y0 + bar_h], radius=12, fill=(8, 10, 14, 210))
    img = Image.alpha_composite(img.convert("RGBA"), overlay).convert("RGB")
    draw = ImageDraw.Draw(img)
    draw.text((x0 + 34, y0 + 18 - bbox[1]), text, font=font_obj, fill=(240, 244, 250))
    return from_pil(img)

def tag(frame_bgr, text, x, y, color=(37, 99, 235)):
    img = to_pil(frame_bgr)
    draw = ImageDraw.Draw(img, "RGBA")
    bbox = draw.textbbox((0, 0), text, font=F_TAG)
    tw, th = bbox[2] - bbox[0], bbox[3] - bbox[1]
    pad = 14
    draw.rounded_rectangle([x, y, x + tw + pad * 2, y + th + pad * 2], radius=10,
                           fill=(*color, 220))
    draw.text((x + pad, y + pad - bbox[1]), text, font=F_TAG, fill=(255, 255, 255))
    return from_pil(img)

def card(lines, sub=None):
    img = Image.new("RGB", (W, H), (9, 11, 15))
    draw = ImageDraw.Draw(img)
    draw.rectangle([0, 0, W, 4], fill=(37, 99, 235))
    y = H // 2 - len(lines) * 46
    for txt, f, color in lines:
        bbox = draw.textbbox((0, 0), txt, font=f)
        draw.text(((W - (bbox[2] - bbox[0])) // 2, y), txt, font=f, fill=color)
        y += (bbox[3] - bbox[1]) + 42
    if sub:
        bbox = draw.textbbox((0, 0), sub, font=F_SMALL)
        draw.text(((W - (bbox[2] - bbox[0])) // 2, H - 90), sub, font=F_SMALL, fill=(120, 130, 145))
    return from_pil(img)

def read_segment(t0, t1, fps_out=FPS_OUT):
    cap = cv2.VideoCapture(SRC)
    fps = cap.get(cv2.CAP_PROP_FPS)
    cap.set(cv2.CAP_PROP_POS_FRAMES, int(t0 * fps))
    frames, idx = [], 0
    while True:
        pos = cap.get(cv2.CAP_PROP_POS_FRAMES) / fps
        if pos > t1:
            break
        ok, f = cap.read()
        if not ok:
            break
        if idx % max(1, int(fps / fps_out)) == 0:
            frames.append(crop_16_9(f))
        idx += 1
    cap.release()
    return frames

def write_video(frames, path):
    vw = cv2.VideoWriter(path, cv2.VideoWriter_fourcc(*"mp4v"), FPS_OUT, (W, H))
    for f in frames:
        vw.write(f)
    vw.release()

WHITE = (240, 244, 250)
BLUE = (96, 165, 250)
GRAY = (150, 160, 175)
RED = (248, 113, 113)

timeline = []

# 1) 痛点卡片 3s
c = card(
    [("你在 DSH 里养的大肥鱼", F_BIG, WHITE),
     ("记忆正在被工作会话污染", F_BIG, RED)],
    sub="官方 profile 切换，只隔离插件"
)
timeline += [c] * int(2.5 * FPS_OUT)

# 2) 启动器 4s
seg = read_segment(0.0, 3.0)
seg = [subtitle(f, "每个实例，一个完整独立的数据目录") for f in seg]
timeline += seg

# 3) 双窗口 10s（源 34.5-44.5s，启动器不在前面）
seg = read_segment(35.5, 42.5)
for i, f in enumerate(seg):
    f = tag(f, "工作实例", 120, 60, (37, 99, 235))
    f = tag(f, "汐", 1040, 60, (34, 197, 94))
    seg[i] = f
seg = [subtitle(f, "两个实例，各自的数据目录") for f in seg]
timeline += seg

# 4) 对比卡片 4s
c = card(
    [("官方 profile：只隔离插件", F_MID, GRAY),
     ("本启动器：会话 / 存储 / 凭据全部隔离", F_MID, BLUE)],
    sub="独立 home = 独立 userData = 两把不同的锁"
)
timeline += [c] * (3 * FPS_OUT)

# 5) 启动器再入 3s
seg = read_segment(4.8, 7.3)
seg = [subtitle(f, "启动成功后，指针自动还原") for f in seg]
timeline += seg

# 6) 结尾 4s
c = card(
    [("DSH-NEXT 应用多开启动器", F_BIG, WHITE),
     ("github.com/xinspark/dsh-next-multi-launcher", F_SMALL, BLUE)],
    sub="MIT 开源 · Windows"
)
timeline += [c] * (3 * FPS_OUT)

out = os.path.join(OUT_DIR, "promo-v5.mp4")
write_video(timeline, out)
print(json.dumps({"out": out, "frames": len(timeline), "seconds": round(len(timeline) / FPS_OUT, 1)}))



