//! Cargo integration for the C++-built `protoc-gen-yaff-rust` executable.
//! Generated code depends only on the Rust `yaff` crate at runtime.
#![forbid(unsafe_code)]

use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Default)]
pub struct Config {
    protoc: Option<PathBuf>,
    plugin: Option<PathBuf>,
    out_dir: Option<PathBuf>,
    namespace: Option<String>,
    slice: Option<String>,
    output: Option<String>,
    trusted: Option<String>,
}

impl Config {
    /// Generate optional trusted views. Also enable the runtime's `trusted` feature.
    pub fn trusted(&mut self, enabled: bool) -> &mut Self {
        self.trusted = Some(enabled.to_string());
        self
    }
    pub fn new() -> Self {
        Self::default()
    }
    pub fn protoc(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.protoc = Some(path.into());
        self
    }
    pub fn plugin(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.plugin = Some(path.into());
        self
    }
    pub fn out_dir(&mut self, path: impl Into<PathBuf>) -> &mut Self {
        self.out_dir = Some(path.into());
        self
    }
    pub fn namespace(&mut self, namespace: impl Into<String>) -> &mut Self {
        self.namespace = Some(namespace.into());
        self
    }
    pub fn slice(&mut self, slice: impl Into<String>) -> &mut Self {
        self.slice = Some(slice.into());
        self
    }
    pub fn output(&mut self, output: impl Into<String>) -> &mut Self {
        self.output = Some(output.into());
        self
    }

    /// Compile all inputs and their referenced imports into one module tree.
    /// Include the resulting file at crate root. Include directories must also
    /// cover `yaff/proto/options.proto` when the schema uses YaFF options.
    pub fn compile<P: AsRef<Path>, I: AsRef<Path>>(
        &self,
        protos: &[P],
        includes: &[I],
    ) -> io::Result<PathBuf> {
        if protos.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "no .proto files specified",
            ));
        }
        let out = self
            .out_dir
            .clone()
            .or_else(|| std::env::var_os("OUT_DIR").map(PathBuf::from))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "set OUT_DIR or Config::out_dir",
                )
            })?;
        std::fs::create_dir_all(&out)?;
        let mut command = Command::new(
            self.protoc
                .clone()
                .or_else(|| std::env::var_os("PROTOC").map(PathBuf::from))
                .unwrap_or_else(|| "protoc".into()),
        );
        if let Some(plugin) = self
            .plugin
            .clone()
            .or_else(|| std::env::var_os("YAFF_PROTOC_GEN_RUST").map(PathBuf::from))
        {
            command.arg(format!(
                "--plugin=protoc-gen-yaff-rust={}",
                plugin.display()
            ));
            println!("cargo:rerun-if-changed={}", plugin.display());
        }
        command.arg(format!("--yaff-rust_out={}", out.display()));
        for (key, value) in [
            ("namespace", &self.namespace),
            ("tag", &self.slice),
            ("output", &self.output),
            ("trusted", &self.trusted),
        ] {
            if let Some(value) = value {
                if value.contains(',') {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "YaFF options cannot contain commas",
                    ));
                }
                command.arg(format!("--yaff-rust_opt={key}={value}"));
            }
        }
        for include in includes {
            command.arg("-I").arg(include.as_ref());
            // Watching roots also catches imported schema changes.
            println!("cargo:rerun-if-changed={}", include.as_ref().display());
        }
        for proto in protos {
            command.arg(proto.as_ref());
            println!("cargo:rerun-if-changed={}", proto.as_ref().display());
        }
        println!("cargo:rerun-if-env-changed=PROTOC");
        println!("cargo:rerun-if-env-changed=YAFF_PROTOC_GEN_RUST");
        let result = command.output()?;
        if !result.status.success() {
            return Err(io::Error::other(format!(
                "YaFF generation failed: {}",
                String::from_utf8_lossy(&result.stderr)
            )));
        }
        Ok(out.join(self.output.as_deref().unwrap_or("yaff.rs")))
    }
}
