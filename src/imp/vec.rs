//! # Unstable functions from `std::vec::Vec`

use std::ptr;
use std::slice;

/// `Vec::drain_filter`
/// #[unstable(feature = "drain_filter", reason = "recently added", issue = "43244")]
pub fn drain_filter<T, F>(this: &mut Vec<T>, filter: F) -> DrainFilter<T, F>
where
    F: FnMut(&mut T) -> bool,
{
    let old_len = this.len();

    // Guard against us getting leaked (leak amplification)
    unsafe {
        this.set_len(0);
    }

    DrainFilter {
        vec: this,
        idx: 0,
        del: 0,
        old_len,
        pred: filter,
    }
}

/// An iterator produced by calling `drain_filter` on Vec.
/// #[unstable(feature = "drain_filter", reason = "recently added", issue = "43244")]
#[derive(Debug)]
pub struct DrainFilter<'a, T: 'a, F>
where
    F: FnMut(&mut T) -> bool,
{
    vec: &'a mut Vec<T>,
    idx: usize,
    del: usize,
    old_len: usize,
    pred: F,
}

/// #[unstable(feature = "drain_filter", reason = "recently added", issue = "43244")]
impl<'a, T, F> Iterator for DrainFilter<'a, T, F>
where
    F: FnMut(&mut T) -> bool,
{
    type Item = T;

    fn next(&mut self) -> Option<T> {
        unsafe {
            while self.idx != self.old_len {
                let i = self.idx;
                self.idx += 1;
                let v = slice::from_raw_parts_mut(self.vec.as_mut_ptr(), self.old_len);
                if (self.pred)(&mut v[i]) {
                    self.del += 1;
                    return Some(ptr::read(&v[i]));
                } else if self.del > 0 {
                    let del = self.del;
                    let src: *const T = &v[i];
                    let dst: *mut T = &mut v[i - del];
                    // This is safe because self.vec has length 0
                    // thus its elements will not have Drop::drop
                    // called on them in the event of a panic.
                    ptr::copy_nonoverlapping(src, dst, 1);
                }
            }
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.old_len - self.idx))
    }
}

/// #[unstable(feature = "drain_filter", reason = "recently added", issue = "43244")]
impl<'a, T, F> Drop for DrainFilter<'a, T, F>
where
    F: FnMut(&mut T) -> bool,
{
    fn drop(&mut self) {
        self.for_each(drop);
        unsafe {
            self.vec.set_len(self.old_len - self.del);
        }
    }
}
