"""推广旁白生成（千问 qwen-audio-3.0-tts-plus）。

凭据：从 DSH 凭据文件读取 QWEN_API_API_KEY（本文件不保存任何密钥）。
用法：python tools/promo/tts.py
输出：target/promo/voice-v2.mp3

注意：
- 端点用千问 AI 平台的 WebSocket（不是阿里云官方 DashScope）：
  wss://cn-beijing.maas.qianwenaiapi.com/api-ws/v1/inference
- 声音 longanlingxin；文案在 SCRIPT 里改，改完要重新对齐视频时长。
"""
import sys
import re
import os

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

cred = r"C:\Users\FXS\.dsh\.credentials.yaml"
txt = open(cred, encoding="utf-8", errors="replace").read()
keys = dict(re.findall(r"([A-Z0-9_]+_API_KEY):\s*(\S+)", txt))
api_key = keys.get("QWEN_API_API_KEY")
if not api_key:
    print("KEY_MISSING")
    sys.exit(1)
print("key loaded, len =", len(api_key))

import dashscope
from dashscope.audio.tts_v2 import SpeechSynthesizer

dashscope.api_key = api_key
dashscope.base_websocket_api_url = "wss://cn-beijing.maas.qianwenaiapi.com/api-ws/v1/inference"

MODEL = "qwen-audio-3.0-tts-plus"
VOICE = "longanlingxin"

SCRIPT = (
    "你在 DSH 里养的大肥鱼，记忆正在被工作会话污染。"
    "官方的 profile 切换，只隔离插件。"
    "这个启动器，给每个实例一个完整独立的数据目录——"
    "会话、插件、凭据，各一份。"
    "双击桌面图标，仍然进日常环境。"
    "开源，免费，在我的 GitHub。"
)
print("script chars:", len(SCRIPT))

out_dir = r"E:\FXS\Data\DSH000\DSH-NEXT Multi-Launcher\target\promo"
os.makedirs(out_dir, exist_ok=True)

try:
    synthesizer = SpeechSynthesizer(model=MODEL, voice=VOICE)
    audio = synthesizer.call(SCRIPT)
    if not audio:
        print("EMPTY_AUDIO")
        sys.exit(2)
    path = os.path.join(out_dir, "voice-v2.mp3")
    with open(path, "wb") as f:
        f.write(audio)
    print("saved:", path, os.path.getsize(path), "bytes")
except Exception as e:
    print("TTS_FAILED:", type(e).__name__, str(e)[:400])
    sys.exit(3)

