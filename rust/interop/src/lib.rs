include!(concat!(env!("OUT_DIR"), "/yaff.rs"));
include!(concat!(env!("OUT_DIR"), "/light.rs"));

pub mod fuzz;

#[cfg(test)]
mod tests;
