#![cfg(feature = "trusted")]

use yaff::{trusted, Layout, Message, MessageMeta};

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
static NEW: MessageMeta = MessageMeta {
    layout: Layout::Dynamic,
    offsets: &[0, 8, 8, 8, 9, 17, 21],
    deleted: &[2, 3],
    explicit: true,
};
static NEW_FLAT: MessageMeta = MessageMeta {
    layout: Layout::Flat,
    offsets: NEW.offsets,
    deleted: NEW.deleted,
    explicit: true,
};
static SPARSE: MessageMeta = MessageMeta {
    layout: Layout::Sparse,
    offsets: &[],
    deleted: &[],
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
fn trusted_matches_checked_on_all_cpp_layout_variants() {
    for name in [
        "fixed_explicit",
        "flat_explicit",
        "flat_explicit_sized",
        "flat_implicit",
        "flat_implicit_sized",
        "sparse_explicit",
        "sparse_implicit",
    ] {
        let bytes = fixture(name);
        let meta = if name.starts_with("fixed") {
            &FIXED
        } else {
            &SIMPLE
        };
        let checked = Message::read(&bytes, meta).unwrap();
        // SAFETY: immutable serializer-produced fixtures and matching schema.
        unsafe {
            let view = trusted::Message::read_root(&bytes, meta.layout);
            assert_eq!(
                view.scalar(1, 0, meta, false),
                checked.scalar(1, false).unwrap()
            );
            assert_eq!(
                view.scalar(2, 1, meta, 175u32),
                checked.scalar(2, 175u32).unwrap()
            );
            assert_eq!(view.has(2, 1, 4, meta), checked.has(2, 4).unwrap());
            assert_eq!(
                view.scalar(3, 5, meta, 1.25f64),
                checked.scalar(3, 1.25f64).unwrap()
            );
            assert_eq!(view.bytes(4, 13, meta, b""), checked.bytes(4, b"").unwrap());
        }
    }
}

#[test]
fn trusted_handles_deleted_fields_and_sparse_boundaries() {
    for name in ["evolution_explicit", "evolution_implicit"] {
        let bytes = fixture(name);
        let checked = Message::read(&bytes, &NEW).unwrap();
        // SAFETY: these fixtures are generated for the compatible old schema.
        unsafe {
            let view = trusted::Message::read_root(&bytes, NEW.layout);
            assert_eq!(
                view.scalar(4, 8, &NEW, false),
                checked.scalar(4, false).unwrap()
            );
            assert_eq!(
                view.scalar(5, 9, &NEW, 10u64),
                checked.scalar(5, 10u64).unwrap()
            );
            assert_eq!(view.has(5, 9, 8, &NEW), checked.has(5, 8).unwrap());
            assert_eq!(view.scalar(6, 17, &NEW, 1337u32), 1337);
            // The same old buffer through a specialized flat reader.
            assert_eq!(
                view.scalar(4, 8, &NEW_FLAT, false),
                checked.scalar(4, false).unwrap()
            );
            assert_eq!(
                view.scalar(5, 9, &NEW_FLAT, 10u64),
                checked.scalar(5, 10u64).unwrap()
            );
        }
    }
    let bytes = fixture("sparse_boundaries");
    // SAFETY: the fixture uses a sparse schema; flat offsets are unused.
    unsafe {
        let view = trusted::Message::read_root(&bytes, SPARSE.layout);
        assert_eq!(view.scalar(1, 0, &SPARSE, 0i32), -777);
        assert!(view.scalar(31, 0, &SPARSE, false));
        assert_eq!(view.scalar(32, 0, &SPARSE, 0u64), u64::MAX);
        assert_eq!(view.scalar(8190, 0, &SPARSE, 0i32), -123);
    }
}

#[test]
fn trusted_default_storage_preserves_defaults_without_null_dereferences() {
    for meta in [&SIMPLE, &FIXED, &SPARSE] {
        let view = trusted::Message::default_for(meta.layout);
        // SAFETY: default storage supports the selected layout and valid slots.
        unsafe {
            assert_eq!(view.scalar(2, 1, meta, 175u32), 175);
            assert!(!view.has(2, 1, 4, meta));
            assert_eq!(view.bytes(4, 13, meta, b"default"), b"default");
        }
    }
    fn send_sync<T: Send + Sync>() {}
    send_sync::<trusted::Message<'static>>();
    #[cfg(target_pointer_width = "64")]
    assert_eq!(std::mem::size_of::<trusted::Message<'_>>(), 16);
}

#[test]
fn trusted_follows_shared_sparse_metadata() {
    static ROOT: MessageMeta = MessageMeta {
        layout: Layout::Dynamic,
        offsets: &[0, 4, 8, 12, 16, 20, 24],
        deleted: &[],
        explicit: true,
    };
    #[derive(Clone, Copy)]
    struct Child<'a>(trusted::Message<'a>);
    impl<'a> trusted::View<'a> for Child<'a> {
        const META: &'static MessageMeta = &SPARSE;
        unsafe fn from_message(message: trusted::Message<'a>) -> Self {
            Self(message)
        }
        fn message(self) -> trusted::Message<'a> {
            self.0
        }
    }
    let bytes = fixture("collections");
    // SAFETY: the C++ fixture contains the described root and sparse children.
    unsafe {
        let root = trusted::Message::read_root(&bytes, ROOT.layout);
        let children = root.array(4, 12, &ROOT).messages::<Child<'_>>();
        assert_eq!(children.get(0).unwrap().0.scalar(1, 0, &SPARSE, 0u32), 10);
        assert_eq!(children.get(1).unwrap().0.scalar(1, 0, &SPARSE, 0u32), 20);
    }
}
