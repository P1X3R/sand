mod perft_logic;

use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::time::{Duration, Instant};

use perft_logic::*;
use sand::chess::*;

fn main() -> io::Result<()> {
    const USAGE_MSG: &str = r#"Usage: perft_test <file> <depth> <hash size in mb>
The file format is EDP: <FEN> [;D<depth1> <expected1> D<depth2> <expected2> ...]"#;

    let path = env::args().nth(1).expect(USAGE_MSG);
    let test_depth = env::args()
        .nth(2)
        .expect(USAGE_MSG)
        .parse::<usize>()
        .expect("Invalid depth");
    let hash_mb = env::args()
        .nth(3)
        .expect(USAGE_MSG)
        .parse::<usize>()
        .expect("Invalid hash size");

    let file = File::open(&path)?;
    let reader = BufReader::new(file);
    let mut table = PerftTT::new(hash_mb);

    let mut total_nodes = 0;
    let mut total_elapsed = Duration::ZERO;

    for line in reader.lines() {
        let line = line?;
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }

        let fen = fields[..6].join(" ");
        println!("{fen}");

        let expected: Vec<u32> = fields
            .iter()
            .skip(7) // skip fen
            .step_by(2) // skip `;D<depth>` comment
            .take(test_depth)
            .map(|nodes| nodes.parse::<u32>().expect("Invalid digit"))
            .collect();

        let mut board = Board::new(&fen).unwrap();

        for (idx, &expected_nodes) in expected.iter().enumerate() {
            let depth = (idx + 1) as u8;
            let depth_start = Instant::now();
            let nodes = perft(&mut board, depth, &mut table);
            let elapsed = depth_start.elapsed();

            total_nodes += nodes;
            total_elapsed += elapsed;

            println!(
                "Depth {depth}: {nodes}; Expected nodes: {expected_nodes}; Time: {:?}",
                elapsed
            );
            assert_eq!(nodes, expected_nodes);
        }
        println!();
    }

    println!(
        "Estimated: {:.0} N/s",
        total_nodes as f64 / total_elapsed.as_secs_f64(),
    );

    Ok(())
}
