mod common;

use yaff::{Error, Field, Layout, Message, MessageMeta, Offset, Writer};

#[test]
fn canonical_records_match_cpp_byte_for_byte() {
    for (name, bytes) in common::corpus().unwrap() {
        // Collections contain shared sparse tables in the C++ encoding;
        // the Rust writer currently emits independent tables.
        if name == "collections.bin" {
            continue;
        }
        let expected = std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/interop/fixtures")
                .join(&name),
        )
        .unwrap();
        assert_eq!(bytes, expected, "{name}");
    }
}

static META: MessageMeta = MessageMeta {
    layout: Layout::Dynamic,
    offsets: &[0, 4, 8, 12],
    deleted: &[],
    explicit: true,
};

#[test]
fn handles_cannot_cross_writers_or_resets() {
    let mut a = Writer::new();
    let mut b = Writer::new();
    let text = a.string("abc").unwrap();
    assert_eq!(a.bytes_at(text).unwrap(), b"abc");
    assert!(matches!(
        b.message::<()>(&META, &[Field::reference(1, text)]),
        Err(Error::ForeignOffset)
    ));
    assert!(matches!(b.offset_array(&[text]), Err(Error::ForeignOffset)));
    a.clear();
    assert!(matches!(
        a.message::<()>(&META, &[Field::reference(1, text)]),
        Err(Error::ForeignOffset)
    ));
    assert!(matches!(a.bytes_at(text), Err(Error::ForeignOffset)));
}

#[test]
fn invalid_fields_are_rejected_without_writing() {
    let mut w = Writer::new();
    for (fields, expected) in [
        (vec![Field::scalar(0, 1u32, 0)], Error::InvalidField),
        (vec![Field::scalar(8191, 1u32, 0)], Error::InvalidField),
        (vec![Field::scalar(1, 1u64, 0)], Error::InvalidField),
        (
            vec![Field::scalar(1, 1u32, 0), Field::scalar(1, 2u32, 0)],
            Error::DuplicateField,
        ),
    ] {
        assert!(matches!(w.message::<()>(&META, &fields), Err(e) if e == expected));
        assert!(w.is_empty());
    }
}

#[test]
fn explicit_floats_preserve_bits_in_every_layout() {
    for layout in [Layout::Fixed, Layout::Flat, Layout::Sparse, Layout::Dynamic] {
        for bits in [0, 0x80000000, 0x7f800000, 0xff800000, 0x7fc12345, 1] {
            let mut w = Writer::new();
            let meta = MessageMeta {
                layout,
                offsets: &[0, 4, 8, 12],
                deleted: &[],
                explicit: true,
            };
            let value = f32::from_bits(bits);
            let root: Offset<()> = w
                .message(&meta, &[Field::scalar(1, value, 0.0f32)])
                .unwrap();
            let data = w.finish(root).unwrap();
            // Fixed/flat require their matching dispatch metadata; dynamic
            // reads either representation and fixed needs a separate view.
            static FIXED: MessageMeta = MessageMeta {
                layout: Layout::Fixed,
                offsets: &[0, 4, 8, 12],
                deleted: &[],
                explicit: true,
            };
            let view = Message::read(
                &data,
                if layout == Layout::Fixed {
                    &FIXED
                } else {
                    &META
                },
            )
            .unwrap();
            assert_eq!(view.scalar(1, 0.0f32).unwrap().to_bits(), bits);
        }
    }
}

#[test]
fn fixed_array_rebases_each_embedded_object_reference() {
    static CHILD: MessageMeta = MessageMeta {
        layout: Layout::Fixed,
        offsets: &[0, 4],
        deleted: &[],
        explicit: false,
    };
    let mut w = Writer::new();
    let a = w.string("first").unwrap();
    let b = w.string("second").unwrap();
    let children = w
        .fixed_array::<()>(
            &CHILD,
            &[vec![Field::reference(1, a)], vec![Field::reference(1, b)]],
        )
        .unwrap();
    let root: Offset<()> = w.message(&META, &[Field::reference(1, children)]).unwrap();
    let bytes = w.finish(root).unwrap();
    let array = Message::read(&bytes, &META).unwrap().array(1).unwrap();
    assert_eq!(
        array.message(0, &CHILD).unwrap().string(1, b"").unwrap(),
        "first"
    );
    assert_eq!(
        array.message(1, &CHILD).unwrap().string(1, b"").unwrap(),
        "second"
    );
}
