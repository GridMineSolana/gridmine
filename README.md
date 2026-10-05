# GRIDMINE

GRIDMINE ($GRID) is a 25-tile mining game on Solana, forked from ORE. Each round players deploy SOL across
tiles; a VRF-drawn winning tile shares the round's SOL and $GRID with its miners, plus that tile's asset pot.

Mainnet program id: `FAv6eoP4y6wJz2qCTDdKwcLQ2mE93trMi8sZpYdrvpoQ` (https://gridmine.fun)

## Layout

- `api/` accounts, instructions, SDK builders
- `program/` instruction processors and LiteSVM tests
- `mock_vrf/`, `mock_swap/` test-only stand-ins for MagicBlock VRF and Jupiter

## Build and test

```sh
cargo build-sbf          # target/deploy/grid.so (+ mocks used by tests)
cargo test               # tests load target/deploy/*.so
cargo clippy --all-targets
```

## Verify the deployed program

```sh
solana-verify build --library-name grid
solana-verify get-executable-hash target/deploy/grid.so
solana-verify get-program-hash -u mainnet FAv6eoP4y6wJz2qCTDdKwcLQ2mE93trMi8sZpYdrvpoQ
```

## Audits

Three internal review rounds; all findings were fixed and the final round found no new issues.
Reports are in [`audits/`](audits/). Security contact: see [SECURITY.md](SECURITY.md).

## Credit

Forked from [ORE](https://github.com/regolith-labs/ore) by Regolith Labs. Apache-2.0, see [LICENSE](LICENSE) and [NOTICE](NOTICE).
