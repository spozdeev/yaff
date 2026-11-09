use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::{Error, Layout, MessageMeta, Result, Scalar, MAX_FIELD_ID, MAX_OFFSET};

/// Type marker for a string/bytes record.
#[derive(Debug)]
pub enum Bytes {}
/// Type marker for an array of wire elements.
#[derive(Debug)]
pub struct Sequence<T>(PhantomData<T>);

/// A typed reference to an object written by one particular writer.
/// Distances survive buffer growth. Foreign and stale handles are rejected.
#[derive(Debug)]
pub struct Offset<T> {
    distance: u32,
    owner: u64,
    marker: PhantomData<fn() -> T>,
}

impl<T> Copy for Offset<T> {}
impl<T> Clone for Offset<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Offset<T> {
    pub fn is_null(self) -> bool {
        self.distance == 0
    }
}

#[derive(Clone, Copy, Debug)]
enum Value {
    Scalar {
        value: [u8; 8],
        default: [u8; 8],
        width: usize,
        equal: bool,
    },
    Reference {
        distance: u32,
        owner: u64,
    },
}

/// One populated field. Omit absent fields entirely. The writer handles sorting.
#[derive(Clone, Copy, Debug)]
pub struct Field {
    id: u16,
    value: Value,
}

impl Field {
    pub fn scalar<T: Scalar>(id: u16, value: T, default: T) -> Self {
        let mut encoded = [0; 8];
        let mut encoded_default = [0; 8];
        value.encode(&mut encoded[..T::WIDTH]);
        default.encode(&mut encoded_default[..T::WIDTH]);
        Self {
            id,
            value: Value::Scalar {
                value: encoded,
                default: encoded_default,
                width: T::WIDTH,
                equal: value.equal(default),
            },
        }
    }

    pub fn reference<T>(id: u16, offset: Offset<T>) -> Self {
        Self {
            id,
            value: Value::Reference {
                distance: offset.distance,
                owner: offset.owner,
            },
        }
    }

    fn width(&self) -> usize {
        match self.value {
            Value::Scalar { width, .. } => width,
            Value::Reference { .. } => 4,
        }
    }

    fn is_default(&self) -> bool {
        matches!(self.value, Value::Scalar { equal: true, .. })
    }
}

static NEXT_OWNER: AtomicU64 = AtomicU64::new(1);
fn owner() -> u64 {
    let mut id = NEXT_OWNER.load(Ordering::Relaxed);
    loop {
        let next = id
            .checked_add(1)
            .expect("YaFF writer identity space exhausted");
        match NEXT_OWNER.compare_exchange_weak(id, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return id,
            Err(actual) => id = actual,
        }
    }
}

/// Builds the buffer back to front. The internal byte vector is reversed once
/// at finish, so appending objects and growing storage are amortized linear.
#[derive(Debug)]
pub struct Writer {
    reversed: Vec<u8>,
    owner: u64,
    alternative: Layout,
}

impl Default for Writer {
    fn default() -> Self {
        Self::new()
    }
}

impl Writer {
    pub fn new() -> Self {
        Self {
            reversed: Vec::new(),
            owner: owner(),
            alternative: Layout::Flat,
        }
    }

    /// Choose flat or sparse for dense dynamic messages. Gapped dynamic schemas
    /// always use sparse, matching C++ generation.
    pub fn set_dynamic_layout(&mut self, layout: Layout) -> Result<()> {
        if !matches!(layout, Layout::Flat | Layout::Sparse) {
            return Err(Error::InvalidLayout);
        }
        self.alternative = layout;
        Ok(())
    }

    pub fn clear(&mut self) {
        self.reversed.clear();
        self.owner = owner();
    }
    pub fn len(&self) -> usize {
        self.reversed.len()
    }
    pub fn is_empty(&self) -> bool {
        self.reversed.is_empty()
    }

    fn offset<T>(&self, distance: u32) -> Offset<T> {
        Offset {
            distance,
            owner: self.owner,
            marker: PhantomData,
        }
    }

    fn check_reference(&self, distance: u32, owner: u64) -> Result<()> {
        if owner != self.owner {
            return Err(Error::ForeignOffset);
        }
        if distance as usize > self.len() {
            return Err(Error::InvalidOffset);
        }
        Ok(())
    }

    fn check_size(&self, size: usize) -> Result<usize> {
        let total = self.len().checked_add(size).ok_or(Error::SizeLimit)?;
        // Reserve space for the root offset as well.
        if total > MAX_OFFSET - 4 {
            return Err(Error::SizeLimit);
        }
        Ok(total)
    }

    fn append<T>(&mut self, block: &[u8]) -> Result<Offset<T>> {
        let end = self.check_size(block.len())?;
        self.reversed.extend(block.iter().rev());
        Ok(self.offset(end as u32))
    }

    pub fn bytes(&mut self, value: &[u8]) -> Result<Offset<Bytes>> {
        let size = value.len().checked_add(5).ok_or(Error::SizeLimit)?;
        let end = self.check_size(size)?;
        self.reversed.push(0);
        self.reversed.extend(value.iter().rev());
        self.reversed
            .extend((value.len() as u32).to_le_bytes().iter().rev());
        Ok(self.offset(end as u32))
    }

    pub fn string(&mut self, value: &str) -> Result<Offset<Bytes>> {
        self.bytes(value.as_bytes())
    }

    /// Copy a previously written string for map-key ordering. Reading finished
    /// buffers uses borrowed views; this helper only runs during construction.
    pub fn bytes_at(&self, offset: Offset<Bytes>) -> Result<Vec<u8>> {
        self.check_reference(offset.distance, offset.owner)?;
        let start = offset.distance as usize;
        if start < 5 {
            return Err(Error::InvalidOffset);
        }
        let len =
            u32::from_le_bytes(std::array::from_fn(|i| self.reversed[start - 1 - i])) as usize;
        if len > start - 5 {
            return Err(Error::InvalidOffset);
        }
        Ok((0..len).map(|i| self.reversed[start - 5 - i]).collect())
    }

    pub fn scalar_array<T: Scalar>(&mut self, values: &[T]) -> Result<Offset<Sequence<T>>> {
        if values.is_empty() {
            return Ok(self.offset(0));
        }
        let size = values
            .len()
            .checked_mul(T::WIDTH)
            .and_then(|n| n.checked_add(4))
            .ok_or(Error::SizeLimit)?;
        let end = self.check_size(size)?;
        let mut encoded = [0; 8];
        for value in values.iter().rev() {
            value.encode(&mut encoded[..T::WIDTH]);
            self.reversed.extend(encoded[..T::WIDTH].iter().rev());
        }
        self.reversed
            .extend((values.len() as u32).to_le_bytes().iter().rev());
        Ok(self.offset(end as u32))
    }

    /// Array of strings or non-fixed messages. For fixed messages use
    /// `fixed_array` (generated fixed builders expose `write_array`).
    pub fn offset_array<T>(&mut self, values: &[Offset<T>]) -> Result<Offset<Sequence<T>>> {
        if values.is_empty() {
            return Ok(self.offset(0));
        }
        let size = values
            .len()
            .checked_mul(4)
            .and_then(|n| n.checked_add(4))
            .ok_or(Error::SizeLimit)?;
        let end = self.check_size(size)?;
        for value in values {
            self.check_reference(value.distance, value.owner)?;
            if value.is_null() {
                return Err(Error::InvalidOffset);
            }
        }
        let base = end - 4;
        for value in values.iter().rev() {
            self.reversed.extend(
                ((base - value.distance as usize) as u32)
                    .to_le_bytes()
                    .iter()
                    .rev(),
            );
        }
        self.reversed
            .extend((values.len() as u32).to_le_bytes().iter().rev());
        Ok(self.offset(end as u32))
    }

    pub fn fixed_array<T>(
        &mut self,
        meta: &MessageMeta,
        elements: &[Vec<Field>],
    ) -> Result<Offset<Sequence<T>>> {
        if meta.layout != Layout::Fixed {
            return Err(Error::InvalidLayout);
        }
        if elements.is_empty() {
            return Ok(self.offset(0));
        }
        if meta.fixed_size() == 0 {
            return Err(Error::InvalidLayout);
        }
        let size = elements
            .len()
            .checked_mul(meta.fixed_size())
            .and_then(|n| n.checked_add(4))
            .ok_or(Error::SizeLimit)?;
        self.check_size(size)?;
        if elements.len() > u32::MAX as usize {
            return Err(Error::SizeLimit);
        }
        let start = self.len();
        for fields in elements.iter().rev() {
            if let Err(error) = self.message::<T>(meta, fields) {
                self.reversed.truncate(start);
                return Err(error);
            }
        }
        self.append(&(elements.len() as u32).to_le_bytes())
    }

    pub fn message<T>(&mut self, meta: &MessageMeta, fields: &[Field]) -> Result<Offset<T>> {
        if meta.offsets.len() > usize::from(MAX_FIELD_ID) + 1
            || meta.offsets.first().is_some_and(|&v| v != 0)
            || meta
                .offsets
                .windows(2)
                .any(|p| !matches!(p[1].checked_sub(p[0]), Some(0 | 1 | 4 | 8)))
        {
            return Err(Error::InvalidField);
        }
        let layout = match meta.layout {
            Layout::Dynamic if !meta.deleted.is_empty() => Layout::Sparse,
            Layout::Dynamic => self.alternative,
            other => other,
        };
        if matches!(layout, Layout::Fixed | Layout::Flat) && !meta.deleted.is_empty() {
            return Err(Error::InvalidLayout);
        }
        let mut ordered = fields.to_vec();
        ordered.sort_by_key(|f| f.id);
        let mut previous = 0;
        for field in &ordered {
            if field.id == 0 || field.id > MAX_FIELD_ID {
                return Err(Error::InvalidField);
            }
            if previous == field.id {
                return Err(Error::DuplicateField);
            }
            previous = field.id;
            if layout != Layout::Sparse || !meta.offsets.is_empty() {
                meta.field(field.id, field.width())?;
            }
            if let Value::Reference { distance, owner } = field.value {
                self.check_reference(distance, owner)?;
            }
        }
        ordered.retain(|f| match f.value {
            Value::Reference { distance, .. } => distance != 0,
            Value::Scalar { equal, .. } => layout == Layout::Fixed || meta.explicit || !equal,
        });
        match layout {
            Layout::Fixed => self.fixed(meta, &ordered),
            Layout::Flat => self.flat(meta, &ordered),
            Layout::Sparse => self.sparse(&ordered),
            Layout::Dynamic => unreachable!(),
        }
    }

    fn write_field(
        block: &mut [u8],
        pos: usize,
        field: &Field,
        message_distance: usize,
        xor: bool,
    ) {
        match field.value {
            Value::Scalar {
                value,
                default,
                width,
                ..
            } => {
                for i in 0..width {
                    block[pos + i] = value[i] ^ if xor { default[i] } else { 0 };
                }
            }
            Value::Reference { distance, .. } => {
                block[pos..pos + 4].copy_from_slice(
                    &((message_distance - distance as usize) as u32).to_le_bytes(),
                );
            }
        }
    }

    fn fixed<T>(&mut self, meta: &MessageMeta, fields: &[Field]) -> Result<Offset<T>> {
        let size = meta.fixed_size();
        let end = self.check_size(size)?;
        let mut block = vec![0; size];
        for field in fields {
            let pos = meta.field(field.id, field.width())?;
            Self::write_field(&mut block, pos, field, end, true);
        }
        self.append(&block)
    }

    fn flat<T>(&mut self, meta: &MessageMeta, fields: &[Field]) -> Result<Offset<T>> {
        let Some(last) = fields.last() else {
            return self.append(&0x8000u16.to_le_bytes());
        };
        let highest = usize::from(last.id);
        let sized = meta.layout == Layout::Dynamic;
        let explicit = meta.explicit && fields.iter().any(Field::is_default);
        let stride = usize::from(explicit) + 2 * usize::from(sized);
        let meta_size = (highest * stride).div_ceil(8);
        let data_size = 2 + meta.field(last.id, last.width())? + last.width();
        self.check_size(data_size + meta_size)?;
        let end = self.len() + data_size;
        let mut block = vec![0; data_size + meta_size];
        let header = 0x8000 | ((last.id + 1) << 2) | (u16::from(sized) << 1) | u16::from(explicit);
        block[meta_size..meta_size + 2].copy_from_slice(&header.to_le_bytes());
        if sized {
            for i in 0..highest {
                let width = meta.offsets[i + 1]
                    .checked_sub(meta.offsets[i])
                    .ok_or(Error::InvalidField)?;
                let code = match width {
                    0 => 0,
                    1 => 1,
                    4 => 2,
                    8 => 3,
                    _ => return Err(Error::InvalidField),
                };
                for b in 0..2 {
                    let bit = i * stride + usize::from(explicit) + b;
                    block[meta_size - bit / 8 - 1] |= ((code >> b) & 1) << (bit % 8);
                }
            }
        }
        for field in fields {
            if explicit {
                let bit = (usize::from(field.id) - 1) * stride;
                block[meta_size - bit / 8 - 1] |= 1 << (bit % 8);
            }
            let pos = meta_size + 2 + meta.field(field.id, field.width())?;
            Self::write_field(&mut block, pos, field, end, true);
        }
        self.append::<T>(&block)?;
        Ok(self.offset(end as u32))
    }

    fn sparse<T>(&mut self, fields: &[Field]) -> Result<Offset<T>> {
        let Some(last) = fields.last() else {
            return self.append(&2u16.to_le_bytes());
        };
        let highest = usize::from(last.id);
        let meta_size = highest + highest.saturating_sub(31);
        let data_size = 2 + fields.iter().map(Field::width).sum::<usize>();
        if data_size > u16::MAX as usize {
            return Err(Error::SizeLimit);
        }
        self.check_size(meta_size + 4 + data_size)?;
        let end = self.len() + data_size;
        let start = meta_size + 4;
        let mut block = vec![0; start + data_size];
        block[meta_size..start].copy_from_slice(&(-4i32).to_le_bytes());
        block[start..start + 2].copy_from_slice(&(((last.id + 1) << 2) | 3).to_le_bytes());
        let mut pos = 2;
        for field in fields {
            let id = usize::from(field.id);
            if id < 32 {
                block[meta_size - id] = u8::try_from(pos).map_err(|_| Error::SizeLimit)?;
            } else {
                let index = meta_size - (id * 2 - 31);
                block[index..index + 2].copy_from_slice(&(pos as u16).to_le_bytes());
            }
            Self::write_field(&mut block, start + pos, field, end, false);
            pos += field.width();
        }
        self.append::<T>(&block)?;
        Ok(self.offset(end as u32))
    }

    pub fn finish<T>(mut self, root: Offset<T>) -> Result<Vec<u8>> {
        self.check_reference(root.distance, root.owner)?;
        if root.is_null() {
            return Err(Error::InvalidOffset);
        }
        let distance = self.len() + 4 - root.distance as usize;
        self.reversed
            .extend((distance as u32).to_le_bytes().iter().rev());
        self.reversed.reverse();
        Ok(self.reversed)
    }
}
