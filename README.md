# Covering a Polyomino-Shaped Stain with Non-Overlapping Identical Stickers

This repository contains supplemental material for the paper *"Covering a Polyomino-Shaped Stain with Non-Overlapping Identical Stickers,"* accepted at The Thirteenth International Conference on Fun with Algorithms (FUN 2026).

## Overview

Puzzle creator Naoki Inaba proposed the following [problem](http://inabapuzzle.com/hirameki/suuri_7.html) on his website (translated by the README author):

> There are many identical polyomino pieces (shapes consisting of several squares connected by edges) without holes.
> 
> I tried to arrange these pieces without overlapping them to cover a 1 × 5 area, but I couldn't do it no matter how hard I tried.
> 
> ■■■■■
> 
> What do these pieces look like?

The answer can be found on the [solution page](http://inabapuzzle.com/hirameki/suuri_ans7.html) of the same site. As stated there, this puzzle can be generalized into the following problem:

*   We say that a polyomino $P$ can cover a polyomino $Q$ if multiple copies of $P$ can cover $Q$ without overlapping (rotations and reflections are allowed). For a given polyomino $Q$, either prove that **any** polyomino $P$ can cover $Q$, or provide a **counterexample** (a specific polyomino $P$ that cannot cover $Q$).

We have solved this generalized problem for an arbitrary $Q$.

This repository contains the complete list of counterexamples required for the full proof, alongside the program used to discover them.

## Directory Structure

*   `problem/`
    *   `<num>/<name>.never` - Polyominoes consisting of `<num>` cells that **any** polyomino can cover (refer to the paper for the proof).
    *   `<num>/<name>.yes` - Polyominoes consisting of `<num>` cells for which there exists a counterexample polyomino that **cannot** cover it.
*   `solution/`
    *   `<num>/<name>.txt` - The shape of the specific counterexample polyomino that cannot cover the corresponding problem polyomino.
*   `src/`
    *   The Rust source code of the program used to find these counterexamples.

## How to Run the Program

First, install the latest stable version of [Rust](https://www.rust-lang.org/), and then run the following command. It will initiate the search and output any polyominoes it finds that cannot cover the given target $Q$.

```bash
Q=problem/6/T.yes # The target polyomino (Q) to be covered
N=23              # The maximum height and width of the covering polyomino (P)
I=50000           # The number of iterations

RUST_LOG=info cargo run --release solve $Q $N $I
```

You can also append the following optional arguments to customize the search:

*   `--forbidden <D>`: A heuristic flag to speed up the search. It restricts the relative distance between two polyominoes to be at least `D`. This prunes deeply interlocking shapes, assuming they are highly likely to cause overlaps.
*   `--never-retry`: Disables the heuristic that reverts the simulated annealing process to the best-known minimum penalty state when it gets stuck in a local optimum for an extended period.
*   `--no-change-side`: Restricts the search space size. If no valid polyomino is found within the `N` $\times$ `N` bounding box, the program will terminate instead of automatically attempting larger sizes.
*   `--nproc <X>`: Speeds up the search by running `X` independent simulated annealing processes in parallel using multiple CPU cores (defaults to 16).


## Hardness Gadget Search Artifacts

Additional code used for the NP-hardness gadget search/verification is stored in `hardness/`.

- `hardness/rust_search/`: Rust + Rayon search implementation.
- `hardness/search_hardness_gadget.py`: Python exploratory search script.
- `hardness/verify_rust_candidates.py`: independent Python verifier for Rust outputs.
- `hardness/README.md`: usage notes for this directory.
