use crate::{uDebug, uWrite, Formatter};

impl<T, const N: usize> uDebug for [T; N]
where
    T: uDebug,
{
    #[inline]
    fn fmt<W>(&self, f: &mut Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: uWrite + ?Sized,
    {
        <[T] as uDebug>::fmt(self, f)
    }
}
