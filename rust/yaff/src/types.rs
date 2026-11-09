use crate::{Array, Error, Result};
use std::marker::PhantomData;

/// A protobuf string in a YaFF buffer. The wire format permits arbitrary bytes;
/// UTF-8 validation is explicit and does not allocate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct ByteString<'a>(&'a [u8]);

impl<'a> ByteString<'a> {
    /// Read a buffer finished with a string/bytes record as its root object.
    pub fn read(bytes: &'a [u8]) -> Result<Self> {
        Ok(Self(crate::read::root_bytes(bytes)?))
    }

    pub fn new(bytes: &'a [u8]) -> Self {
        Self(bytes)
    }
    pub fn as_bytes(self) -> &'a [u8] {
        self.0
    }
    pub fn as_str(self) -> Result<&'a str> {
        std::str::from_utf8(self.0).map_err(|_| Error::InvalidUtf8)
    }
}

pub trait OpenEnum: Copy {
    fn from_raw(value: i32) -> Self;
    fn raw(self) -> i32;
}

#[derive(Clone, Copy, Debug)]
pub struct EnumArray<'a, E>(Array<'a>, PhantomData<E>);

impl<'a, E: OpenEnum + 'a> EnumArray<'a, E> {
    pub fn new(array: Array<'a>) -> Self {
        Self(array, PhantomData)
    }
    pub fn len(&self) -> usize {
        self.0.len()
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub fn get(&self, index: usize) -> Result<E> {
        Ok(E::from_raw(self.0.scalar(index)?))
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = Result<E>> + 'a {
        (0..self.len()).map(move |i| self.get(i))
    }
}
