use crate::{Error, Result};

mod sealed {
    pub trait Sealed {}
}

/// The fixed-width scalar types supported by YaFF. Enums use `i32`.
/// Sealed to keep the wire representation independent of user implementations.
pub trait Scalar: sealed::Sealed + Copy + std::fmt::Debug + 'static {
    const WIDTH: usize;
    #[doc(hidden)]
    fn decode(bytes: &[u8]) -> Result<Self>;
    #[doc(hidden)]
    fn encode(self, bytes: &mut [u8]);
    #[doc(hidden)]
    fn xor(self, default: Self) -> Self;
    #[doc(hidden)]
    fn equal(self, other: Self) -> bool;
}

macro_rules! integer {
    ($($ty:ty),*) => {$ (
        impl sealed::Sealed for $ty {}
        impl Scalar for $ty {
            const WIDTH: usize = std::mem::size_of::<Self>();
            #[inline]
            fn decode(bytes: &[u8]) -> Result<Self> {
                Ok(Self::from_le_bytes(bytes.try_into().map_err(|_| Error::OutOfBounds)?))
            }
            #[inline]
            fn encode(self, bytes: &mut [u8]) { bytes.copy_from_slice(&self.to_le_bytes()); }
            #[inline]
            fn xor(self, default: Self) -> Self { self ^ default }
            #[inline]
            fn equal(self, other: Self) -> bool { self == other }
        }
    )*};
}
integer!(i32, u32, i64, u64);

impl sealed::Sealed for bool {}
impl Scalar for bool {
    const WIDTH: usize = 1;
    #[inline]
    fn decode(bytes: &[u8]) -> Result<Self> {
        match bytes {
            [0] => Ok(false),
            [1] => Ok(true),
            [_] => Err(Error::InvalidBoolean),
            _ => Err(Error::OutOfBounds),
        }
    }
    #[inline]
    fn encode(self, bytes: &mut [u8]) {
        bytes[0] = u8::from(self);
    }
    #[inline]
    fn xor(self, default: Self) -> Self {
        self ^ default
    }
    #[inline]
    fn equal(self, other: Self) -> bool {
        self == other
    }
}

macro_rules! float {
    ($ty:ty, $bits:ty) => {
        impl sealed::Sealed for $ty {}
        impl Scalar for $ty {
            const WIDTH: usize = std::mem::size_of::<Self>();
            #[inline]
            fn decode(bytes: &[u8]) -> Result<Self> {
                Ok(Self::from_bits(<$bits>::decode(bytes)?))
            }
            #[inline]
            fn encode(self, bytes: &mut [u8]) {
                self.to_bits().encode(bytes);
            }
            #[inline]
            fn xor(self, default: Self) -> Self {
                Self::from_bits(self.to_bits() ^ default.to_bits())
            }
            #[inline]
            fn equal(self, other: Self) -> bool {
                // Match yaff::IsEqual, including its treatment of signed zero,
                // nearby defaults, NaNs and infinities.
                (self - other).abs() <= self.abs().min(other.abs()) * Self::EPSILON
            }
        }
    };
}
float!(f32, u32);
float!(f64, u64);
