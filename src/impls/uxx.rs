use core::mem::MaybeUninit;
use core::ops::Range;
use core::slice;
use core::str;

use crate::{uDebug, uDisplay, uWrite, Formatter};

macro_rules! write_loop {
    ($n:ident, $end:ident, $at:ident, $buf:expr) => {
        let Range { $end, .. } = $buf.as_mut_ptr_range();
        let mut $at = $end;

        loop {
            unsafe {
                $at = $at.sub(1);
                (*$at).write(($n % 10) as u8 + b'0');
            }
            $n /= 10;

            if $n == 0 {
                break;
            }
        }
    };
}
pub(super) use write_loop;

pub(super) unsafe fn buf_to_str<'s>(
    end: *mut MaybeUninit<u8>,
    at: *mut MaybeUninit<u8>,
) -> &'s str {
    let len = end.offset_from(at) as usize;
    let bytes: &[u8] = slice::from_raw_parts(at as *const _, len);
    str::from_utf8_unchecked(bytes)
}

macro_rules! uxx {
    ($n:expr, $buf:expr) => {{
        let mut n = $n;

        write_loop!(n, end, at, $buf);

        unsafe {
            let len = end.offset_from(at) as usize;
            let bytes: &[u8] = slice::from_raw_parts(at as *const _, len);
            str::from_utf8_unchecked(bytes)
        }
    }};
}

macro_rules! impl_uxx {
    ($ty:ident, $buf_len:expr) => {
        impl uDebug for $ty {
            #[inline]
            fn fmt<W>(&self, f: &mut Formatter<'_, W>) -> Result<(), W::Error>
            where
                W: uWrite + ?Sized,
            {
                f.write_str(uxx!(*self, [MaybeUninit::uninit(); $buf_len]))
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

impl_uxx!(u8, 3);
impl_uxx!(u16, 5);
impl_uxx!(u32, 10);
impl_uxx!(u64, 20);
impl_uxx!(u128, 39);

#[cfg(target_pointer_width = "16")]
impl_uxx!(usize, 5);
#[cfg(target_pointer_width = "32")]
impl_uxx!(usize, 10);
#[cfg(target_pointer_width = "64")]
impl_uxx!(usize, 20);
