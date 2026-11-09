use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    OutOfBounds,
    InvalidOffset,
    InvalidLayout,
    InvalidField,
    InvalidBoolean,
    InvalidString,
    InvalidUtf8,
    SizeLimit,
    ForeignOffset,
    DuplicateField,
    InvalidMap,
    InvalidOneof,
    DepthLimit,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OutOfBounds => "YaFF access is outside the buffer or array",
            Self::InvalidOffset => "invalid YaFF object offset",
            Self::InvalidLayout => "YaFF layout does not match the schema",
            Self::InvalidField => "invalid YaFF field or field width",
            Self::InvalidBoolean => "YaFF boolean is neither zero nor one",
            Self::InvalidString => "YaFF string has no zero terminator",
            Self::InvalidUtf8 => "YaFF string is not valid UTF-8",
            Self::SizeLimit => "YaFF size limit exceeded",
            Self::ForeignOffset => "YaFF offset belongs to another writer or an earlier buffer",
            Self::DuplicateField => "YaFF field was written more than once",
            Self::InvalidMap => "YaFF map contains duplicate or unordered keys",
            Self::InvalidOneof => "more than one YaFF oneof member is set",
            Self::DepthLimit => "YaFF message nesting limit exceeded",
        })
    }
}

impl std::error::Error for Error {}
