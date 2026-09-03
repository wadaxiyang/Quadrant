"""Generate platform icon assets from Quadrant's canonical AppMark geometry."""

from __future__ import annotations

from pathlib import Path

from PIL import Image, ImageDraw


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "assets" / "branding"
SIZES = (16, 20, 24, 32, 40, 48, 64, 128, 256, 512)
SUPERSAMPLE = 4
ACCENT = (0, 95, 184, 255)
WHITE = (255, 255, 255, 255)
WHITE_MUTED = (255, 255, 255, 199)


def scaled(value: float, size: int) -> int:
    return round(value * size / 48 * SUPERSAMPLE)


def render(size: int) -> Image.Image:
    canvas_size = size * SUPERSAMPLE
    image = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(image)
    draw.rounded_rectangle(
        (0, 0, canvas_size - 1, canvas_size - 1),
        radius=scaled(10, size),
        fill=ACCENT,
    )
    for x, y, color in (
        (9, 9, WHITE),
        (26, 9, WHITE_MUTED),
        (9, 26, WHITE_MUTED),
        (26, 26, WHITE),
    ):
        left = scaled(x, size)
        top = scaled(y, size)
        right = scaled(x + 13, size) - 1
        bottom = scaled(y + 13, size) - 1
        draw.rounded_rectangle(
            (left, top, right, bottom),
            radius=scaled(3, size),
            fill=color,
        )
    return image.resize((size, size), Image.Resampling.LANCZOS)


def main() -> None:
    OUTPUT.mkdir(parents=True, exist_ok=True)
    images = {size: render(size) for size in SIZES}
    for size, image in images.items():
        image.save(OUTPUT / f"quadrant-{size}.png", optimize=True)
    (OUTPUT / "quadrant-32.rgba").write_bytes(images[32].tobytes())
    images[256].save(
        OUTPUT / "quadrant.ico",
        format="ICO",
        sizes=[(size, size) for size in SIZES if size <= 256],
    )
    render(1024).save(
        OUTPUT / "Quadrant.icns",
        format="ICNS",
        sizes=[(size, size) for size in (16, 32, 64, 128, 256, 512, 1024)],
    )


if __name__ == "__main__":
    main()
