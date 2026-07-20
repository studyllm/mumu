"""生成沐目托盘图标（design.md § 八）

- tray-eye-open.png  ：薄荷绿椭圆眼睛 + 中央瞳孔（工作状态）
- tray-eye-closed.png：薄荷绿弧线一条（休息状态）
- 尺寸 32×32 RGBA
- 主色 #87A878
"""
from PIL import Image, ImageDraw

SIZE = 32
GREEN = (135, 168, 120, 255)
GREEN_DARK = (110, 145, 100, 255)


def make_open(path: str) -> None:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # 椭圆眼睑（白填充 + 薄荷绿描边）
    eye_box = (4, 10, 28, 22)
    d.ellipse(eye_box, fill=(255, 255, 255, 255), outline=GREEN, width=2)

    # 中央瞳孔（薄荷绿实心圆）
    pupil_box = (13, 12, 19, 18)
    d.ellipse(pupil_box, fill=GREEN_DARK)

    img.save(path, "PNG")


def make_closed(path: str) -> None:
    img = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)

    # 弧线：模拟闭眼（眼皮下垂的弧）
    # 用 PIL arc：从 (4,16) 到 (28,16)，包住下半圆
    arc_box = (4, 12, 28, 24)
    d.arc(arc_box, start=200, end=340, fill=GREEN, width=3)

    img.save(path, "PNG")


if __name__ == "__main__":
    import os
    out_dir = r"D:\wxw-workspace\project\study\EyeProtectionTool\mumu\src-tauri\icons"
    make_open(os.path.join(out_dir, "tray-eye-open.png"))
    make_closed(os.path.join(out_dir, "tray-eye-closed.png"))
    print("done")