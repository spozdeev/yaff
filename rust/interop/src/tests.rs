use crate::protoyaff::interop::*;
use std::path::{Path, PathBuf};
use yaff::{Error, Layout, Writer};

macro_rules! assert_payload {
    ($message:expr) => {{
        let m = $message;
        assert_eq!(m.get_id().unwrap(), i64::MIN);
        assert!(m.has_zero().unwrap());
        assert_eq!(m.get_zero().unwrap(), 0);
        assert_eq!(m.get_text().unwrap().as_str().unwrap(), "hello");
        assert_eq!(m.get_payload().unwrap(), b"\xff\0");
        assert_eq!(m.get_state().unwrap().0, -777);
        assert!(m.has_child().unwrap());
        let child = m.get_child().unwrap();
        assert!(child.has_note().unwrap());
        assert_eq!(child.get_note().unwrap().as_bytes(), b"");
        assert_eq!(child.get_counter().unwrap(), u64::MAX);
        assert_eq!(
            m.get_numbers()
                .unwrap()
                .iter()
                .collect::<yaff::Result<Vec<_>>>()
                .unwrap(),
            [-10, 0, 10]
        );
        assert_eq!(m.get_words().unwrap().get(0).unwrap(), b"");
        assert_eq!(m.get_words().unwrap().get_str(1).unwrap(), "world");
        assert_eq!(m.get_blobs().unwrap().get(0).unwrap(), b"\xff\0");
        assert_eq!(m.get_states().unwrap().get(0).unwrap(), State::ACTIVE);
        assert_eq!(m.get_states().unwrap().get(1).unwrap().0, 12345);
        let children = m.get_children().unwrap();
        assert_eq!(children.len(), 2);
        assert!(!children.get(0).unwrap().has_note().unwrap());
        assert_eq!(children.get(1).unwrap().get_counter().unwrap(), u64::MAX);
        let points = m.get_points().unwrap();
        assert_eq!(points.len(), 2);
        assert_eq!(points.get(0).unwrap().get_x().unwrap(), 1.0);
        assert_eq!(points.get(1).unwrap().get_y().unwrap(), 4.0);
        assert_eq!(
            m.get_lookup()
                .unwrap()
                .get(0)
                .unwrap()
                .get_key()
                .unwrap()
                .as_bytes(),
            b"a"
        );
        assert_eq!(
            m.find_lookup(b"z").unwrap().unwrap().get_counter().unwrap(),
            u64::MAX
        );
        assert!(m.find_lookup(b"absent").unwrap().is_none());
        assert!(!m.find_lookup(b"a").unwrap().unwrap().has_note().unwrap());
        assert_eq!(m.find_labels(-2).unwrap().unwrap().as_bytes(), b"negative");
        assert_eq!(m.find_labels(0).unwrap().unwrap().as_bytes(), b"");
        assert_eq!(m.find_labels(7).unwrap().unwrap().as_bytes(), b"positive");
        assert_eq!(
            m.which_choice().unwrap(),
            Everything_choiceCase::Field_zero_choice
        );
        assert!(m.has_zero_choice().unwrap());
        assert!(!m.has_text_choice().unwrap());
        assert_eq!(m.get_zero_choice().unwrap(), 0);
        let defaults = m.get_defaults().unwrap();
        assert!(defaults.has_signed32().unwrap());
        assert_eq!(defaults.get_signed32().unwrap(), -123);
        assert!(!defaults.has_unsigned32().unwrap());
        assert_eq!(defaults.get_unsigned32().unwrap(), u32::MAX);
        assert_eq!(defaults.get_signed64().unwrap(), i64::MIN);
        assert_eq!(defaults.get_unsigned64().unwrap(), u64::MAX);
        assert_eq!(defaults.get_single().unwrap(), 0.1234f32);
        assert_eq!(defaults.get_real().unwrap(), 0.000001f64);
        assert!(defaults.has_flag().unwrap());
        assert!(defaults.get_flag().unwrap());
        assert_eq!(defaults.get_text().unwrap().as_bytes(), b"");
        assert_eq!(defaults.get_raw().unwrap(), b"\0\xff");
        let sparse = m.get_sparse().unwrap();
        assert_eq!(sparse.get_first().unwrap(), -10);
        assert!(sparse.get_tiny_last().unwrap());
        assert_eq!(sparse.get_wide_first().unwrap(), u64::MAX);
        assert_eq!(sparse.get_last().unwrap(), -123);
        assert_eq!(m.get_point().unwrap().get_x().unwrap(), 3.0);
        assert_eq!(m.get_point().unwrap().get_y().unwrap(), 4.0);
        assert_eq!(
            m.get_flags()
                .unwrap()
                .iter()
                .collect::<yaff::Result<Vec<_>>>()
                .unwrap(),
            [true, false]
        );
        assert!(m.get_enabled().unwrap());
        assert_eq!(m.get_large().unwrap(), -123.5);
        assert_eq!(m.get_small().unwrap(), 1.25);
        assert_eq!(
            m.get_doubles().unwrap().get(0).unwrap().to_bits(),
            (-0.0f64).to_bits()
        );
        assert_eq!(m.get_doubles().unwrap().get(1).unwrap(), 1.5);
        assert_eq!(m.get_timestamp().unwrap().get_seconds().unwrap(), -123);
        assert_eq!(m.get_timestamp().unwrap().get_nanos().unwrap(), 456);
        assert_eq!(m.get_node().unwrap().get_value().unwrap(), 1);
        assert_eq!(
            m.get_node()
                .unwrap()
                .get_next()
                .unwrap()
                .get_value()
                .unwrap(),
            2
        );
    }};
}

fn check(bytes: &[u8]) {
    assert_payload!(Everything::read(bytes).unwrap());
    // SAFETY: callers use complete fixtures from the C++ serializer or the
    // generated Rust builders. Malformed-input fuzzing never calls this helper.
    let trusted = unsafe { Everything::read_trusted(bytes) };
    assert_payload!(trusted);
    if let Some(flat) = trusted.as_flat() {
        assert!(trusted.as_sparse().is_none());
        assert_payload!(flat);
    } else {
        assert_payload!(trusted.as_sparse().unwrap());
    }
}

fn create(layout: Layout) -> Vec<u8> {
    let mut w = Writer::new();
    w.set_dynamic_layout(layout).unwrap();
    let empty = w.string("").unwrap();
    let child_builder = ChildBuilder {
        note: Some(empty),
        counter: Some(u64::MAX),
    };
    let child = child_builder.write(&mut w).unwrap();
    let empty_child = ChildBuilder::default().write(&mut w).unwrap();
    let children =
        ChildBuilder::write_array(&mut w, &[ChildBuilder::default(), child_builder]).unwrap();
    let a = w.string("a").unwrap();
    let z = w.string("z").unwrap();
    let lookup = Everything_LookupEntryBuilder::write_array(
        &mut w,
        &[
            Everything_LookupEntryBuilder {
                key: Some(z),
                value: Some(child),
            },
            Everything_LookupEntryBuilder {
                key: Some(a),
                value: Some(empty_child),
            },
        ],
    )
    .unwrap();
    let negative = w.string("negative").unwrap();
    let positive = w.string("positive").unwrap();
    let labels = Everything_LabelsEntryBuilder::write_array(
        &mut w,
        &[
            Everything_LabelsEntryBuilder {
                key: 7,
                value: Some(positive),
            },
            Everything_LabelsEntryBuilder {
                key: 0,
                value: Some(empty),
            },
            Everything_LabelsEntryBuilder {
                key: -2,
                value: Some(negative),
            },
        ],
    )
    .unwrap();
    let world = w.string("world").unwrap();
    let words = w.offset_array(&[empty, world]).unwrap();
    let raw = w.bytes(b"\xff\0").unwrap();
    let blobs = w.offset_array(&[raw]).unwrap();
    let raw_default = w.bytes(b"\0\xff").unwrap();
    let defaults = DefaultsBuilder {
        signed32: Some(-123),
        flag: Some(true),
        text: Some(empty),
        raw: Some(raw_default),
        ..Default::default()
    }
    .write(&mut w)
    .unwrap();
    let point_builder = PointBuilder { x: 3.0, y: 4.0 };
    let point = point_builder.write(&mut w).unwrap();
    let points =
        PointBuilder::write_array(&mut w, &[PointBuilder { x: 1.0, y: 2.0 }, point_builder])
            .unwrap();
    let sparse = SparseBuilder {
        first: -10,
        tiny_last: true,
        wide_first: u64::MAX,
        last: -123,
    }
    .write(&mut w)
    .unwrap();
    let timestamp = crate::protoyaff::google::protobuf::TimestampBuilder {
        seconds: -123,
        nanos: 456,
    }
    .write(&mut w)
    .unwrap();
    let tail = NodeBuilder {
        value: 2,
        next: None,
    }
    .write(&mut w)
    .unwrap();
    let node = NodeBuilder {
        value: 1,
        next: Some(tail),
    }
    .write(&mut w)
    .unwrap();
    let root = EverythingBuilder {
        id: i64::MIN,
        zero: Some(0),
        text: Some(w.string("hello").unwrap()),
        payload: Some(raw),
        state: State(-777),
        child: Some(child),
        numbers: Some(w.scalar_array(&[-10i32, 0, 10]).unwrap()),
        words: Some(words),
        blobs: Some(blobs),
        states: Some(w.scalar_array(&[1i32, 12345]).unwrap()),
        children: Some(children),
        points: Some(points),
        lookup: Some(lookup),
        labels: Some(labels),
        zero_choice: Some(0),
        text_choice: None,
        defaults: Some(defaults),
        sparse: Some(sparse),
        point: Some(point),
        flags: Some(w.scalar_array(&[true, false]).unwrap()),
        enabled: true,
        large: -123.5,
        small: 1.25,
        doubles: Some(w.scalar_array(&[-0.0f64, 1.5]).unwrap()),
        timestamp: Some(timestamp),
        node: Some(node),
    }
    .write(&mut w)
    .unwrap();
    w.finish(root).unwrap()
}

fn cpp(arguments: &[&Path]) {
    let executable = std::env::var_os("YAFF_CPP_INTEROP")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../build/local/tests/interop/yaff_generated_interop")
        });
    let result = std::process::Command::new(executable)
        .args(arguments)
        .output()
        .expect("build yaff_generated_interop before running these tests");
    assert!(
        result.status.success(),
        "C++ oracle failed: {}{}",
        String::from_utf8_lossy(&result.stdout),
        String::from_utf8_lossy(&result.stderr)
    );
}

struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        loop {
            let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("yaff-interop-{}-{nonce}-{id}", std::process::id()));
            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => panic!("cannot create interop temporary directory: {error}"),
            }
        }
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn generated_bidirectional_round_trips() {
    let temp = Temp::new();
    for (layout, mode) in [
        (Layout::Flat, "write-flat"),
        (Layout::Sparse, "write-sparse"),
    ] {
        let cpp_file = temp.0.join("cpp.bin");
        cpp(&[Path::new(mode), &cpp_file]);
        let input = std::fs::read(&cpp_file).unwrap();
        check(&input);
        let mut w = Writer::new();
        w.set_dynamic_layout(layout).unwrap();
        let root = Everything::read(&input).unwrap().copy_to(&mut w).unwrap();
        let copied = w.finish(root).unwrap();
        check(&copied);
        let rust_file = temp.0.join("rust.bin");
        std::fs::write(&rust_file, &copied).unwrap();
        cpp(&[Path::new("check"), &rust_file]);
        let native = create(layout);
        check(&native);
        std::fs::write(&rust_file, &native).unwrap();
        cpp(&[Path::new("transcode"), &rust_file, &cpp_file]);
        check(&std::fs::read(&cpp_file).unwrap());
    }
}

#[test]
fn slice_remaps_ids_in_both_languages() {
    let temp = Temp::new();
    let file = temp.0.join("slice.bin");
    cpp(&[Path::new("write-slice"), &file]);
    let bytes = std::fs::read(&file).unwrap();
    assert_eq!(
        crate::light::interop::Sliced::read(&bytes)
            .unwrap()
            .get_id()
            .unwrap(),
        77
    );
    let mut w = Writer::new();
    let root = crate::light::interop::SlicedBuilder { id: 77 }
        .write(&mut w)
        .unwrap();
    std::fs::write(&file, w.finish(root).unwrap()).unwrap();
    cpp(&[Path::new("check-slice"), &file]);
}

#[test]
fn builder_rejects_invalid_oneof_and_duplicate_map_keys() {
    let mut w = Writer::new();
    let text = w.string("").unwrap();
    let invalid = EverythingBuilder {
        zero_choice: Some(0),
        text_choice: Some(text),
        ..Default::default()
    };
    assert!(matches!(invalid.write(&mut w), Err(Error::InvalidOneof)));
    let map = [
        Everything_LabelsEntryBuilder {
            key: 1,
            value: Some(text),
        },
        Everything_LabelsEntryBuilder {
            key: 1,
            value: None,
        },
    ];
    assert!(matches!(
        Everything_LabelsEntryBuilder::write_array(&mut w, &map),
        Err(Error::InvalidMap)
    ));
}

#[test]
fn recursive_copy_has_a_depth_budget() {
    let mut w = Writer::new();
    let mut next = None;
    for value in 0..150 {
        next = Some(NodeBuilder { value, next }.write(&mut w).unwrap());
    }
    let bytes = w.finish(next.unwrap()).unwrap();
    let node = Node::read(&bytes).unwrap();
    assert!(matches!(
        node.copy_to(&mut Writer::new()),
        Err(Error::DepthLimit)
    ));
    let mut output = Writer::new();
    let root = node
        .to_builder(&mut output, 151)
        .unwrap()
        .write(&mut output)
        .unwrap();
    let bytes = output.finish(root).unwrap();
    assert_eq!(Node::read(&bytes).unwrap().get_value().unwrap(), 149);
}

#[test]
fn generated_names_and_enum_aliases() {
    let mut w = Writer::new();
    let mut builder = r#type::builder();
    builder.self_ = Some(1);
    builder.read = Some(2);
    builder.fields = Some(3);
    let root = builder.write(&mut w).unwrap();
    let bytes = w.finish(root).unwrap();
    let view = r#type::read(&bytes).unwrap();
    assert_eq!(view.get_self().unwrap(), 1);
    assert_eq!(view.get_read().unwrap(), 2);
    assert_eq!(view.get_fields().unwrap(), 3);
    assert_eq!(State::UNKNOWN, State::UNKNOWN_ALIAS);
    let mut w = Writer::new();
    let mut builder = crate::protoyaff::rustnames::Vec_::builder();
    builder.value = 42;
    let root = builder.write(&mut w).unwrap();
    let bytes = w.finish(root).unwrap();
    assert_eq!(
        crate::protoyaff::rustnames::Vec_::read(&bytes)
            .unwrap()
            .get_value()
            .unwrap(),
        42
    );
    assert_eq!(
        crate::protoyaff::rustnames::Option_::default()
            .get_value()
            .unwrap(),
        0
    );
}

#[test]
fn trusted_native_roundtrip_and_defaults() {
    for layout in [Layout::Flat, Layout::Sparse] {
        let bytes = create(layout);
        check(&bytes);
        // SAFETY: generated Rust builders produced the complete matching graph.
        let trusted = unsafe { Everything::read_trusted(&bytes) };
        assert!(matches!(
            trusted.get_numbers().unwrap().get(usize::MAX),
            Err(Error::OutOfBounds)
        ));
        assert!(matches!(
            trusted.get_children().unwrap().get(usize::MAX),
            Err(Error::OutOfBounds)
        ));
        assert!(matches!(
            trusted.get_blobs().unwrap().get(usize::MAX),
            Err(Error::OutOfBounds)
        ));
    }
    let empty = EverythingTrusted::<0>::default();
    assert!(!empty.has_zero().unwrap());
    assert_eq!(empty.get_child().unwrap().get_counter().unwrap(), 0);
    assert!(empty.get_numbers().unwrap().is_empty());
    assert_eq!(
        DefaultsTrusted::<0>::default().get_signed64().unwrap(),
        i64::MIN
    );
    assert_eq!(
        DefaultsTrusted::<0>::default().get_raw().unwrap(),
        b"\0\xff"
    );
    assert_eq!(PointTrusted::<0>::default().get_y().unwrap(), 0.0);
    assert!(PointTrusted::<0>::default().as_flat().is_none());
    assert!(PointTrusted::<0>::default().as_sparse().is_none());
}

#[test]
fn schema_evolution_across_both_languages_and_layouts() {
    let temp = Temp::new();
    let file = temp.0.join("evolution.bin");
    cpp(&[Path::new("write-old"), &file]);
    let data = std::fs::read(&file).unwrap();
    let new = NewShape::read(&data).unwrap();
    assert_eq!(new.get_first().unwrap(), 10);
    assert_eq!(new.get_tail().unwrap(), 20);
    assert_eq!(new.get_added().unwrap().as_bytes(), b"");
    cpp(&[Path::new("write-new"), &file]);
    let data = std::fs::read(&file).unwrap();
    let old = OldShape::read(&data).unwrap();
    assert_eq!(old.get_first().unwrap(), 10);
    assert_eq!(old.get_retired().unwrap(), 0);
    assert_eq!(old.get_tail().unwrap(), 20);
    for layout in [Layout::Flat, Layout::Sparse] {
        let mut w = Writer::new();
        w.set_dynamic_layout(layout).unwrap();
        let root = OldShapeBuilder {
            first: 10,
            retired: 9,
            tail: 20,
        }
        .write(&mut w)
        .unwrap();
        std::fs::write(&file, w.finish(root).unwrap()).unwrap();
        cpp(&[Path::new("check-new"), &file]);
    }
    let mut w = Writer::new();
    let added = w.string("future").unwrap();
    let root = NewShapeBuilder {
        first: 10,
        tail: 20,
        added: Some(added),
    }
    .write(&mut w)
    .unwrap();
    std::fs::write(&file, w.finish(root).unwrap()).unwrap();
    cpp(&[Path::new("check-old"), &file]);
}
