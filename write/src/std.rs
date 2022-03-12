use std::{collections::TryReserveError, ptr};

use crate::uWrite;

#[cfg(feature = "std")]
impl uWrite for String {
    type Error = TryReserveError;

    #[inline(never)]
    fn write_str(&mut self, s: &str) -> Result<(), Self::Error> {
        unsafe {
            let vec = self.as_mut_vec();
            vec.try_reserve(s.len())?;

            let cap = vec.as_mut_ptr_range().end;
            ptr::copy(s.as_ptr(), cap, s.len());
            vec.set_len(vec.len() + s.len());
        }
        Ok(())
    }
}
