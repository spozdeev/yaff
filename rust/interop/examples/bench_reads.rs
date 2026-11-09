use std::hint::black_box;
use std::time::Instant;
use yaff_interop::protoyaff::interop::Everything;

fn measure(name: &str, mut read: impl FnMut() -> u64) {
    const ITERATIONS: usize = 5_000_000;
    for _ in 0..1_000_000 {
        black_box(read());
    }
    let mut samples = Vec::new();
    for _ in 0..7 {
        let mut sum = 0u64;
        let start = Instant::now();
        for _ in 0..ITERATIONS {
            sum = sum.wrapping_add(read());
        }
        samples.push(start.elapsed().as_nanos() as f64 / ITERATIONS as f64);
        black_box(sum);
    }
    samples.sort_by(f64::total_cmp);
    println!("rust,{name},{:.3}", samples[3]);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::env::args_os()
        .nth(1)
        .ok_or("usage: bench_reads GENERATED_BUFFER")?;
    let bytes = std::fs::read(path)?;
    let view = Everything::read(&bytes)?;
    measure("scalar", || black_box(&view).get_id().unwrap() as u64);
    measure("nested", || {
        black_box(&view).get_child().unwrap().get_counter().unwrap()
    });
    measure("array", || {
        black_box(&view).get_numbers().unwrap().get(2).unwrap() as u64
    });
    Ok(())
}
