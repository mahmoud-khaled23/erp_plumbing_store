"""Render actual React print fixtures through the shared WebKitGTK PDF adapter.

Checks page counts, physical sizes, nonempty content, and CODE128 at 203 dpi.
This is development tooling, never part of the installed application.
"""
from pathlib import Path
import re
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[1]
PDF = ROOT / "output/pdf"
NAMES = ("invoice-a4", "receipt-80mm", "goods-received", "report-a4", "barcode-labels")


def run(*args: str) -> str:
    return subprocess.check_output(args, text=True, timeout=110)


def info(path: Path) -> tuple[int, float, float]:
    text = run("pdfinfo", str(path))
    pages = re.search(r"^Pages:\s+(\d+)", text, re.M)
    size = re.search(r"^Page size:\s+([\d.]+) x ([\d.]+) pts", text, re.M)
    assert pages and size, f"Invalid PDF metadata: {path.name}"
    return int(pages[1]), float(size[1]), float(size[2])


def render() -> None:
    for name in NAMES:
        fixture = ROOT / f"output/print-fixtures/{name}.html"
        size = re.search(r"@page\s*\{\s*size:\s*(\d+)mm\s+(\d+)mm", fixture.read_text(encoding="utf-8"))
        assert size, f"Missing physical page size: {name}"
        native = PDF / f"native-{name}.pdf"
        run(str(ROOT / "output/target/debug/examples/native_print_check"),
            str(fixture), str(native), size[1], size[2])
        expected = info(PDF / f"{name}.pdf")
        actual = info(native)
        assert actual[0] == expected[0], f"Extra/missing native pages: {name}: {actual} vs {expected}"
        assert all(abs(a - b) < 1 for a, b in zip(actual[1:], expected[1:])), name
        for i in range(1, actual[0] + 1):
            text = run("pdftotext", "-f", str(i), "-l", str(i), str(native), "-")
            assert len(text.strip()) > 20, f"Blank native page: {name}/{i}"
        print(f"{name}: {actual[0]} pages, {actual[1]} x {actual[2]} pt; native PDF verified", flush=True)
    for prefix in ("", "native-"):
        with tempfile.TemporaryDirectory(prefix="erp-barcodes-") as folder:
            base = str(Path(folder) / "labels")
            run("pdftoppm", "-r", "203", "-png", str(PDF / f"{prefix}barcode-labels.pdf"), base)
            decoded = set()
            for png in Path(folder).glob("labels-*.png"):
                decoded.update(run("zbarimg", "--quiet", "--raw", "--nodbus", str(png)).splitlines())
            assert decoded == {"6220000000015", "6220000000022"}, decoded
            print(f"{prefix or 'chromium-'}barcodes: both unit codes decoded at 203 dpi", flush=True)


if __name__ == "__main__":
    render()
