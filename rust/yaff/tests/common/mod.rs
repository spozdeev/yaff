use std::collections::BTreeMap;
use yaff::{Field, Layout, MessageMeta, Offset, Result, Writer};

pub fn corpus() -> Result<BTreeMap<String, Vec<u8>>> {
    let mut corpus = BTreeMap::new();
    for (layout, name) in [
        (Layout::Fixed, "fixed"),
        (Layout::Flat, "flat"),
        (Layout::Sparse, "sparse"),
    ] {
        for implicit in [false, true] {
            for sized in [false, true] {
                if layout != Layout::Flat && sized || layout == Layout::Fixed && implicit {
                    continue;
                }
                let meta = MessageMeta {
                    layout: if sized { Layout::Dynamic } else { layout },
                    offsets: &[0, 1, 5, 13, 17],
                    deleted: &[],
                    explicit: !implicit,
                };
                let mut w = Writer::new();
                let text = w.bytes(b"A\0B")?;
                let root: Offset<()> = w.message(
                    &meta,
                    &[
                        Field::scalar(1, true, false),
                        Field::scalar(2, 175u32, 175),
                        Field::scalar(3, -12.5f64, 1.25),
                        Field::reference(4, text),
                    ],
                )?;
                corpus.insert(
                    format!(
                        "{name}_{}{}.bin",
                        if implicit { "implicit" } else { "explicit" },
                        if sized { "_sized" } else { "" }
                    ),
                    w.finish(root)?,
                );
            }
        }
    }
    let sparse = MessageMeta {
        layout: Layout::Sparse,
        offsets: &[],
        deleted: &[],
        explicit: true,
    };
    let root_meta = MessageMeta {
        layout: Layout::Dynamic,
        offsets: &[0, 4, 8, 12, 16, 20, 24],
        deleted: &[],
        explicit: true,
    };
    let pair_meta = MessageMeta {
        layout: Layout::Fixed,
        offsets: &[0, 4, 12],
        deleted: &[],
        explicit: false,
    };
    let mut w = Writer::new();
    let nums = w.scalar_array(&[i64::MIN, -1, 0, i64::MAX])?;
    let flags = w.scalar_array(&[true, false, true])?;
    let empty = w.string("")?;
    let raw = w.bytes(b"\xff\0z")?;
    let strings = w.offset_array(&[empty, raw, empty])?;
    let first: Offset<()> = w.message(&sparse, &[Field::scalar(1, 10u32, 0)])?;
    let second: Offset<()> = w.message(&sparse, &[Field::scalar(1, 20u32, 0)])?;
    let messages = w.offset_array(&[first, second])?;
    let pairs = w.fixed_array::<()>(
        &pair_meta,
        &[
            vec![Field::scalar(1, 1i32, 0), Field::scalar(2, 100u64, 0)],
            vec![Field::scalar(1, 2i32, 0), Field::scalar(2, 200u64, 0)],
        ],
    )?;
    let root: Offset<()> = w.message(
        &root_meta,
        &[
            Field::reference(1, nums),
            Field::reference(2, flags),
            Field::reference(3, strings),
            Field::reference(4, messages),
            Field::reference(5, pairs),
            Field::reference(6, empty),
        ],
    )?;
    corpus.insert("collections.bin".into(), w.finish(root)?);
    let old = MessageMeta {
        layout: Layout::Dynamic,
        offsets: &[0, 8, 12, 20, 21, 29],
        deleted: &[],
        explicit: true,
    };
    for explicit in [false, true] {
        let mut w = Writer::new();
        let root: Offset<()> = w.message(
            &old,
            &[
                Field::scalar(1, 20u64, 0),
                Field::scalar(2, 3u32, 0),
                Field::scalar(3, 15u64, 10),
                Field::scalar(4, true, false),
                Field::scalar(5, if explicit { 10u64 } else { 11 }, 10),
            ],
        )?;
        corpus.insert(
            format!(
                "evolution_{}.bin",
                if explicit { "explicit" } else { "implicit" }
            ),
            w.finish(root)?,
        );
    }
    let mut w = Writer::new();
    let root: Offset<()> = w.message(
        &sparse,
        &[
            Field::scalar(8190, -123i32, 0),
            Field::scalar(32, u64::MAX, 0),
            Field::scalar(31, true, false),
            Field::scalar(1, -777i32, 0),
        ],
    )?;
    corpus.insert("sparse_boundaries.bin".into(), w.finish(root)?);
    for (layout, name) in [
        (Layout::Flat, "empty_flat.bin"),
        (Layout::Sparse, "empty_sparse.bin"),
    ] {
        let mut w = Writer::new();
        let meta = MessageMeta {
            layout,
            offsets: &[0],
            deleted: &[],
            explicit: true,
        };
        let root: Offset<()> = w.message(&meta, &[])?;
        corpus.insert(name.into(), w.finish(root)?);
    }
    let mut w = Writer::new();
    let root = w.bytes(b"\xff\0z")?;
    corpus.insert("root_bytes.bin".into(), w.finish(root)?);
    let mut w = Writer::new();
    let root = w.scalar_array(&[i64::MIN, 0, i64::MAX])?;
    corpus.insert("root_array.bin".into(), w.finish(root)?);
    Ok(corpus)
}
