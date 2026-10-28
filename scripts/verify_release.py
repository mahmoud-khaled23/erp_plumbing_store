"""Validate a real NSIS artifact and collect reproducible release evidence."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import struct
import sys
import tomllib

ROOT = Path(__file__).resolve().parents[1]


def pe_info(path: Path) -> dict:
    data = path.read_bytes()
    if data[:2] != b"MZ" or len(data) < 64:
        raise ValueError(f"Not a Windows executable: {path}")
    pe = struct.unpack_from("<I", data, 60)[0]
    if data[pe:pe + 4] != b"PE\0\0":
        raise ValueError(f"Invalid PE signature: {path}")
    machine, count = struct.unpack_from("<HH", data, pe + 4)
    opt_size = struct.unpack_from("<H", data, pe + 20)[0]
    opt = pe + 24
    magic = struct.unpack_from("<H", data, opt)[0]
    if magic not in (0x10B, 0x20B):
        raise ValueError("Unsupported optional PE header")
    directories = opt + (112 if magic == 0x20B else 96)
    sections = opt + opt_size

    def offset(rva: int) -> int:
        headers = struct.unpack_from("<I", data, opt + 60)[0]
        if rva < headers:
            return rva
        for i in range(count):
            size, address, raw_size, raw = struct.unpack_from("<IIII", data, sections + i * 40 + 8)
            if address <= rva < address + max(size, raw_size):
                return raw + rva - address
        raise ValueError(f"Unmapped PE address: {rva}")

    imports = []
    import_rva = struct.unpack_from("<I", data, directories + 8)[0]
    if import_rva:
        pos = offset(import_rva)
        while any(data[pos:pos + 20]):
            name = offset(struct.unpack_from("<I", data, pos + 12)[0])
            imports.append(data[name:data.index(b"\0", name)].decode("ascii"))
            pos += 20
    return {"machine": hex(machine), "imports": sorted(imports), "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest()}


def check_configuration() -> dict:
    config = json.loads((ROOT / "src-tauri/tauri.conf.json").read_text(encoding="utf-8"))
    package = json.loads((ROOT / "package.json").read_text(encoding="utf-8"))
    rust = tomllib.loads((ROOT / "src-tauri/Cargo.toml").read_text(encoding="utf-8"))
    if len({config["version"], package["version"], rust["package"]["version"]}) != 1:
        raise ValueError("Package, Cargo and Tauri versions must match")
    windows = config["bundle"]["windows"]
    if windows["webviewInstallMode"]["type"] != "skip" or windows.get("minimumWebview2Version"):
        raise ValueError("Compact offline installer must never bootstrap or update WebView2")
    if windows["allowDowngrades"] or windows["nsis"]["languages"] != ["Arabic"]:
        raise ValueError("Arabic-only installer and downgrade protection are required")
    if config["bundle"]["targets"] != ["nsis"]:
        raise ValueError("The release must be NSIS")
    if not list((ROOT / "dist/assets").glob("cairo-arabic*.woff2")):
        raise ValueError("The offline Arabic font is missing")
    if list((ROOT / "dist").rglob("*.map")):
        raise ValueError("Do not ship source maps")
    return config


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--target-dir", type=Path, default=ROOT / "src-tauri/target")
    parser.add_argument("--origin", choices=("fedora-cross", "windows-native"), required=True)
    parser.add_argument("--output-dir", type=Path, default=ROOT / "output/releases")
    args = parser.parse_args()
    config = check_configuration()
    release = args.target_dir / "x86_64-pc-windows-msvc/release"
    binary = release / "plumbing-erp.exe"
    installers = list((release / "bundle/nsis").glob(f'*_{config["version"]}_x64-setup.exe'))
    if len(installers) != 1:
        raise ValueError(f"Expected one NSIS installer, found {len(installers)}")
    installer = installers[0]
    app_info, installer_info = pe_info(binary), pe_info(installer)
    if app_info["machine"] != "0x8664" or installer_info["machine"] != "0x14c":
        raise ValueError("Expected x64 application and x86 NSIS launcher")
    if installer_info["bytes"] >= 20_000_000:
        raise ValueError(f'Installer exceeds the 20 MB target: {installer_info["bytes"]} bytes')
    forbidden = ("vcruntime", "msvcp", "msvcr", "libgcc", "libstdc++", "libwinpthread")
    if any(name.lower().startswith(forbidden) for name in app_info["imports"]):
        raise ValueError("Application unexpectedly needs a separate C/C++ runtime DLL")
    destination = args.output_dir / config["version"] / args.origin
    destination.mkdir(parents=True, exist_ok=True)
    output = destination / installer.name
    shutil.copy2(installer, output)
    report = {"version": config["version"], "target": "x86_64-pc-windows-msvc",
              "build_origin": args.origin, "installer": {"file": output.name, **installer_info},
              "application": app_info, "size_limit_bytes": 20_000_000,
              "windows_execution": "See separate smoke report; compilation is not execution",
              "performance": "Unmeasured until measure-windows.ps1 runs on a reference PC"}
    (destination / "release-manifest.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8", newline="\n")
    (destination / "SHA256SUMS").write_text(f'{installer_info["sha256"]}  {output.name}\n', encoding="utf-8", newline="\n")
    print(json.dumps(report, indent=2))
    print(f"Installer: {output}")


if __name__ == "__main__":
    try:
        main()
    except (OSError, ValueError, KeyError, struct.error) as error:
        sys.exit(str(error))
