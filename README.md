# OpenForge
Open source CurseForge patcher

## Why

CurseForge was sold to Overwolf, an Israeli company, you get the idea...
There are MULTIPLE telemetry, advertisement, and data scrape endpoints that point to overwolf, they are harvesting your data without your consent.

## Requirements
- Windows 10 or later
- Rust (cargo)

## Crates

| Crate             | Purpose                                                                 |
|-------------------|-------------------------------------------------------------------------|
| `openforge`       | The client library + `openforge` CLI and `openforge-gui` binaries       |
| `openforge-patch` | Patches the `.ASAR` and removes the Ads and more.    |

## Build

```sh
cargo build --release -p openforge-patch
```