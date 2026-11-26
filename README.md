# Tessera

*Working name — subject to a trademark check before launch.*

A privacy-first, cross-platform disk-space analyser for Windows and macOS.
It shows what is using your storage, where it is, and (later) helps you
recover space safely through a review-and-Trash workflow.

**Current stage:** the scanning engine (the core of the product) is built and
tested on Windows. The desktop app, website and installers are not built yet.
See [STATUS.md](STATUS.md) for exactly what works and how it was verified.

## For the owner: running what exists today

All commands are typed into **PowerShell** (Start menu → type "PowerShell").

1. Go to the project folder:

   ```powershell
   cd C:\Users\mengh\projects\tessera
   ```

2. Build the command-line scanner (first build takes a few minutes):

   ```powershell
   cargo build --release -p tessera-cli
   ```

3. List your drives, with capacity as reported by Windows:

   ```powershell
   .\target\release\tessera-cli.exe volumes
   ```

4. Scan any folder (read-only — it never changes or deletes anything):

   ```powershell
   .\target\release\tessera-cli.exe scan "C:\Program Files"
   ```

5. Run the automated tests (they only touch temporary folders):

   ```powershell
   cargo test --workspace
   ```

## For developers

* Toolchain: Rust 1.100.0 (pinned in `rust-toolchain.toml`); on Windows,
  Visual Studio 2022 Build Tools with "Desktop development with C++".
* Layout and design: [ARCHITECTURE.md](ARCHITECTURE.md).
* Tests: [TESTING.md](TESTING.md). Benchmarks: [BENCHMARKS.md](BENCHMARKS.md).
* Security model: [SECURITY.md](SECURITY.md).

Benchmark fixtures:

```powershell
.\target\release\tessera-cli.exe gen .fixtures\f100k --files 100000
.\target\release\tessera-cli.exe bench .fixtures\f100k --runs 5
```

`gen` only writes into a new, empty folder; `bench` refuses to run on any
folder that is not a generated fixture.

## Licence

Proprietary. All rights reserved. Third-party components:
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
