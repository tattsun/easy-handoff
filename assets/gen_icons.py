"""Generate the app icon (assets/icon.ico) and MSIX logos (packaging/msix/Assets).

Run from the repo root:
    mise exec uv python@3.13 -- uv run --no-project --with pillow python assets/gen_icons.py
"""

from pathlib import Path

from PIL import Image, ImageDraw

N = 1024
BLUE = (10, 132, 255, 255)
WHITE = (255, 255, 255, 255)


def badge() -> Image.Image:
    """Blue circle with two white arrows (⇄)."""
    img = Image.new("RGBA", (N, N), (0, 0, 0, 0))
    d = ImageDraw.Draw(img)
    m = 32
    d.ellipse([m, m, N - m, N - m], fill=BLUE)
    t = 64  # stroke width

    def arrow(y: int, right: bool) -> None:
        x0, x1 = 280, 744
        d.rounded_rectangle([x0, y - t // 2, x1, y + t // 2], radius=t // 2, fill=WHITE)
        tip = x1 + 40 if right else x0 - 40
        back = tip - 170 if right else tip + 170
        d.polygon([(tip, y), (back, y - 130), (back, y + 130)], fill=WHITE)

    arrow(400, True)
    arrow(624, False)
    return img


def fit(img: Image.Image, w: int, h: int, scale: float = 1.0) -> Image.Image:
    """Center `img` on a transparent w x h canvas, sized to `scale` of the shorter side."""
    side = round(min(w, h) * scale)
    canvas = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    canvas.paste(img.resize((side, side), Image.LANCZOS), ((w - side) // 2, (h - side) // 2))
    return canvas


def main() -> None:
    img = badge()
    img.save("assets/icon.ico", sizes=[(s, s) for s in (16, 20, 24, 32, 48, 64, 128, 256)])

    out = Path("packaging/msix/Assets")
    out.mkdir(parents=True, exist_ok=True)
    fit(img, 44, 44).save(out / "Square44x44Logo.png")
    fit(img, 50, 50).save(out / "StoreLogo.png")
    fit(img, 150, 150, 0.66).save(out / "Square150x150Logo.png")
    fit(img, 310, 150, 0.66).save(out / "Wide310x150Logo.png")


if __name__ == "__main__":
    main()
