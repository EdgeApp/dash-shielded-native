# dash-shielded-native

## Unreleased

- added: Move shielded value out to a Platform or Core address
- added: Resume a shield whose asset lock is already on chain
- added: React Native reaches the same calls as Node
- added: Fund the shielded pool from L1 through an asset lock
- added: Transparent receive address, Core SPV sync, and L1 balance
- added: Smoke test covers the Orchard proving key warm-up
- added: Asset locks from an earlier run are listed and resume after a restart
- changed: Build against dashpay/platform v4.2.0-beta.7 with Rust 1.98.1
- fixed: Shielded spends after the first no longer fail for lack of an anchor
- fixed: Testnet reaches DAPI on port 1443 instead of an unreachable 443
- fixed: The macOS prebuild loads instead of being killed at require
- fixed: Android ships a library for armeabi-v7a as well as arm64-v8a
- fixed: Stopping a wallet no longer panics the shielded sync task
- fixed: Dependency installs resolve instead of failing on peer conflicts

## 0.1.0 (2026-08-31)

- Initial Node N-API + React Native UniFFI package for Dash Platform shielded addresses and transfers.
