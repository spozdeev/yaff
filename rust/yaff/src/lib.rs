//! Zero-copy views over YaFF buffers, compatible with the C++ runtime.
//!
//! Default access is bounds checked. Scalars are returned by value; variable-sized
//! data borrows the input. A view is lazy, so constructing it does not validate
//! every field. Each accessor returns an error if the data it accesses is invalid.
//! The optional `trusted` feature adds an explicit unsafe entry point for valid,
//! immutable buffers, with the same trust contract as the C++ reader.
#![cfg_attr(not(feature = "trusted"), forbid(unsafe_code))]
#![deny(unsafe_code)]

mod error;
mod read;
mod scalar;
mod types;
mod write;

#[cfg(feature = "trusted")]
#[allow(unsafe_code)]
pub mod trusted;

pub use error::{Error, Result};
pub use read::{Array, BytesArray, Layout, Message, MessageArray, MessageMeta, ScalarArray, View};
pub use scalar::Scalar;
pub use types::{ByteString, EnumArray, OpenEnum};
pub use write::{Bytes, Field, Offset, Sequence, Writer};

/// Largest offset accepted by the C++ serializer.
pub const MAX_OFFSET: usize = 0x7fff_ffff;
/// Largest YaFF field identifier (which may differ from the protobuf number).
pub const MAX_FIELD_ID: u16 = 8190;
/// Generator/runtime API version; independent of an individual buffer's layout.
pub const VERSION: u32 = 100;
