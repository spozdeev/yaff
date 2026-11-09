//! Compare checked and trusted generated readers on C++ serializer fixtures.
//! Inputs must be valid immutable buffers produced by the interop fixture tool.
use std::hint::black_box;
use std::time::Instant;
use yaff::trusted::View;
use yaff_interop::protoyaff::interop::{Everything, EverythingTrusted};

#[inline(always)]
fn opaque_ref<T>(value: &T) -> &T {
    let mut pointer = value as *const T;
    // SAFETY: the empty compiler barrier leaves the live reference unchanged.
    unsafe {
        std::arch::asm!("/* {pointer} */", pointer = inout(reg) pointer, options(nostack, preserves_flags));
        &*pointer
    }
}

#[inline(always)]
fn opaque_trusted<const MODE: u8>(
    view: EverythingTrusted<'_, MODE>,
) -> EverythingTrusted<'_, MODE> {
    let mut pointer = view.message().as_ptr();
    let mut header = usize::from(view.message().header());
    // SAFETY: the barrier does not modify the pointer or its immutable graph.
    // Reconstruct the same generated view with the same borrowed lifetime.
    unsafe {
        std::arch::asm!("/* {pointer} {header} */", pointer = inout(reg) pointer, header = inout(reg) header, options(nostack, preserves_flags));
        EverythingTrusted::from_message(yaff::trusted::Message::from_parts(pointer, header as u16))
    }
}

#[inline(always)]
fn opaque_bytes(bytes: &[u8]) -> &[u8] {
    let mut ptr = bytes.as_ptr();
    // SAFETY: the empty barrier preserves the original slice pointer and length.
    unsafe {
        std::arch::asm!("/* {ptr} */", ptr = inout(reg) ptr, options(nostack, preserves_flags));
        std::slice::from_raw_parts(ptr, bytes.len())
    }
}

fn measure(name: &str, mut read: impl FnMut() -> u64) {
    const COUNT: usize = 5_000_000;
    for _ in 0..1_000_000 {
        black_box(read());
    }
    let mut samples = Vec::new();
    for _ in 0..7 {
        let start = Instant::now();
        let mut sum = 0u64;
        for _ in 0..COUNT {
            sum = sum.wrapping_add(read());
        }
        samples.push(start.elapsed().as_nanos() as f64 / COUNT as f64);
        black_box(sum);
    }
    samples.sort_by(f64::total_cmp);
    println!("{name},{:.4}", samples[3]);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(
        std::env::args_os()
            .nth(1)
            .ok_or("expected a C++ interop fixture")?,
    )?;
    let checked = Everything::read(&bytes)?;
    assert!(checked.get_numbers()?.len() >= 3);
    // SAFETY: the benchmark accepts serializer-produced fixtures, not arbitrary
    // or fuzzed input. The checked read above is a sanity check, not a verifier.
    let trusted = unsafe { Everything::read_trusted(&bytes) };
    assert_eq!(checked.get_id()?, trusted.get_id()?);
    assert_eq!(
        checked.get_child()?.get_counter()?,
        trusted.get_child()?.get_counter()?
    );
    assert_eq!(
        checked.get_numbers()?.get(2)?,
        trusted.get_numbers()?.get(2)?
    );
    println!("checked_view_bytes,{}", std::mem::size_of_val(&checked));
    println!("trusted_view_bytes,{}", std::mem::size_of_val(&trusted));
    measure("checked_open_scalar", || {
        Everything::read(opaque_bytes(&bytes))
            .unwrap()
            .get_id()
            .unwrap() as u64
    });
    measure("trusted_open_scalar", || {
        // SAFETY: same immutable serializer-produced fixture as above.
        unsafe { Everything::read_trusted(opaque_bytes(&bytes)) }
            .get_id()
            .unwrap() as u64
    });
    measure("specialized_open_scalar", || {
        // SAFETY: same immutable serializer-produced fixture as above.
        let view = unsafe { Everything::read_trusted(opaque_bytes(&bytes)) };
        if let Some(flat) = view.as_flat() {
            flat.get_id().unwrap() as u64
        } else {
            view.as_sparse().unwrap().get_id().unwrap() as u64
        }
    });
    measure("checked_scalar", || {
        opaque_ref(&checked).get_id().unwrap() as u64
    });
    measure("trusted_scalar", || {
        opaque_trusted(trusted).get_id().unwrap() as u64
    });
    measure("checked_nested", || {
        opaque_ref(&checked)
            .get_child()
            .unwrap()
            .get_counter()
            .unwrap()
    });
    measure("trusted_nested", || {
        opaque_trusted(trusted)
            .get_child()
            .unwrap()
            .get_counter()
            .unwrap()
    });
    measure("checked_array", || {
        opaque_ref(&checked).get_numbers().unwrap().get(2).unwrap() as u64
    });
    measure("trusted_array", || {
        opaque_trusted(trusted)
            .get_numbers()
            .unwrap()
            .get(2)
            .unwrap() as u64
    });
    measure("trusted_array_unchecked_index", || {
        // SAFETY: the immutable fixture's array length was checked above.
        unsafe {
            opaque_trusted(trusted)
                .get_numbers()
                .unwrap()
                .get_unchecked(2) as u64
        }
    });
    if let Some(flat) = trusted.as_flat() {
        specialized(flat);
    } else if let Some(sparse) = trusted.as_sparse() {
        specialized(sparse);
    }
    Ok(())
}

fn specialized<const MODE: u8>(view: EverythingTrusted<'_, MODE>) {
    measure("specialized_scalar", || {
        opaque_trusted(view).get_id().unwrap() as u64
    });
    measure("specialized_nested", || {
        let child = opaque_trusted(view).get_child().unwrap();
        if let Some(flat) = child.as_flat() {
            flat.get_counter().unwrap()
        } else {
            child.as_sparse().unwrap().get_counter().unwrap()
        }
    });
    measure("specialized_array", || {
        opaque_trusted(view).get_numbers().unwrap().get(2).unwrap() as u64
    });
    measure("specialized_array_unchecked_index", || {
        // SAFETY: called only after the fixture's array length has been checked.
        unsafe { opaque_trusted(view).get_numbers().unwrap().get_unchecked(2) as u64 }
    });
}
