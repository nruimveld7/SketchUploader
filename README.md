# ALDER

Arduino Library Development Environment Resolver is a desktop companion for Arduino development workflows that rely on `arduino-cli`.  
It is intended to provide a focused, repeatable environment for compiling and uploading sketches, selecting local library folders, and monitoring serial output without switching between multiple terminal sessions.

## Intent

This project is designed for iterative firmware and library development where tight feedback loops matter:

- Select a sketch and compile quickly against a chosen board FQBN.
- Upload to a detected serial port with configurable upload options.
- Pass local library directories to compile using `--library` to validate in-progress libraries.
- Use an integrated serial monitor to verify runtime behavior immediately after upload.

## Features

- Board discovery via `arduino-cli`
- Port discovery via native serial-port enumeration (Rust `serialport`)
- Compile and upload actions with command output logging
- Local library folder support for compile-time overrides
- Optional visibility of installed libraries reported by `arduino-cli`
- Integrated serial monitor (connect, stream, send input, line ending control)
- Persistent app settings in `alder.config.json`
- Startup checks for `arduino-cli` availability and required cores
- Missing `arduino-cli` prompt with one-click install on Windows (via `winget`)

## Tech Stack

- Frontend: React + Vite + TypeScript
- Desktop runtime: Tauri 2
- Native backend: Rust
- Arduino integration: `arduino-cli`

## Prerequisites

- Node.js + npm
- Rust toolchain (`rustup`, `cargo`)
- Tauri system dependencies for your OS
- `arduino-cli` installed and available in `PATH`

Helper scripts are included:

- Windows: `scripts/CheckDeps.ps1`
- macOS/Linux: `scripts/CheckDeps.sh`

## Getting Started

1. Install dependencies:

```bash
npm install
```

2. Run the desktop app in development mode:

```bash
npm run tauri:dev
```

3. Build a production app:

```bash
npm run tauri:build
```

## Release Artifacts For Downstream CI

This repository publishes desktop binaries on version tags (`v*`) via GitHub Actions.

- Workflow: `.github/workflows/release.yml`
- Release assets include platform bundles plus:
  - `SHA256SUMS`
  - `release-manifest.json` (machine-readable asset list with checksums)

For dependent repositories, pin a specific version tag instead of using `latest`.
An example consumer workflow is provided at:

- `docs/downstream-release-consumer-example.yml`

## Configuration

ALDER reads and writes `alder.config.json` at runtime.  
Use `alder.config.example.json` as a reference template.

Notable configuration areas:

- `preferences`: theme, verbosity, warnings, verify/clean flags
- `libraries`: selected local library paths and installed-library list visibility
- `tools`: required cores and programmer override
- `startupChecks`: startup validation behavior
- `build`: extra compile/upload args

## Typical Workflow

1. Select local library folder(s) and a sketch (`.ino`).
2. Pick board and serial port.
3. Run **Compile** to validate build output.
4. Run **Upload** to flash the board.
5. Open **Serial Monitor** for runtime verification.

## Repository

- Source: https://github.com/nruimveld7/ALDER
- Issues: https://github.com/nruimveld7/ALDER/issues

## License

This project is licensed under the MIT License. See `LICENSE` for details.


