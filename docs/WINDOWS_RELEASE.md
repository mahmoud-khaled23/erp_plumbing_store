# Stage 5: Windows installer, tests and performance evidence

Development can stay on Fedora. The deployment target is **Windows 10/11 x64**,
`x86_64-pc-windows-msvc`, packaged as a per-user Arabic NSIS `.exe`. The application
contains its SQLite engine, Cairo font, frontend and Rust services. Node.js, Python,
Rust, Docker and a database server are development tools, not deployment dependencies.

This stage prepares a release candidate. Compilation and a file-size check do not
establish Windows compatibility, printer support or the requested runtime performance.
The evidence files below distinguish these results. Do not use a store's only data copy
for release acceptance tests.

## Recorded candidate: 28 October 2026

The Fedora MSVC build produced and verified this actual artifact:

```text
output/releases/0.1.0/fedora-cross/PlumbingStore_0.1.0_x64-setup.exe
Size:   3,713,278 bytes (3.71 MB; 3.54 MiB)
SHA256: 76522bc5254de1361787f19601f537884f35a1c1a6ff7d9a62fce969c0468ba4
```

The installed application payload is 12,915,200 bytes. Both the Python PE reader and
LLVM confirmed an x64 Windows GUI executable importing only Windows system DLLs;
no separate VC++ runtime DLL is needed. `sha256sum --check SHA256SUMS` passed against
the copied installer. `release-manifest.json` records the full import lists and hashes.
These generated files are local build outputs and are intentionally gitignored.

Verification passed: 29 Rust tests, 32 Python tests, 12 frontend unit tests, 10 browser
integration workflows, TypeScript/Arabic-contract/format checks, NSIS hook compilation
with warnings denied, and PowerShell syntax/process-tree accounting on Linux. The
build is unsigned. Windows installation, native interaction, physical printers and
Windows startup/memory budgets remain **unmeasured**. The Windows CI workflow has
been supplied but has not been dispatched as part of this local delivery.

## Offline runtime and installer behavior

The compact installer requires a previously installed **WebView2 Evergreen x64 runtime**.
It checks the Microsoft machine/user registry locations and rejects missing runtimes
or versions below 109, with an Arabic message. That number is an API compatibility
floor; deploy a maintained runtime version. The installer neither downloads nor updates
WebView2. For a disconnected PC, obtain Microsoft's standalone offline runtime installer
on a connected machine, copy it to USB, and install it before the ERP.

Embedding Microsoft's offline runtime would exceed the 20 MB budget; its size is not
included in the ERP installer measurement. See [Tauri Windows distribution](https://v2.tauri.app/distribute/windows-installer/)
and [Microsoft WebView2 distribution](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution).

- Arabic is the only installer language. Custom messages originate in
  `src/i18n/ar.json` under `installer`; `scripts/generate_installer.py` emits the
  checked-in `src-tauri/windows/Arabic.nsh` file. NSIS supplies standard wizard text.
- Installation is per user and needs no administrator elevation for the ERP.
- Downgrades are disabled. Close the ERP before installing, reinstalling or removing
  it. A Restart Manager guard refuses an in-use executable instead of terminating it.
- User data lives under `%LOCALAPPDATA%\com.plumbingstore.erp\store`, separately from
  the installed program. Ordinary uninstall leaves that data in place. The optional
  data-removal checkbox explicitly deletes local store data and must be left unchecked
  to retain it. Keep external backups before upgrades or any explicit deletion.
- Release binaries are unsigned unless a distributor supplies a signing certificate
  and configures Tauri signing. No private key or signing identity is included. Windows
  can show an unknown-publisher warning. A SHA-256 checksum identifies the delivered
  file; it is not an Authenticode signature.

## Build locally on Fedora

Requirements: Docker Engine, Node.js 22.22.0 (or compatible Node 22), npm 11.21.1,
internet for the initial dependency/SDK downloads, and free space for compilation.
The Docker build uses pinned Rust 1.95.0 and cargo-xwin 0.23.1 with Clang/LLVM and NSIS.
MSVC compilation is supported by the Windows SDK/CRT supplied through xwin, not MinGW.
The target-scoped `.cargo/config.toml` enables static CRT linkage for MSVC builds;
the artifact verifier checks the resulting imports instead of assuming the flag worked.
See the [Rust linkage reference](https://doc.rust-lang.org/reference/linkage.html#static-and-dynamic-c-runtimes).

Review the [Microsoft SDK/CRT terms](https://go.microsoft.com/fwlink/?LinkId=2086102)
before using cargo-xwin: its [upstream README](https://github.com/rust-cross/cargo-xwin)
states that using it constitutes acceptance. The explicit flag records the operator's
decision for this build command; it is not needed for normal development or tests.

From the repository root:

```bash
npx --yes --package npm@11.21.1 npm ci
bash scripts/build-windows-fedora.sh --accept-sdk-license
```

The script builds `docker/Dockerfile.windows-release`, then compiles and bundles the
application as your UID/GID. The SELinux `:Z` mount supports Fedora. It does not install
Windows toolchains on the host. Dependencies and build outputs remain in `output/`:

| Directory | Purpose |
| --- | --- |
| `output/cargo` | Rust dependency cache |
| `output/xwin` | Microsoft SDK/CRT cache |
| `output/bundler-cache` | Tauri NSIS plugin cache |
| `output/target-msvc` | Windows MSVC objects, executable and NSIS output |
| `output/releases/0.1.0/fedora-cross` | Verified installer, manifest and SHA-256 file |

The first build is much slower than subsequent builds. `CARGO_BUILD_JOBS=2` limits
parallel Rust compilation. Do not run two release builds in the same target directory.
Internet is needed to populate the build caches; no build-cache contents are deployed.

The equivalent command inside the configured container is:

```bash
npm run tauri -- build --runner cargo-xwin --target x86_64-pc-windows-msvc --ci -- --locked
python3 scripts/verify_release.py --target-dir output/target-msvc --origin fedora-cross
```

Do not treat a successful GNU-target `cargo check` as an MSVC installer build. The GNU
configuration from stage 4 is only a fast binding/compile check.

## Build and validate natively on Windows

Install Visual Studio Build Tools with **Desktop development with C++**, Windows SDK,
Rust/rustup, Node.js 22.22.0, Python 3.11+ (the `python` command on PATH), and WebView2.
These are build-machine prerequisites. Use PowerShell from the repository root:

```powershell
npm install --global npm@11.21.1
rustup toolchain install 1.95.0 --profile minimal --component rustfmt --component clippy
rustup target add x86_64-pc-windows-msvc --toolchain 1.95.0
npm ci
npm run format:check
npm run check
npm test
python -m unittest discover -s tests -v
python scripts/export_schema.py --check
cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check
cargo test --locked --manifest-path src-tauri/Cargo.toml --no-default-features
cargo clippy --locked --manifest-path src-tauri/Cargo.toml --features desktop --all-targets -- -D warnings
./scripts/check-powershell.ps1
npm run build:windows
npm run verify:release -- --origin windows-native
```

Stop if any command fails. The default native bundle is:

```text
src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/PlumbingStore_0.1.0_x64-setup.exe
```

If you set `CARGO_TARGET_DIR`, pass the same directory to `verify_release.py` using
`--target-dir`. The native workflow uses `output/windows-target` for this purpose.
`npm run build:windows` invokes the TypeScript, Arabic-label and generated-contract
checks before the Vite production build, then uses the locked Cargo dependency graph.

`.github/workflows/windows-release.yml` provides this Windows 2022 runner build, backend
tests and disposable-profile install/start/reinstall/uninstall checks. It runs on manual
dispatch or `v*` tags after the source is pushed. It uploads artifacts and test evidence;
it does not publish a GitHub release or sign the installer. Adding the workflow locally
does not mean it has executed remotely.

## What the artifact verifier checks

`scripts/verify_release.py` reads the actual PE files and fails on:

- Package/Cargo/Tauri version mismatch, a missing bundled Arabic font, source maps,
  non-Arabic NSIS configuration, enabled downgrade or a WebView download/update setting.
- A non-x64 application or an unexpected NSIS launcher architecture. NSIS itself uses
  an x86 launcher to install the x64 application; that is expected.
- Imports requiring a separately installed VC++/GNU runtime DLL.
- An installer of **20,000,000 bytes or more**. MB means decimal MB here, not MiB.

After passing, it copies the real installer into `output/releases/<version>/<origin>`
and writes `release-manifest.json` plus `SHA256SUMS`. The manifest records actual sizes,
PE imports, SHA-256 hashes and build origin. It explicitly leaves runtime/performance
claims to Windows evidence. There are no substitute executables or simulated results.

## Windows installation smoke test

Run in a disposable Windows account or fresh VM with WebView2 installed, with the
network disconnected for offline acceptance. The script deliberately refuses an account
with existing ERP data or an existing registered installation:

```powershell
./scripts/smoke-windows.ps1 `
  -Installer ./output/releases/0.1.0/fedora-cross/PlumbingStore_0.1.0_x64-setup.exe `
  -DisposableProfile
```

It silently installs into a path containing Arabic characters and spaces, waits for
the frontend's actual ready signal, closes the app gracefully, checks the new SQLite
database read-only, reinstalls the same version and verifies data hashes, starts again,
then uninstalls with default data retention. It verifies schema migrations, WAL mode,
SQLite integrity, foreign keys and the first-run state. It writes
`output/windows-smoke/smoke.json`, retaining test data as evidence. Rerun in a fresh
profile. This is a same-version reinstallation check; a future version still requires
an upgrade test against a backup of the previous version's database.

On a separate disposable VM without WebView2, the silent installer must return 1603
without downloading anything. An unsupported OS/architecture returns 1633; an in-use
installation returns 1618. Interactive mode must show the corresponding Arabic message.
Also review the RTL installer, setup/login, barcode sales, returns, Excel import/export,
backup/restore and Arabic paths using the installed application, then print A4/80 mm
samples on the store's actual printers. Browser integration tests do not replace this.

## Measure startup and idle memory honestly

Use the installed **release** executable on the intended Windows reference PC, with
WebView2 present and no other ERP process running. The script uses an opt-in environment
variable, `PLUMBING_ERP_STARTUP_REPORT`, to request a new local JSON file. It is not
telemetry; normal launches do not write this diagnostic file. IPC cannot choose a path.

The ready signal comes after bootstrap data, the first-run/login screen, local fonts
and two rendered frames. Rust's internal time starts at `main`; the script's wall time
also includes process creation and its 50 ms polling interval. It does not measure the
time needed for a user to type a password or populate a dashboard after login.

After rebooting, before launching the ERP once:

```powershell
./scripts/measure-windows.ps1 `
  -Executable "$env:LOCALAPPDATA/PlumbingStore/plumbing-erp.exe" `
  -Runs 1 -ColdAfterReboot -EnforceBudgets
```

Use the real installed path if a different location was selected. `ColdAfterReboot`
records your assertion that this is the first launch after reboot; it cannot prove a
cold filesystem cache. Repeat on separate reboots for representative cold-start data.
Ordinary repeat launches can be measured separately:

```powershell
./scripts/measure-windows.ps1 `
  -Executable "$env:LOCALAPPDATA/PlumbingStore/plumbing-erp.exe" -Runs 5
```

After 15 seconds idle, each run samples memory ten times. It sums **private working
set for the application and all descendant WebView2 processes**, records private commit
separately, and uses the largest sampled working set against the **150,000,000-byte**
budget. It does not conceal the webview's memory by reporting only the Rust process.
Record additional Task Manager observations for a representative loaded store session;
the automated script measures the startup/login state only.

JSON reports in `output/windows-performance` include hardware, OS, last boot, WebView2
version, binary hash, timing and memory samples. Enforcing budgets requires a cold run
and fails if wall startup is at least 2,000 ms or idle private working set is at least
150,000,000 bytes. A warm run never claims the cold-start target passed. Processes close
through the window close request; the script never force-kills the store.

## Fedora checks independent of a Windows machine

```bash
npm run format:check
npm run check
npm test
npm run test:ui
python3 -m unittest discover -s tests -v
python3 scripts/export_schema.py --check
python3 scripts/generate_installer.py --check
bash scripts/test-backend.sh
docker build -f docker/Dockerfile.installer-check -t plumbing-erp-installer-check:stage5 .
docker run --rm --user "$(id -u):$(id -g)" -v "$PWD:/work:Z" -w /work \
  plumbing-erp-installer-check:stage5 makensis -V3 tests/installer-hooks.nsi
docker run --rm --user "$(id -u):$(id -g)" -e HOME=/tmp \
  -v "$PWD:/work:Z" -w /work mcr.microsoft.com/powershell:7.5-debian-12 \
  pwsh -NoLogo -NoProfile -File scripts/check-powershell.ps1
```

The NSIS fixture compiles installer hooks into a temporary compiler-test file inside
the container; it is never distributed. The PowerShell check parses every script and
tests descendant-process memory accounting with deterministic inputs. Neither runs
Windows. The fixture image extends `plumbing-erp-tools:stage2`, described in
[DEVELOPMENT.md](DEVELOPMENT.md).

## Versioning and distribution

Update `package.json`, its lockfile, `src-tauri/Cargo.toml`, its lockfile and
`src-tauri/tauri.conf.json` consistently before a new release. Add migrations instead
of changing ones already applied to a deployed database. `.gitattributes` enforces LF
so a Windows checkout cannot change the embedded migration checksums; Python tools
read and write Arabic as explicit UTF-8 independently of the Windows locale. Take a verified external
backup before upgrades. Keep the installer and its manifest together; if signing is
added, sign before recording the final checksum and size. The
[Arabic operator guide](../README_AR.md) and [developer README](../README.md)
complete the operating and development documentation.
