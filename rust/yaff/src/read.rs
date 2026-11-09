use std::marker::PhantomData;

use crate::{Error, Result, Scalar, MAX_FIELD_ID, MAX_OFFSET};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Layout {
    Fixed,
    Flat,
    Sparse,
    Dynamic,
}

/// Schema metadata. Offsets exclude the flat header and include a trailing end
/// offset. Deleted IDs identify untyped gaps, not typed deprecated fields.
#[derive(Debug)]
pub struct MessageMeta {
    pub layout: Layout,
    pub offsets: &'static [u16],
    pub deleted: &'static [u16],
    pub explicit: bool,
}

impl MessageMeta {
    #[inline]
    pub fn fixed_size(&self) -> usize {
        usize::from(self.offsets.last().copied().unwrap_or(0))
    }

    #[inline(always)]
    pub(crate) fn field(&self, id: u16, width: usize) -> Result<usize> {
        if id == 0 || id > MAX_FIELD_ID || self.deleted.contains(&id) {
            return Err(Error::InvalidField);
        }
        let start = self
            .offsets
            .get(usize::from(id - 1))
            .ok_or(Error::InvalidField)?;
        let end = self
            .offsets
            .get(usize::from(id))
            .ok_or(Error::InvalidField)?;
        if end.checked_sub(*start).map(usize::from) != Some(width) {
            return Err(Error::InvalidField);
        }
        Ok(usize::from(*start))
    }
}

#[inline]
pub(crate) fn range(bytes: &[u8], start: usize, size: usize) -> Result<&[u8]> {
    let end = start.checked_add(size).ok_or(Error::OutOfBounds)?;
    bytes.get(start..end).ok_or(Error::OutOfBounds)
}

#[inline]
pub(crate) fn number<T: Scalar>(bytes: &[u8], at: usize) -> Result<T> {
    T::decode(range(bytes, at, T::WIDTH)?)
}

#[inline]
fn add(at: usize, offset: usize) -> Result<usize> {
    at.checked_add(offset).ok_or(Error::InvalidOffset)
}

#[inline]
fn root(bytes: &[u8]) -> Result<usize> {
    if bytes.len() > MAX_OFFSET {
        return Err(Error::SizeLimit);
    }
    let root = number::<u32>(bytes, 0)? as usize;
    if root < 4 {
        return Err(Error::InvalidOffset);
    }
    Ok(root)
}

#[inline]
pub(crate) fn root_bytes(bytes: &[u8]) -> Result<&[u8]> {
    string(bytes, root(bytes)?)
}

#[inline]
fn string(bytes: &[u8], at: usize) -> Result<&[u8]> {
    let len = number::<u32>(bytes, at)? as usize;
    let data = add(at, 4)?;
    let end = add(data, len)?;
    if range(bytes, end, 1)?[0] != 0 {
        return Err(Error::InvalidString);
    }
    range(bytes, data, len)
}

// Decode and validate the wire tag once. Data stays immutably borrowed for
// the lifetime of every derived view, so later reads can reuse this result.
#[inline]
fn read_header(bytes: &[u8], at: usize) -> Result<u16> {
    let header = u16::from_le_bytes(range(bytes, at, 2)?.try_into().unwrap());
    if header & 0x8000 == 0 && header != 2 && header & 3 != 3 {
        return Err(Error::InvalidLayout);
    }
    Ok(header)
}

/// A lazy view retaining the entire buffer, including metadata before a message.
#[derive(Clone, Copy, Debug)]
pub struct Message<'a> {
    bytes: &'a [u8],
    // All actual objects follow the root prefix; zero denotes the default view.
    at: u32,
    header: Result<u16>,
    meta: &'static MessageMeta,
}

impl<'a> Message<'a> {
    /// Select the schema used by subsequent checked accesses. Generated getters
    /// pass their constant metadata so the optimizer can specialize dispatch.
    #[doc(hidden)]
    #[inline]
    pub fn with_meta(self, meta: &'static MessageMeta) -> Self {
        Self { meta, ..self }
    }

    #[inline]
    pub fn read(bytes: &'a [u8], meta: &'static MessageMeta) -> Result<Self> {
        Self::at(bytes, root(bytes)?, meta)
    }

    #[inline]
    pub fn default_for(meta: &'static MessageMeta) -> Self {
        Self {
            bytes: &[],
            at: 0,
            header: Ok(0x8000),
            meta,
        }
    }

    #[inline]
    fn at(bytes: &'a [u8], at: usize, meta: &'static MessageMeta) -> Result<Self> {
        if at == 0 {
            return Err(Error::InvalidOffset);
        }
        let value = Self {
            bytes,
            at: u32::try_from(at).map_err(|_| Error::SizeLimit)?,
            // Fixed messages have no header. Cache the bytes independently so
            // rebinding metadata through with_meta remains checked as before.
            header: read_header(bytes, at),
            meta,
        };
        if meta.layout == Layout::Fixed {
            range(bytes, at, meta.fixed_size())?;
        } else {
            value.header()?;
        }
        Ok(value)
    }

    #[inline]
    fn header(&self) -> Result<u16> {
        let header = self.header?;
        let flat = header & 0x8000 != 0;
        match self.meta.layout {
            Layout::Flat if !flat => return Err(Error::InvalidLayout),
            Layout::Sparse if flat => return Err(Error::InvalidLayout),
            Layout::Fixed => return Err(Error::InvalidLayout),
            _ => (),
        }
        Ok(header)
    }

    #[inline]
    fn meta_bits(&self, bit: usize, width: usize) -> Result<u8> {
        let at = self.at as usize;
        let mut result = 0;
        for i in 0..width {
            let index = bit.checked_add(i).ok_or(Error::InvalidOffset)?;
            let pos = at.checked_sub(index / 8 + 1).ok_or(Error::InvalidOffset)?;
            result |= ((range(self.bytes, pos, 1)?[0] >> (index % 8)) & 1) << i;
        }
        Ok(result)
    }

    // Generated calls have constant schema/field metadata. Inlining this
    // dispatch eliminates unused layouts and gap-correction paths.
    #[inline(always)]
    fn location(&self, id: u16, width: usize) -> Result<Option<usize>> {
        if id == 0 || id > MAX_FIELD_ID {
            return Err(Error::InvalidField);
        }
        if self.at == 0 {
            return Ok(None);
        }
        let at = self.at as usize;
        if self.meta.layout == Layout::Fixed {
            let offset = self.meta.field(id, width)?;
            let pos = add(at, offset)?;
            range(self.bytes, pos, width)?;
            return Ok(Some(pos));
        }
        let header = self.header()?;
        if id >= (header & 0x7fff) >> 2 {
            return Ok(None);
        }
        let pos = if header & 0x8000 != 0 {
            let mut offset = self.meta.field(id, width)?;
            for deleted in self.meta.deleted.iter().copied().filter(|&d| d < id) {
                if deleted == 0 || header & 2 == 0 {
                    return Err(Error::InvalidLayout);
                }
                let explicit = usize::from(header & 1);
                let size =
                    self.meta_bits((usize::from(deleted) - 1) * (2 + explicit) + explicit, 2)?;
                offset = add(offset, [0, 1, 4, 8][usize::from(size)])?;
            }
            add(add(at, 2)?, offset)?
        } else {
            let pointer = at.checked_sub(4).ok_or(Error::InvalidOffset)?;
            let delta = number::<i32>(self.bytes, pointer)?;
            let meta = if delta < 0 {
                at.checked_sub(delta.unsigned_abs() as usize)
                    .ok_or(Error::InvalidOffset)?
            } else {
                add(at, delta as usize)?
            };
            let offset = if id < 32 {
                let loc = meta
                    .checked_sub(usize::from(id))
                    .ok_or(Error::InvalidOffset)?;
                usize::from(range(self.bytes, loc, 1)?[0])
            } else {
                let loc = meta
                    .checked_sub(usize::from(id) * 2 - 31)
                    .ok_or(Error::InvalidOffset)?;
                usize::from(u16::from_le_bytes(
                    range(self.bytes, loc, 2)?.try_into().unwrap(),
                ))
            };
            if offset == 0 {
                return Ok(None);
            }
            if offset < 2 {
                return Err(Error::InvalidOffset);
            }
            add(at, offset)?
        };
        range(self.bytes, pos, width)?;
        Ok(Some(pos))
    }

    #[inline(always)]
    pub fn scalar<T: Scalar>(&self, id: u16, default: T) -> Result<T> {
        let Some(pos) = self.location(id, T::WIDTH)? else {
            return Ok(default);
        };
        let value = number::<T>(self.bytes, pos)?;
        if self.meta.layout == Layout::Fixed || self.header()? & 0x8000 != 0 {
            Ok(value.xor(default))
        } else {
            Ok(value)
        }
    }

    /// Presence of a scalar (`width = T::WIDTH`) or reference (`width = 4`).
    #[inline(always)]
    pub fn has(&self, id: u16, width: usize) -> Result<bool> {
        if !matches!(width, 1 | 4 | 8) {
            return Err(Error::InvalidField);
        }
        let Some(pos) = self.location(id, width)? else {
            return Ok(false);
        };
        if self.meta.layout != Layout::Fixed {
            let header = self.header()?;
            if header & 0x8000 == 0 {
                return Ok(true);
            }
            if header & 1 != 0 {
                let stride = if header & 2 != 0 { 3 } else { 1 };
                return Ok(self.meta_bits((usize::from(id) - 1) * stride, 1)? != 0);
            }
        }
        Ok(range(self.bytes, pos, width)?.iter().any(|&b| b != 0))
    }

    #[inline(always)]
    fn target(&self, id: u16) -> Result<Option<usize>> {
        let Some(pos) = self.location(id, 4)? else {
            return Ok(None);
        };
        let offset = number::<u32>(self.bytes, pos)? as usize;
        if offset == 0 {
            if self.meta.layout != Layout::Fixed && self.header()? & 0x8000 == 0 {
                return Err(Error::InvalidOffset);
            }
            return Ok(None);
        }
        if offset > MAX_OFFSET {
            return Err(Error::InvalidOffset);
        }
        Ok(Some(add(self.at as usize, offset)?))
    }

    #[inline]
    pub fn bytes(&self, id: u16, default: &'a [u8]) -> Result<&'a [u8]> {
        match self.target(id)? {
            Some(at) => string(self.bytes, at),
            None => Ok(default),
        }
    }

    #[inline]
    pub fn string(&self, id: u16, default: &'a [u8]) -> Result<&'a str> {
        std::str::from_utf8(self.bytes(id, default)?).map_err(|_| Error::InvalidUtf8)
    }

    #[inline(always)]
    pub fn message(&self, id: u16, meta: &'static MessageMeta) -> Result<Self> {
        match self.target(id)? {
            Some(at) => Self::at(self.bytes, at, meta),
            None => Ok(Self::default_for(meta)),
        }
    }

    #[inline(always)]
    pub fn array(&self, id: u16) -> Result<Array<'a>> {
        match self.target(id)? {
            Some(at) => Array::at(self.bytes, at),
            None => Ok(Array {
                bytes: self.bytes,
                data: 0,
                len: 0,
            }),
        }
    }
}

/// Common interface implemented by generated message views.
pub trait View<'a>: Copy {
    const META: &'static MessageMeta;
    fn from_message(message: Message<'a>) -> Self;
    fn message(&self) -> Message<'a>;

    #[inline]
    fn read(bytes: &'a [u8]) -> Result<Self> {
        Ok(Self::from_message(Message::read(bytes, Self::META)?))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Array<'a> {
    bytes: &'a [u8],
    data: usize,
    len: usize,
}

impl<'a> Array<'a> {
    /// Read a buffer finished with an array as its root object.
    #[inline]
    pub fn read(bytes: &'a [u8]) -> Result<Self> {
        Self::at(bytes, root(bytes)?)
    }

    #[inline]
    fn at(bytes: &'a [u8], at: usize) -> Result<Self> {
        let len = number::<u32>(bytes, at)? as usize;
        Ok(Self {
            bytes,
            data: add(at, 4)?,
            len,
        })
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Validate the complete inline span before iteration or materialization.
    #[inline]
    pub fn check_width(&self, width: usize) -> Result<()> {
        if width == 0 && self.len != 0 {
            return Err(Error::InvalidLayout);
        }
        let size = self.len.checked_mul(width).ok_or(Error::OutOfBounds)?;
        range(self.bytes, self.data, size)?;
        Ok(())
    }

    #[inline]
    fn element(&self, index: usize, width: usize) -> Result<usize> {
        if index >= self.len {
            return Err(Error::OutOfBounds);
        }
        let offset = index.checked_mul(width).ok_or(Error::OutOfBounds)?;
        let at = add(self.data, offset)?;
        range(self.bytes, at, width)?;
        Ok(at)
    }

    #[inline]
    fn target(&self, index: usize) -> Result<usize> {
        let offset = number::<u32>(self.bytes, self.element(index, 4)?)? as usize;
        if offset == 0 || offset > MAX_OFFSET {
            return Err(Error::InvalidOffset);
        }
        add(self.data, offset)
    }

    #[inline]
    pub fn scalar<T: Scalar>(&self, index: usize) -> Result<T> {
        number(self.bytes, self.element(index, T::WIDTH)?)
    }

    #[inline]
    pub fn bytes(&self, index: usize) -> Result<&'a [u8]> {
        string(self.bytes, self.target(index)?)
    }

    #[inline]
    pub fn message(&self, index: usize, meta: &'static MessageMeta) -> Result<Message<'a>> {
        let at = if meta.layout == Layout::Fixed {
            self.element(index, meta.fixed_size())?
        } else {
            self.target(index)?
        };
        Message::at(self.bytes, at, meta)
    }

    #[inline]
    pub fn scalars<T: Scalar>(self) -> ScalarArray<'a, T> {
        ScalarArray(self, PhantomData)
    }
    #[inline]
    pub fn messages<T: View<'a>>(self) -> MessageArray<'a, T> {
        MessageArray(self, PhantomData)
    }
    #[inline]
    pub fn strings(self) -> BytesArray<'a> {
        BytesArray(self)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ScalarArray<'a, T>(Array<'a>, PhantomData<T>);

impl<'a, T: Scalar> ScalarArray<'a, T> {
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    #[inline]
    pub fn get(&self, index: usize) -> Result<T> {
        self.0.scalar(index)
    }
    #[inline]
    pub fn iter(self) -> impl ExactSizeIterator<Item = Result<T>> + 'a {
        (0..self.len()).map(move |i| self.get(i))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MessageArray<'a, T>(Array<'a>, PhantomData<T>);

impl<'a, T: View<'a> + 'a> MessageArray<'a, T> {
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    #[inline]
    pub fn get(&self, index: usize) -> Result<T> {
        Ok(T::from_message(self.0.message(index, T::META)?))
    }
    #[inline]
    pub fn iter(self) -> impl ExactSizeIterator<Item = Result<T>> + 'a {
        (0..self.len()).map(move |i| self.get(i))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BytesArray<'a>(Array<'a>);

impl<'a> BytesArray<'a> {
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    #[inline]
    pub fn get(&self, index: usize) -> Result<&'a [u8]> {
        self.0.bytes(index)
    }
    #[inline]
    pub fn get_str(&self, index: usize) -> Result<&'a str> {
        std::str::from_utf8(self.get(index)?).map_err(|_| Error::InvalidUtf8)
    }
    #[inline]
    pub fn iter(self) -> impl ExactSizeIterator<Item = Result<&'a [u8]>> + 'a {
        (0..self.len()).map(move |i| self.get(i))
    }
}
