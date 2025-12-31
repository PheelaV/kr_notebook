# /// script
# requires-python = ">=3.12"
# dependencies = ["pillow"]
# ///
"""
Favicon Generator Script

Generates a complete favicon bundle from the app's logo design.
Creates all necessary formats and sizes for desktop and mobile browsers.

Usage:
    uv run scripts/generate_favicon.py

Output files (in static/):
    - favicon.ico (16x16, 32x32, 48x48 multi-resolution)
    - favicon-16x16.png
    - favicon-32x32.png
    - apple-touch-icon.png (180x180)
    - android-chrome-192x192.png
    - android-chrome-512x512.png
    - favicon.svg (scalable version)
    - site.webmanifest
"""

import json
from io import BytesIO
from pathlib import Path

from PIL import Image, ImageDraw

# Project paths
SCRIPT_DIR = Path(__file__).parent
PROJECT_ROOT = SCRIPT_DIR.parent
STATIC_DIR = PROJECT_ROOT / "static"

# Colors (matching Tailwind theme)
BG_COLOR = (238, 242, 255)  # #EEF2FF - indigo-50
STROKE_COLOR = (79, 70, 229)  # #4F46E5 - indigo-600

# Favicon sizes following current best practices
FAVICON_SIZES = {
    "favicon-16x16.png": 16,
    "favicon-32x32.png": 32,
    "apple-touch-icon.png": 180,
    "android-chrome-192x192.png": 192,
    "android-chrome-512x512.png": 512,
}

# ICO file contains multiple resolutions
ICO_SIZES = [16, 32, 48]


def draw_logo(size: int) -> Image.Image:
    """
    Draw the stylized hieut logo at the specified size.

    The logo consists of:
    - A circle at the top (representing the ㅇ part of ㅎ)
    - Two horizontal lines below (the horizontal strokes)

    Drawn on a rounded rectangle background for visibility.
    """
    # Create image with slight oversample for better antialiasing
    scale = 4 if size < 64 else 2
    canvas_size = size * scale

    img = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Calculate proportions (based on 64x64 reference design)
    padding = canvas_size * 0.125  # 8/64 = 0.125
    corner_radius = canvas_size * 0.1875  # 12/64 = 0.1875

    # Draw rounded rectangle background
    draw.rounded_rectangle(
        [0, 0, canvas_size - 1, canvas_size - 1],
        radius=int(corner_radius),
        fill=BG_COLOR,
    )

    # Calculate stroke width (scales with size)
    stroke_width = max(1, int(canvas_size * 0.055))  # ~3.5/64

    # Circle parameters (top element)
    circle_center_x = canvas_size / 2
    circle_center_y = canvas_size * 0.3125  # 20/64
    circle_radius = canvas_size * 0.125  # 8/64

    # Draw circle outline
    draw.ellipse(
        [
            circle_center_x - circle_radius,
            circle_center_y - circle_radius,
            circle_center_x + circle_radius,
            circle_center_y + circle_radius,
        ],
        outline=STROKE_COLOR,
        width=stroke_width,
    )

    # Top horizontal line
    line1_y = canvas_size * 0.53125  # 34/64
    line1_x1 = canvas_size * 0.25  # 16/64
    line1_x2 = canvas_size * 0.75  # 48/64

    draw.line(
        [(line1_x1, line1_y), (line1_x2, line1_y)],
        fill=STROKE_COLOR,
        width=stroke_width,
    )

    # Draw rounded caps for line 1
    cap_radius = stroke_width / 2
    draw.ellipse(
        [
            line1_x1 - cap_radius,
            line1_y - cap_radius,
            line1_x1 + cap_radius,
            line1_y + cap_radius,
        ],
        fill=STROKE_COLOR,
    )
    draw.ellipse(
        [
            line1_x2 - cap_radius,
            line1_y - cap_radius,
            line1_x2 + cap_radius,
            line1_y + cap_radius,
        ],
        fill=STROKE_COLOR,
    )

    # Bottom horizontal line (shorter)
    line2_y = canvas_size * 0.71875  # 46/64
    line2_x1 = canvas_size * 0.3125  # 20/64
    line2_x2 = canvas_size * 0.6875  # 44/64

    draw.line(
        [(line2_x1, line2_y), (line2_x2, line2_y)],
        fill=STROKE_COLOR,
        width=stroke_width,
    )

    # Draw rounded caps for line 2
    draw.ellipse(
        [
            line2_x1 - cap_radius,
            line2_y - cap_radius,
            line2_x1 + cap_radius,
            line2_y + cap_radius,
        ],
        fill=STROKE_COLOR,
    )
    draw.ellipse(
        [
            line2_x2 - cap_radius,
            line2_y - cap_radius,
            line2_x2 + cap_radius,
            line2_y + cap_radius,
        ],
        fill=STROKE_COLOR,
    )

    # Downscale with antialiasing
    if scale > 1:
        img = img.resize((size, size), Image.Resampling.LANCZOS)

    return img


def generate_png_favicons() -> dict[str, Path]:
    """Generate all PNG favicon files."""
    generated = {}

    for filename, size in FAVICON_SIZES.items():
        output_path = STATIC_DIR / filename
        img = draw_logo(size)
        img.save(output_path, "PNG", optimize=True)
        generated[filename] = output_path
        print(f"  Created {filename} ({size}x{size})")

    return generated


def generate_ico_favicon() -> Path:
    """Generate multi-resolution ICO file."""
    images = []

    for size in ICO_SIZES:
        img = draw_logo(size)
        images.append(img)

    output_path = STATIC_DIR / "favicon.ico"

    # Save as ICO with multiple sizes
    images[0].save(
        output_path,
        format="ICO",
        sizes=[(s, s) for s in ICO_SIZES],
        append_images=images[1:],
    )

    print(f"  Created favicon.ico ({', '.join(f'{s}x{s}' for s in ICO_SIZES)})")
    return output_path


def generate_favicon_svg() -> Path:
    """Generate SVG favicon for modern browsers."""
    svg = '''<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 64 64" fill="none">
  <rect width="64" height="64" rx="12" fill="#EEF2FF"/>
  <circle cx="32" cy="20" r="8" stroke="#4F46E5" stroke-width="3.5" fill="none"/>
  <path d="M16 34 L48 34" stroke="#4F46E5" stroke-width="3.5" stroke-linecap="round"/>
  <path d="M20 46 L44 46" stroke="#4F46E5" stroke-width="3.5" stroke-linecap="round"/>
</svg>'''

    output_path = STATIC_DIR / "favicon.svg"
    output_path.write_text(svg)
    print("  Created favicon.svg (scalable)")
    return output_path


def generate_webmanifest() -> Path:
    """Generate web app manifest for PWA support."""
    manifest = {
        "name": "Hangul Learn",
        "short_name": "Hangul",
        "description": "Learn Korean Hangul characters with spaced repetition",
        "start_url": "/",
        "display": "standalone",
        "background_color": "#EEF2FF",
        "theme_color": "#4F46E5",
        "icons": [
            {
                "src": "/static/android-chrome-192x192.png",
                "sizes": "192x192",
                "type": "image/png",
            },
            {
                "src": "/static/android-chrome-512x512.png",
                "sizes": "512x512",
                "type": "image/png",
            },
            {
                "src": "/static/android-chrome-512x512.png",
                "sizes": "512x512",
                "type": "image/png",
                "purpose": "maskable",
            },
        ],
    }

    output_path = STATIC_DIR / "site.webmanifest"
    output_path.write_text(json.dumps(manifest, indent=2))
    print("  Created site.webmanifest")
    return output_path


def main() -> None:
    print("Favicon Generator")
    print("=" * 40)

    # Verify static directory exists
    if not STATIC_DIR.exists():
        print(f"Error: Static directory not found: {STATIC_DIR}")
        return

    # Generate SVG favicon
    print("\nGenerating SVG favicon...")
    generate_favicon_svg()

    # Generate PNG favicons
    print("\nGenerating PNG favicons...")
    generate_png_favicons()

    # Generate ICO file
    print("\nGenerating ICO favicon...")
    generate_ico_favicon()

    # Generate web manifest
    print("\nGenerating web manifest...")
    generate_webmanifest()

    print("\n" + "=" * 40)
    print("Favicon bundle generated successfully!")
    print("\nFiles created in static/:")
    print("  - favicon.ico (multi-resolution)")
    print("  - favicon.svg (scalable)")
    print("  - favicon-16x16.png")
    print("  - favicon-32x32.png")
    print("  - apple-touch-icon.png (180x180)")
    print("  - android-chrome-192x192.png")
    print("  - android-chrome-512x512.png")
    print("  - site.webmanifest")


if __name__ == "__main__":
    main()
