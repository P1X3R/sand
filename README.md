# Sand

> *The first word it came to my mind and a chess engine*

A UCI-compliant chess engine in the alpha-beta framework and a major source of pain to my life. I ~~failed spectacularly~~ did my best to keep the source code elegant and idiomatic while maintaining good performance on the critical paths (I wasn't coding drunk, I swear).

## Features

### Board representation
- Bitboards
- Fancy magic bitboards
- 8x8 board (for fast look-up)

### Search
- Alpha-Beta (duhh)
- Iterative deepening
- Quiescence
- Transposition table
- Move ordering:
    * PV-move first
    * TT-move first
    * MVV/LVA
    * Killer heuristic
    * History heuristics
        - Gravity formula
        - History maluses
- Selectivity:
    * SEE pruning (quiescence only)
    * Mate distance pruning
    * Delta pruning

### Evaluation
- Material
- Piece-Square tables (~~stolen~~ borrowed from PesTO)
- Tapered evaluation

## Build

```bash
cargo build --bin sand --release
```

Specify the `--bin` because it also has other binaries (like `perft_test` and `find_magics`). 

## Usage

- To run the engine, go to `target/release` and run `./sand`.
- To run a perft test you compile with `--bin perft_test` and run `./target/release/perft_test <epd test suite> <depth> <hash table size in mb>`
- To recompute the magics (if you dare) just run `cargo r --bin find_magics -r` and copy-paste to the file `src/chess/attacks/magics.rs`

## UCI Compatibility

### Supports

- All basic UCI commands (`uci`, `isready`, `position`, `go wtime ...`, etc).
- Pondering

### Unsupported

- `go mate` and `go nodes`
- Options

## Known issues

- Might lose to Stockfish in 0.5 seconds.
- Sometimes hallucinates illegal moves when sleep-deprived.

## Performance

On my overpowered Intel Pentium Silver N5030 it achieves:

- 9M-24M N/s on perft depth 5 (depending on CPU usage and hash size).
- 1.5 M N/s on average during search.

## ELO

Sand was tested against Maia 1500 at blitz time control (`tc=60+0.5`) over 400 games (200 as White, 200 as Black) using the **Unbalanced Human Openings (UHO 2022)**.  

- Observed draw rate: ~10.5% (anti-draw openings).  
- Sand scored **1857 ± 33 ELO** assuming Maia is exactly 1500 at this time control.  

## License

Sand is currently under the MIT license. However, I'm developing the "I don't fucking care" license, which allows you to do whatever you want with the source code.
