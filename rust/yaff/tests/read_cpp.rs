use yaff::{Error, Layout, Message, MessageMeta};

static SIMPLE: MessageMeta = MessageMeta {
    layout: Layout::Dynamic,
    offsets: &[0, 1, 5, 13, 17],
    deleted: &[],
    explicit: true,
};
static FIXED: MessageMeta = MessageMeta {
    layout: Layout::Fixed,
    offsets: &[0, 1, 5, 13, 17],
    deleted: &[],
    explicit: true,
};
static ROOT: MessageMeta = MessageMeta {
    layout: Layout::Dynamic,
    offsets: &[0, 4, 8, 12, 16, 20, 24],
    deleted: &[],
    explicit: true,
};
static PAIR: MessageMeta = MessageMeta {
    layout: Layout::Fixed,
    offsets: &[0, 4, 12],
    deleted: &[],
    explicit: false,
};
static SPARSE: MessageMeta = MessageMeta {
    layout: Layout::Sparse,
    offsets: &[],
    deleted: &[],
    explicit: true,
};
static NEW: MessageMeta = MessageMeta {
    layout: Layout::Dynamic,
    offsets: &[0, 8, 8, 8, 9, 17, 21],
    deleted: &[2, 3],
    explicit: true,
};

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/interop/fixtures")
            .join(format!("{name}.bin")),
    )
    .unwrap()
}

#[test]
fn cpp_root_strings_and_arrays() {
    let bytes = fixture("root_bytes");
    let string = yaff::ByteString::read(&bytes).unwrap();
    assert_eq!(string.as_bytes(), b"\xff\0z");
    assert_eq!(string.as_str(), Err(Error::InvalidUtf8));
    let bytes = fixture("root_array");
    let array = yaff::Array::read(&bytes).unwrap();
    array.check_width(8).unwrap();
    assert_eq!(
        array
            .scalars::<i64>()
            .iter()
            .collect::<yaff::Result<Vec<_>>>()
            .unwrap(),
        [i64::MIN, 0, i64::MAX]
    );
}

#[test]
fn cached_headers_preserve_fixed_and_rebound_metadata_semantics() {
    static TINY: MessageMeta = MessageMeta {
        layout: Layout::Fixed,
        offsets: &[0, 1],
        deleted: &[],
        explicit: false,
    };
    // A fixed message may be shorter than a header. Reading it must still work.
    let bytes = [4, 0, 0, 0, 1];
    let fixed = Message::read(&bytes, &TINY).unwrap();
    assert!(fixed.scalar(1, false).unwrap());
    assert_eq!(
        fixed.with_meta(&SPARSE).scalar(1, 0u32),
        Err(Error::OutOfBounds)
    );
    let data = fixture("flat_explicit_sized");
    let dynamic = Message::read(&data, &SIMPLE).unwrap();
    assert_eq!(
        dynamic.with_meta(&SPARSE).scalar(1, 0u32),
        Err(Error::InvalidLayout)
    );
    let default = Message::default_for(&TINY).with_meta(&SPARSE);
    assert_eq!(default.scalar(1, 123u32).unwrap(), 123);
    #[cfg(target_pointer_width = "64")]
    assert_eq!(std::mem::size_of::<Message<'_>>(), 32);
}

#[test]
fn all_cpp_layouts_and_presence_modes() {
    for (name, meta, present) in [
        ("fixed_explicit", &FIXED, false),
        ("flat_explicit", &SIMPLE, true),
        ("flat_explicit_sized", &SIMPLE, true),
        ("flat_implicit", &SIMPLE, false),
        ("flat_implicit_sized", &SIMPLE, false),
        ("sparse_explicit", &SIMPLE, true),
        ("sparse_implicit", &SIMPLE, false),
    ] {
        let data = fixture(name);
        let m = Message::read(&data, meta).unwrap();
        assert!(m.scalar(1, false).unwrap(), "{name}");
        assert_eq!(m.scalar(2, 175u32).unwrap(), 175, "{name}");
        assert_eq!(m.has(2, 4).unwrap(), present, "{name}");
        assert_eq!(m.scalar(3, 1.25f64).unwrap(), -12.5, "{name}");
        assert_eq!(m.bytes(4, b"").unwrap(), b"A\0B", "{name}");
        // Every truncation must be rejected while reading this complete value.
        for len in 0..data.len() {
            let result = Message::read(&data[..len], meta).and_then(|v| {
                v.scalar(1, false)?;
                v.scalar(2, 175u32)?;
                v.scalar(3, 1.25f64)?;
                v.bytes(4, b"")
            });
            assert!(result.is_err(), "{name} accepted truncation at {len}");
        }
    }
}

#[test]
fn cpp_collections_and_shared_metadata() {
    let data = fixture("collections");
    let m = Message::read(&data, &ROOT).unwrap();
    let nums = m.array(1).unwrap().scalars::<i64>();
    assert_eq!(
        nums.iter().collect::<yaff::Result<Vec<_>>>().unwrap(),
        [i64::MIN, -1, 0, i64::MAX]
    );
    assert_eq!(nums.get(4), Err(Error::OutOfBounds));
    assert_eq!(
        m.array(2)
            .unwrap()
            .scalars::<bool>()
            .iter()
            .collect::<yaff::Result<Vec<_>>>()
            .unwrap(),
        [true, false, true]
    );
    let strings = m.array(3).unwrap().strings();
    assert_eq!(strings.get(0).unwrap(), b"");
    assert_eq!(strings.get(1).unwrap(), b"\xff\0z");
    assert_eq!(strings.get_str(1), Err(Error::InvalidUtf8));
    assert_eq!(strings.get(2).unwrap(), b"");
    // C++ deduplicates both empty strings and the two sparse metadata tables.
    assert_eq!(
        strings.get(0).unwrap().as_ptr(),
        strings.get(2).unwrap().as_ptr()
    );
    let messages = m.array(4).unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(
        messages
            .message(0, &SPARSE)
            .unwrap()
            .scalar(1, 0u32)
            .unwrap(),
        10
    );
    assert_eq!(
        messages
            .message(1, &SPARSE)
            .unwrap()
            .scalar(1, 0u32)
            .unwrap(),
        20
    );
    let pairs = m.array(5).unwrap();
    for i in 0..2 {
        let pair = pairs.message(i, &PAIR).unwrap();
        assert_eq!(pair.scalar(1, 0i32).unwrap(), i as i32 + 1);
        assert_eq!(pair.scalar(2, 0u64).unwrap(), (i as u64 + 1) * 100);
    }
    assert!(m.has(6, 4).unwrap());
    assert_eq!(m.string(6, b"fallback").unwrap(), "");
}

#[test]
fn schema_evolution_corrects_flat_offsets() {
    for (name, value) in [("evolution_explicit", 10u64), ("evolution_implicit", 11)] {
        let data = fixture(name);
        let m = Message::read(&data, &NEW).unwrap();
        assert_eq!(m.scalar(1, 0u64).unwrap(), 20);
        assert!(m.scalar(4, false).unwrap());
        assert!(m.has(5, 8).unwrap());
        assert_eq!(m.scalar(5, 10u64).unwrap(), value);
        assert!(!m.has(6, 4).unwrap());
        assert_eq!(m.scalar(6, 1337u32).unwrap(), 1337);
    }
}

#[test]
fn sparse_table_width_boundaries_and_unknown_enum() {
    let data = fixture("sparse_boundaries");
    let m = Message::read(&data, &SPARSE).unwrap();
    assert_eq!(m.scalar(1, 0i32).unwrap(), -777);
    assert!(m.scalar(31, false).unwrap());
    assert_eq!(m.scalar(32, 0u64).unwrap(), u64::MAX);
    assert_eq!(m.scalar(8190, 0i32).unwrap(), -123);
    assert!(!m.has(30, 4).unwrap());
    assert_eq!(m.scalar(0, 0u32), Err(Error::InvalidField));
    assert_eq!(m.scalar(8191, 0u32), Err(Error::InvalidField));
}

#[test]
fn empty_and_absent_messages_return_schema_defaults() {
    for name in ["empty_flat", "empty_sparse"] {
        let data = fixture(name);
        let m = Message::read(&data, &SIMPLE).unwrap();
        assert!(!m.has(1, 1).unwrap());
        assert_eq!(m.scalar(2, 175u32).unwrap(), 175);
        assert_eq!(m.bytes(4, b"default").unwrap(), b"default");
        assert_eq!(
            m.message(4, &FIXED).unwrap().scalar(2, 175u32).unwrap(),
            175
        );
        assert!(m.array(4).unwrap().is_empty());
    }
}

#[test]
fn malformed_data_never_panics() {
    // Deterministic mutation coverage, also exercises unsigned/signed overflow
    // paths without allowing the unchecked C++ reader to consume corrupt data.
    for name in ["flat_explicit_sized", "sparse_explicit", "collections"] {
        let original = fixture(name);
        for i in 0..original.len() {
            for value in [0, 1, 127, 128, 255] {
                let mut bytes = original.clone();
                bytes[i] = value;
                if let Ok(m) = Message::read(&bytes, &ROOT) {
                    for field in 1..=6 {
                        let _ = m.scalar(field, 0u32);
                        let _ = m.has(field, 4);
                        let _ = m.bytes(field, b"");
                        if let Ok(a) = m.array(field) {
                            for index in [0, 1, usize::MAX] {
                                let _ = a.scalar::<bool>(index);
                                let _ = a.bytes(index);
                                let _ = a.message(index, &SPARSE);
                            }
                        }
                    }
                }
            }
        }
    }
}
