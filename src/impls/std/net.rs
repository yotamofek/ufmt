use std::net::Ipv4Addr;

use crate::{uDebug, uDisplay, uWrite, uwrite, Formatter};

impl uDisplay for Ipv4Addr {
    #[inline]
    fn fmt<W>(&self, f: &mut Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: uWrite + ?Sized,
    {
        let [a, b, c, d] = self.octets();
        uwrite!(f, "{a}.{b}.{c}.{d}")
    }
}

impl uDebug for Ipv4Addr {
    #[inline]
    fn fmt<W>(&self, f: &mut Formatter<'_, W>) -> Result<(), W::Error>
    where
        W: uWrite + ?Sized,
    {
        uDisplay::fmt(self, f)
    }
}
