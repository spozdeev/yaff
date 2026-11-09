fn main() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let plugin = std::env::var_os("YAFF_PROTOC_GEN_RUST")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| root.join("build/local/src/protoc-plugin/protoc-gen-yaff-rust"));
    let protos = [
        "tests/protoyaff/proto2.proto",
        "tests/protoyaff/proto3.proto",
        "tests/protoyaff/proto_api.proto",
        "tests/interop/schema.proto",
        "tests/interop/rust_names.proto",
    ]
    .map(|p| root.join(p));
    let includes = [
        root.join("include"),
        root.join("tests"),
        root.join("tests/interop"),
    ];
    yaff_build::Config::new()
        .plugin(&plugin)
        .trusted(true)
        .compile(&protos, &includes)
        .unwrap();
    yaff_build::Config::new()
        .plugin(&plugin)
        .trusted(true)
        .slice("light")
        .namespace("light")
        .output("light.rs")
        .compile(&[root.join("tests/interop/schema.proto")], &includes)
        .unwrap();
}
