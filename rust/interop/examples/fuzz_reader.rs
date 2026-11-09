use std::error::Error;

fn random(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

fn main() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args().skip(1);
    let count: usize = args
        .next()
        .ok_or("usage: fuzz_reader CASES SEED_FILES...")?
        .parse()?;
    let mut seeds = vec![vec![], vec![0; 32], vec![255; 64]];
    for path in args {
        seeds.push(std::fs::read(path)?);
    }
    let mut state = 0x59aff00d12345678;
    for index in 0..count {
        let mut data = seeds[index % seeds.len()].clone();
        for _ in 0..(random(&mut state) % 16 + 1) {
            let at = (random(&mut state) as usize) % (data.len() + 1);
            match random(&mut state) % 4 {
                0 => {
                    if let Some(byte) = data.get_mut(at) {
                        *byte ^= random(&mut state) as u8;
                    }
                }
                1 => data.truncate(at),
                2 if data.len() < 65536 => data.insert(at, random(&mut state) as u8),
                _ => {
                    if let Some(tail) = data.get_mut(at..at.saturating_add(4)) {
                        tail.fill(255);
                    }
                }
            }
        }
        yaff_interop::fuzz::exercise(&data);
    }
    println!("Mutation fuzzing: {count} deterministic cases passed (seed 0x59aff00d12345678)");
    Ok(())
}
