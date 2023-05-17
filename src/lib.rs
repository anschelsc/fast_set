use std::alloc::{alloc, dealloc, Layout, LayoutError};
use std::error::Error;
use std::fmt::Display;


#[derive(Debug)]
/// An `OutOfBounds` error occurs when [`FastSet::add`] or [`FastSet::remove`]
/// is called with a key that is higher than the set's capacity
pub struct OutOfBounds {
    pub cap: usize,
    pub key: usize,
}

impl Display for OutOfBounds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "index out of range: cap = {}, key = {}",
            self.cap, self.key
        )
    }
}
impl Error for OutOfBounds {}

/// A `FastSet` is a set of `usize` with fast add, remove, contains, and clear operations.
/// Each instance of `FastSet` has some maximal value, and uses heap space
/// proportional to that value. Every operation except cloning, including [`new`](FastSet::new)
/// and [`clear`](FastSet::clear), runs in constant time.
pub struct FastSet {
    sparse: *mut usize,
    backref: *mut usize,
    len: usize,
    cap: usize,
}

impl FastSet {
    /// Create a new `FastSet`, which will hold values less than `cap`.
    /// Allocates `O(cap)` bytes of heap memory.
    /// Returns an error if `cap` is greater than `isize::MAX`.
    pub fn new(cap: usize) -> Result<FastSet, LayoutError> {
        let layout = Layout::array::<usize>(cap)?;
        let sparse = unsafe { alloc(layout) as *mut usize };
        let backref = unsafe { alloc(layout) as *mut usize };
        Ok(FastSet {
            sparse,
            backref,
            len: 0,
            cap,
        })
    }

    /// Returns the length of the set, i.e. the number of items it contains.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns the capacity of the set, i.e. the lowest value that cannot be
    /// stored. This is always equal to the value passed when calling
    /// [`new`](FastSet::new).
    pub fn cap(&self) -> usize {
        self.cap
    }

    /// Checks whether the set contains the given key. Will always return
    /// `false` if `key >= self.cap()`.
    pub fn contains(&self, key: usize) -> bool {
        if key >= self.cap {
            false
        } else {
            unsafe { self.unchecked_contains(key) }
        }
    }

    /// Adds the given key to the set. Returns an `OutOfBounds` if `key > self.cap()`.
    /// No-op if `self.contains(key)`.
    pub fn add(&mut self, key: usize) -> Result<(), OutOfBounds> {
        if key >= self.cap {
            return Err(OutOfBounds { cap: self.cap, key });
        }
        unsafe {
            if !self.unchecked_contains(key) {
                self.unchecked_add(key);
            }
        }
        Ok(())
    }

    /// Removes the given key from the set.
    /// Returns an `OutOfBounds` if `key > self.cap()`.
    /// No-op if `!self.contains(key)`.
    pub fn remove(&mut self, key: usize) -> Result<(), OutOfBounds> {
        if key >= self.cap {
            return Err(OutOfBounds { cap: self.cap, key });
        }
        unsafe {
            if self.unchecked_contains(key) {
                self.unchecked_remove(key);
            }
        }
        Ok(())
    }

    /// Removes all elements from the set.
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// Returns a slice containing the keys of the set, in arbitrary order.
    pub fn keys(&self) -> &[usize] {
        unsafe { std::slice::from_raw_parts(self.backref, self.len) }
    }

    /// Checks whether the set contains the given key. The key must be less than
    /// `self.cap()`.
    pub unsafe fn unchecked_contains(&self, key: usize) -> bool {
        // We are assuming key < cap, in particular key < isize::MAX
        let index = *self.sparse.offset(key as isize);
        if index >= self.len {
            return false;
        }
        *self.backref.offset(index as isize) == key
    }

    /// Adds the given key to the set. The key must be less than `self.cap()`
    /// and not already in the set.
    pub unsafe fn unchecked_add(&mut self, key: usize) {
        // Assuming key < cap and key is not already in the set
        *self.sparse.offset(key as isize) = self.len;
        *self.backref.offset(self.len as isize) = key;
        self.len += 1;
    }

    /// Removes the given key from the set. The key must be less than
    /// `self.cap()` and already in the set.
    pub unsafe fn unchecked_remove(&mut self, key: usize) {
        // Assuming self.contains(key) so in particular key < cap
        let to_delete_index = *self.sparse.offset(key as isize);
        let to_delete = self.backref.offset(to_delete_index as isize);
        let last = self.backref.offset(self.len as isize - 1);
        let moved_key = *last;
        *to_delete = moved_key;
        *self.sparse.offset(moved_key as isize) = to_delete_index;
        self.len -= 1;
    }
}

impl Drop for FastSet {
    fn drop(&mut self) {
        let layout = Layout::array::<usize>(self.cap).unwrap(); // If this was gonna fail it would have at New()
        unsafe {
            dealloc(self.sparse as *mut u8, layout);
            dealloc(self.backref as *mut u8, layout);
        }
    }
}

/// This implementation wraps [`FastSet::keys`]; nothing fancy going on.
impl<'a> IntoIterator for &'a FastSet {
    type Item = &'a usize;
    type IntoIter = std::slice::Iter<'a, usize>;

    fn into_iter(self) -> Self::IntoIter {
        self.keys().into_iter()
    }
}

impl Clone for FastSet {
    /// Cloning a `FastSet` takes `O(self.len())` time.
    fn clone(&self) -> Self {
        let mut ret = Self::new(self.cap).unwrap();
        unsafe {
            for key in self {
                ret.unchecked_add(*key);
            }
        }
        ret
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let mut set = FastSet::new(234).unwrap();
        assert!(!set.contains(5));
        assert!(!set.contains(300));
        set.add(5).unwrap();
        set.add(3).unwrap();
        set.add(5).unwrap();
        assert!(set.contains(3));
        assert!(!set.contains(4));
        assert!(set.contains(5));
        assert_eq!(set.len(), 2);
        set.remove(3).unwrap();
        assert!(set.contains(5));
        assert!(!set.contains(3));
        assert_eq!(set.len(), 1);
        for key in &set {
            assert_eq!(*key, 5);
        }
        let other = set.clone();
        set.clear();
        assert!(!set.contains(5));
        assert!(other.contains(5));
    }
}
