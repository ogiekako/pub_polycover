# Hardness Gadget Search Artifacts

This directory stores the code used to search and verify the hardness gadget in the FUN 2026 paper draft (`hardness.tex`).

## Contents

- `rust_search/`
  - Parallel Rust program (Rayon) used for exhaustive neighborhood search around seed gadgets.
  - Output format: JSON list of candidate gadgets.
- `search_hardness_gadget.py`
  - Python exploratory search script used during earlier iterations.
- `verify_rust_candidates.py`
  - Independent Python checker used to validate Rust candidates.
  - Checks include connectedness constraints for `Q0`, single-cover uniqueness, and absence of multi-cover solutions.

## Quick Start

Run Rust search:

```bash
cd hardness/rust_search
cargo run --release > /tmp/rust_search_results.json
```

Run Python verification on Rust output:

```bash
cd hardness
python3 verify_rust_candidates.py --candidates /tmp/rust_search_results.json
```

## Reproducibility Smoke Test

To quickly check deterministic behavior, run a reduced search (`--max-flips 3`) twice and compare bytes:

```bash
cd hardness/rust_search
cargo run --release -- --max-flips 3 > /tmp/rust_search_repro_a.json
cargo run --release -- --max-flips 3 > /tmp/rust_search_repro_b.json
sha256sum /tmp/rust_search_repro_a.json /tmp/rust_search_repro_b.json
cmp /tmp/rust_search_repro_a.json /tmp/rust_search_repro_b.json
```

If both checks match, output ordering/content are deterministic for that mode.
