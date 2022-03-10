use core::{mem::MaybeUninit, str};

use crate::{uDebug, uWrite, Formatter};

macro_rules! hex {
    ($self:expr, $f:expr, $N:expr) => {{
        let mut buf: [MaybeUninit<u8>; $N] = [MaybeUninit::uninit(); $N];

        let i = hex(*$self as usize, &mut buf);

        unsafe {
            let bytes: &[u8] =
                &*(buf.get_unchecked(i..) as *const [MaybeUninit<u8>] as *const [u8]);
            $f.write_str(str::from_utf8_unchecked(bytes))
        }
    }};
}

fn hex(mut n: usize, buf: &mut [MaybeUninit<u8>]) -> usize {
    let mut i = buf.len() - 1;

    loop {
        let d = (n % 16) as u8;
        unsafe { buf.get_unchecked_mut(i) }.write(if d < 10 { d + b'0' } else { (d - 10) + b'a' });
        n /= 16;

        i -= 1;
        if n == 0 {
            break;
        }
    }

    unsafe { buf.get_unchecked_mut(i) }.write(b'x');
    i -= 1;

    unsafe { buf.get_unchecked_mut(i) }.write(b'0');

    i
}

impl<T> uDebug for *const T {
    #[cfg(target_pointer_width = "16")]
    fn fmt<W>(&self, f: &mut Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: uWrite + ?Sized,
    {
        hex!(self, f, 6)
    }

    #[cfg(target_pointer_width = "32")]
    fn fmt<W>(&self, f: &mut Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: uWrite + ?Sized,
    {
        hex!(self, f, 10)
    }

    #[cfg(target_pointer_width = "64")]
    fn fmt<W>(&self, f: &mut Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: uWrite + ?Sized,
    {
        hex!(self, f, 18)
    }
}

impl<T> uDebug for *mut T {
    #[inline(always)]
    fn fmt<W>(&self, f: &mut Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: uWrite + ?Sized,
    {
        (*self as *const T).fmt(f)
    }
}
