//! Compact views with cached headers for trusted, immutable YaFF data.
//!
//! Prefer generated `Foo::read_trusted` entry points. Their caller must guarantee
//! that the entire reachable graph is valid for the generated schema and remains
//! immutable for the borrow's lifetime. Merely constructing a checked lazy view
//! does not establish that guarantee. No verifier is implied by this API.
//!
//! All pointer operations live in this module. Generated accessors supply schema
//! constants; bounds-checked readers remain the default API.

use crate::{Error, Layout, MessageMeta, OpenEnum, Result, Scalar};
use std::{marker::PhantomData, ptr::NonNull, slice};

static EMPTY_FLAT: [u8; 2] = [0, 0x80];
static EMPTY_SPARSE: [u8; 2] = [2, 0];
static EMPTY_ARRAY: [u8; 4] = [0; 4];
// Any supported fixed schema fits in a u16-sized inline region.
static EMPTY_FIXED: [u8; 65536] = [0; 65536];

#[inline(always)]
unsafe fn u16_at(ptr: *const u8) -> u16 {
    u16::from_le(unsafe { ptr.cast::<u16>().read_unaligned() })
}
#[inline(always)]
unsafe fn u32_at(ptr: *const u8) -> u32 {
    u32::from_le(unsafe { ptr.cast::<u32>().read_unaligned() })
}
#[inline(always)]
unsafe fn scalar_at<T: Scalar>(ptr: *const u8) -> T {
    // The entry-point contract guarantees a valid representation and a readable
    // span of T::WIDTH bytes. Scalar is sealed to the wire primitive types.
    unsafe { T::decode(slice::from_raw_parts(ptr, T::WIDTH)).unwrap_unchecked() }
}
#[inline(always)]
unsafe fn string_at<'a>(ptr: *const u8) -> &'a [u8] {
    unsafe { slice::from_raw_parts(ptr.wrapping_add(4), u32_at(ptr) as usize) }
}

/// Low-level storage used by generated trusted views.
#[derive(Clone, Copy, Debug)]
pub struct Message<'a> {
    ptr: NonNull<u8>,
    header: u16,
    lifetime: PhantomData<&'a [u8]>,
}

// Constructors require immutable readable storage for 'a. All operations only
// read that storage, so sharing a view has the same requirements as &'a [u8].
unsafe impl Send for Message<'_> {}
unsafe impl Sync for Message<'_> {}

impl<'a> Message<'a> {
    /// # Safety
    /// `bytes` must be a complete, valid YaFF buffer for the schema used by all
    /// subsequent accesses, including all referenced objects and primitive values.
    /// The buffer must remain immutable for 'a.
    #[inline]
    pub unsafe fn read_root(bytes: &'a [u8], layout: Layout) -> Self {
        let ptr = bytes.as_ptr();
        unsafe { Self::from_ptr(ptr.wrapping_add(u32_at(ptr) as usize), layout) }
    }

    /// # Safety
    /// `ptr` must point to a message in the supplied layout, valid for subsequent
    /// schema accesses. Its complete graph must lie in one readable, immutable
    /// allocation for 'a (as in a normal serialized buffer).
    #[inline]
    pub unsafe fn from_ptr(ptr: *const u8, layout: Layout) -> Self {
        let header = if layout == Layout::Fixed {
            0
        } else {
            unsafe { u16_at(ptr) }
        };
        unsafe { Self::from_parts(ptr, header) }
    }

    /// # Safety
    /// In addition to from_ptr's contract, header must be the actual immutable
    /// wire header at ptr, or zero for a fixed message.
    #[doc(hidden)]
    #[inline]
    pub unsafe fn from_parts(ptr: *const u8, header: u16) -> Self {
        Self {
            ptr: unsafe { NonNull::new_unchecked(ptr.cast_mut()) },
            header,
            lifetime: PhantomData,
        }
    }

    #[doc(hidden)]
    #[inline]
    pub fn header(self) -> u16 {
        self.header
    }

    #[inline]
    pub fn as_ptr(self) -> *const u8 {
        self.ptr.as_ptr()
    }

    #[inline]
    pub fn default_for(layout: Layout) -> Self {
        let ptr = match layout {
            Layout::Fixed => EMPTY_FIXED.as_ptr(),
            Layout::Sparse => EMPTY_SPARSE.as_ptr(),
            _ => EMPTY_FLAT.as_ptr(),
        };
        // These static buffers represent absence for any matching layout.
        unsafe { Self::from_ptr(ptr, layout) }
    }

    #[inline(always)]
    unsafe fn bits(self, index: usize, width: usize) -> u8 {
        let mut value = 0;
        for i in 0..width {
            let bit = index + i;
            // Read exactly the required bytes, including corrections crossing a
            // byte boundary. Do not read extra bytes before the validated span.
            let byte = unsafe { self.as_ptr().wrapping_sub(bit / 8 + 1).read() };
            value |= ((byte >> (bit & 7)) & 1) << i;
        }
        value
    }

    #[inline(always)]
    unsafe fn flat(self, id: u16, offset: u16, header: u16, deleted: &[u16]) -> *const u8 {
        let explicit = usize::from(header & 1);
        let mut offset = usize::from(offset) + 2;
        for &gap in deleted.iter().filter(|&&gap| gap < id) {
            let code = unsafe { self.bits((usize::from(gap) - 1) * (2 + explicit) + explicit, 2) };
            offset += [0, 1, 4, 8][usize::from(code)];
        }
        self.as_ptr().wrapping_add(offset)
    }

    #[inline(always)]
    unsafe fn sparse(self, id: u16, header: u16) -> Option<*const u8> {
        if (id << 2) | 3 >= header {
            return None;
        }
        let delta = unsafe { u32_at(self.as_ptr().wrapping_sub(4)) } as i32;
        let meta = self.as_ptr().wrapping_offset(delta as isize);
        let offset = if id < 32 {
            unsafe { meta.wrapping_sub(usize::from(id)).read() as usize }
        } else {
            unsafe { u16_at(meta.wrapping_sub(usize::from(id) * 2 - 31)) as usize }
        };
        (offset != 0).then(|| self.as_ptr().wrapping_add(offset))
    }

    #[inline(always)]
    unsafe fn slot(self, id: u16, offset: u16, meta: &MessageMeta) -> Option<(*const u8, bool)> {
        if meta.layout == Layout::Fixed {
            return Some((self.as_ptr().wrapping_add(usize::from(offset)), true));
        }
        let header = self.header;
        if meta.layout == Layout::Flat {
            return (((id << 2) | 0x8003) < header)
                .then(|| (unsafe { self.flat(id, offset, header, meta.deleted) }, true));
        }
        if meta.layout == Layout::Sparse {
            return unsafe { self.sparse(id, header) }.map(|ptr| (ptr, false));
        }
        if ((id << 2) | 0x8003) < header {
            return Some((unsafe { self.flat(id, offset, header, meta.deleted) }, true));
        }
        if header & 0x8000 != 0 {
            return None;
        }
        unsafe { self.sparse(id, header) }.map(|ptr| (ptr, false))
    }

    /// # Safety
    /// The ID, inline offset, metadata and scalar type must match this message's
    /// schema. The message must satisfy the constructor's validity contract.
    #[inline(always)]
    pub unsafe fn scalar<T: Scalar>(
        self,
        id: u16,
        offset: u16,
        meta: &MessageMeta,
        default: T,
    ) -> T {
        match unsafe { self.slot(id, offset, meta) } {
            Some((ptr, xor)) => {
                let value = unsafe { scalar_at::<T>(ptr) };
                if xor {
                    value.xor(default)
                } else {
                    value
                }
            }
            None => default,
        }
    }

    /// # Safety
    /// The field ID, offset, width (1, 4 or 8) and metadata must match the schema.
    #[inline(always)]
    pub unsafe fn has(self, id: u16, offset: u16, width: usize, meta: &MessageMeta) -> bool {
        let Some((ptr, flat)) = (unsafe { self.slot(id, offset, meta) }) else {
            return false;
        };
        if !flat {
            return true;
        }
        if meta.layout != Layout::Fixed {
            let header = self.header;
            if header & 1 != 0 {
                let stride = if header & 2 != 0 { 3 } else { 1 };
                return unsafe { self.bits((usize::from(id) - 1) * stride, 1) } != 0;
            }
        }
        match width {
            1 => unsafe { ptr.read() != 0 },
            4 => unsafe { u32_at(ptr) != 0 },
            8 => unsafe { scalar_at::<u64>(ptr) != 0 },
            _ => unreachable!("unsupported wire width"),
        }
    }

    #[inline(always)]
    unsafe fn target(self, id: u16, offset: u16, meta: &MessageMeta) -> Option<*const u8> {
        let (ptr, _) = unsafe { self.slot(id, offset, meta) }?;
        let offset = unsafe { u32_at(ptr) } as usize;
        (offset != 0).then(|| self.as_ptr().wrapping_add(offset))
    }

    /// # Safety
    /// The field must be a string/bytes field with the supplied schema position.
    #[inline(always)]
    pub unsafe fn bytes(
        self,
        id: u16,
        offset: u16,
        meta: &MessageMeta,
        default: &'a [u8],
    ) -> &'a [u8] {
        match unsafe { self.target(id, offset, meta) } {
            Some(ptr) => unsafe { string_at(ptr) },
            None => default,
        }
    }

    /// # Safety
    /// The field and child schema must match, and the entire child graph must be valid.
    #[inline(always)]
    pub unsafe fn message(
        self,
        id: u16,
        offset: u16,
        meta: &MessageMeta,
        child: &MessageMeta,
    ) -> Self {
        match unsafe { self.target(id, offset, meta) } {
            Some(ptr) => unsafe { Self::from_ptr(ptr, child.layout) },
            None => Self::default_for(child.layout),
        }
    }

    /// # Safety
    /// The field must be an array with the supplied schema position, including valid elements.
    #[inline(always)]
    pub unsafe fn array(self, id: u16, offset: u16, meta: &MessageMeta) -> Array<'a> {
        let ptr = unsafe { self.target(id, offset, meta) }.unwrap_or(EMPTY_ARRAY.as_ptr());
        Array {
            ptr: unsafe { NonNull::new_unchecked(ptr.cast_mut()) },
            lifetime: PhantomData,
        }
    }
}

/// Implemented by generated trusted views. Construction is unsafe because the
/// low-level pointer does not carry a schema or a validity proof.
pub trait View<'a>: Copy {
    const META: &'static MessageMeta;
    /// # Safety
    /// The message must be valid for Self's schema for its entire lifetime.
    unsafe fn from_message(message: Message<'a>) -> Self;
    fn message(self) -> Message<'a>;
}

#[derive(Clone, Copy, Debug)]
pub struct Array<'a> {
    ptr: NonNull<u8>,
    lifetime: PhantomData<&'a [u8]>,
}
// Same immutable-storage invariant as Message.
unsafe impl Send for Array<'_> {}
unsafe impl Sync for Array<'_> {}

impl<'a> Array<'a> {
    #[inline(always)]
    pub fn len(self) -> usize {
        (unsafe { u32_at(self.ptr.as_ptr()) }) as usize
    }
    #[inline]
    pub fn is_empty(self) -> bool {
        self.len() == 0
    }
    #[inline(always)]
    fn data(self) -> *const u8 {
        self.ptr.as_ptr().wrapping_add(4)
    }
    #[inline(always)]
    unsafe fn target(self, index: usize) -> *const u8 {
        let offset = unsafe { u32_at(self.data().wrapping_add(index * 4)) } as usize;
        self.data().wrapping_add(offset)
    }
    /// # Safety
    /// Every element must have scalar type T and a valid representation.
    #[inline]
    pub unsafe fn scalars<T: Scalar>(self) -> ScalarArray<'a, T> {
        ScalarArray(self, PhantomData)
    }
    /// # Safety
    /// The element representation and complete graphs must match T's schema.
    #[inline]
    pub unsafe fn messages<T: View<'a>>(self) -> MessageArray<'a, T> {
        MessageArray(self, PhantomData)
    }
    /// # Safety
    /// Every element must be a valid string/bytes reference.
    #[inline]
    pub unsafe fn strings(self) -> BytesArray<'a> {
        BytesArray(self)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ScalarArray<'a, T>(Array<'a>, PhantomData<T>);
impl<'a, T: Scalar> ScalarArray<'a, T> {
    #[inline]
    pub fn len(self) -> usize {
        self.0.len()
    }
    #[inline]
    pub fn is_empty(self) -> bool {
        self.0.is_empty()
    }
    #[inline(always)]
    pub fn get(self, index: usize) -> Result<T> {
        if index >= self.len() {
            return Err(Error::OutOfBounds);
        }
        Ok(unsafe { self.get_unchecked(index) })
    }
    /// # Safety
    /// `index` must be smaller than `len()`.
    #[inline(always)]
    pub unsafe fn get_unchecked(self, index: usize) -> T {
        unsafe { scalar_at(self.0.data().wrapping_add(index * T::WIDTH)) }
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = Result<T>> + 'a {
        (0..self.len()).map(move |i| Ok(unsafe { self.get_unchecked(i) }))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MessageArray<'a, T>(Array<'a>, PhantomData<T>);
impl<'a, T: View<'a> + 'a> MessageArray<'a, T> {
    #[inline]
    pub fn len(self) -> usize {
        self.0.len()
    }
    #[inline]
    pub fn is_empty(self) -> bool {
        self.0.is_empty()
    }
    #[inline(always)]
    pub fn get(self, index: usize) -> Result<T> {
        if index >= self.len() {
            return Err(Error::OutOfBounds);
        }
        Ok(unsafe { self.get_unchecked(index) })
    }
    /// # Safety
    /// `index` must be smaller than `len()`.
    #[inline(always)]
    pub unsafe fn get_unchecked(self, index: usize) -> T {
        let ptr = if T::META.layout == Layout::Fixed {
            self.0.data().wrapping_add(index * T::META.fixed_size())
        } else {
            unsafe { self.0.target(index) }
        };
        unsafe { T::from_message(Message::from_ptr(ptr, T::META.layout)) }
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = Result<T>> + 'a {
        (0..self.len()).map(move |i| Ok(unsafe { self.get_unchecked(i) }))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BytesArray<'a>(Array<'a>);
impl<'a> BytesArray<'a> {
    #[inline]
    pub fn len(self) -> usize {
        self.0.len()
    }
    #[inline]
    pub fn is_empty(self) -> bool {
        self.0.is_empty()
    }
    #[inline(always)]
    pub fn get(self, index: usize) -> Result<&'a [u8]> {
        if index >= self.len() {
            return Err(Error::OutOfBounds);
        }
        Ok(unsafe { string_at(self.0.target(index)) })
    }
    pub fn get_str(self, index: usize) -> Result<&'a str> {
        std::str::from_utf8(self.get(index)?).map_err(|_| Error::InvalidUtf8)
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = Result<&'a [u8]>> + 'a {
        (0..self.len()).map(move |i| self.get(i))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EnumArray<'a, E>(ScalarArray<'a, i32>, PhantomData<E>);
impl<'a, E: OpenEnum + 'a> EnumArray<'a, E> {
    /// # Safety
    /// The array must contain i32 enum elements.
    pub unsafe fn new(array: Array<'a>) -> Self {
        Self(unsafe { array.scalars() }, PhantomData)
    }
    #[inline]
    pub fn len(self) -> usize {
        self.0.len()
    }
    #[inline]
    pub fn is_empty(self) -> bool {
        self.0.is_empty()
    }
    #[inline]
    pub fn get(self, index: usize) -> Result<E> {
        Ok(E::from_raw(self.0.get(index)?))
    }
    pub fn iter(self) -> impl ExactSizeIterator<Item = Result<E>> + 'a {
        self.0.iter().map(|value| value.map(E::from_raw))
    }
}
