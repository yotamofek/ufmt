use core::mem::MaybeUninit;
use core::ops::Range;

use crate::{uDebug, uDisplay, uWrite, Formatter};

use super::uxx::{buf_to_str, write_loop};

macro_rules! ixx {
    ($uxx:ident, $n:expr, $buf:expr) => {{
        let n = $n;
        let negative = n.is_negative();

        let mut n = if negative {
            match n.checked_abs() {
                Some(n) => n as $uxx,
                None => std::$uxx::MAX / 2 + 1,
            }
        } else {
            n as $uxx
        };

        write_loop!(n, end, at, $buf);

        if negative {
            unsafe {
                at = at.sub(1);
                (*at).write(b'-');
            }
        }

        unsafe { buf_to_str(end, at) }
    }};
}

macro_rules! impl_ixx {
    ($ty:ident, $uty:ident, $buf_len:expr) => {
        impl uDebug for $ty {
            #[inline]
            fn fmt<W>(&self, f: &mut Formatter<'_, W>) -> Result<(), W::Error>
            where
                W: uWrite + ?Sized,
            {
                f.write_str(ixx!($uty, *self, [MaybeUninit::uninit(); $buf_len]))
            }
        }

        impl uDisplay for $ty {
            #[inline(always)]
            fn fmt<W>(&self, f: &mut Formatter<'_, W>) -> Result<(), W::Error>
            where
                W: uWrite + ?Sized,
            {
                <$ty as uDebug>::fmt(self, f)
            }
        }
    };
}

impl_ixx!(i8, u8, 4);
impl_ixx!(i16, u16, 6);
impl_ixx!(i32, u32, 11);
impl_ixx!(i64, u64, 20);
impl_ixx!(i128, u128, 40);

#[cfg(target_pointer_width = "16")]
impl_ixx!(isize, u16, 6);
#[cfg(target_pointer_width = "32")]
impl_ixx!(isize, u32, 11);
#[cfg(target_pointer_width = "64")]
impl_ixx!(isize, u64, 20);
