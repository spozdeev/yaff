#[path = "../tests/common/mod.rs"]
mod common;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::env::args_os()
        .nth(1)
        .ok_or("usage: wire_corpus OUTPUT_DIRECTORY")?;
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory)?;
    for (name, bytes) in common::corpus()? {
        std::fs::write(directory.join(name), bytes)?;
    }
    println!("Rust wire corpus written");
    Ok(())
}
