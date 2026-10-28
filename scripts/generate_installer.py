"""Keep installer strings in the same Arabic dictionary as the application."""
import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = ROOT / "src-tauri/windows/Arabic.nsh"


def content() -> str:
    labels = json.loads((ROOT / "src/i18n/ar.json").read_text(encoding="utf-8"))["installer"]
    lines = ["; Generated from src/i18n/ar.json by scripts/generate_installer.py."]
    for key, value in labels.items():
        if '"' in value or "\n" in value or "\r" in value:
            raise ValueError(f"Unsupported NSIS string characters: {key}")
        lines.append(f'LangString {key} ${{LANG_ARABIC}} "{value}"')
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    expected = content()
    if args.check:
        if not DESTINATION.exists() or DESTINATION.read_text(encoding="utf-8") != expected:
            raise SystemExit("Installer translations changed: run python3 scripts/generate_installer.py")
        print("Arabic installer translations match ar.json.")
    else:
        DESTINATION.parent.mkdir(parents=True, exist_ok=True)
        DESTINATION.write_text(expected, encoding="utf-8", newline="\n")
