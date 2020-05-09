use crate::fs::BLOCK_SIZE;
use zerocopy::{AsBytes, FromBytes};

#[derive(Debug, PartialEq)]
pub enum State {
    Free,
    Used,
}

#[repr(C)]
#[derive(AsBytes, FromBytes, Clone, Copy)]
pub struct Bitmap {
    /// Stores 4096 bits mapping each bit to a logical block on disk. A 4K bitmap
    /// supports tracking up to 4096 * 8 logical blocks for a total of 32,768 blocks
    /// per bitmap block.
    bitmap: [u64; BLOCK_SIZE / 8],
}

impl Bitmap {
    pub fn new() -> Self {
        Self {
            bitmap: [0; BLOCK_SIZE / 8],
        }
    }

    pub fn parse(buf: &[u8]) -> Self {
        let map: *const Bitmap = buf.as_ptr() as *const Bitmap;
        unsafe { *map }
    }

    pub fn serialize(&self) -> &[u8] {
        self.as_bytes()
    }

    pub fn get(&self, blocknr: usize) -> State {
        assert!(blocknr < (4096 * 8 - 1));
        // Grab of the u64 containing the significant bit.
        let outer_offset = self.bitmap[blocknr / 64];

        let inner_offset = blocknr % 64;
        let mask = 0b01_u64 << inner_offset;
        let block_state = (outer_offset & mask) >> inner_offset;
        match block_state {
            0 => State::Free,
            1 => State::Used,
            _ => unreachable!("Block state returned a non 0 or 1 value. This likely indicates an error with bitmasking"),
        }
    }

    pub fn set_reserved(&mut self, blocknr: usize) {
        assert!(blocknr < (4096 * 8 - 1));
        // Grab of the u64 containing the significant bit.
        let outer_offset = self.bitmap[blocknr / 64];

        let inner_offset = blocknr % 64;
        let mask = 0b01_u64 << inner_offset;
        self.bitmap[blocknr / 64] = outer_offset | mask;
    }

    #[allow(dead_code)]
    pub fn set_free(&mut self, blocknr: usize) {
        assert!(blocknr < (4096 * 8 - 1));
        // Grab of the u64 containing the significant bit.
        let outer_offset = self.bitmap[blocknr / 64];

        let inner_offset = blocknr % 64;
        let mask = 0b00_u64 << inner_offset;
        self.bitmap[blocknr / 64] = outer_offset & mask;
    }
}

/// Implements a naive block allocation policy for new data block requirements. This policy will
/// retrieve the next available sequential block and on each call to the iterator will return the
/// next consecutive available blocks.
///
/// ## Other Pre-Allocation Policies
///
/// 1. Allocation that attempts to find enough contiguous available blocks so data can be allocated
///    close together (speed ups through sequential reads).
/// 2. Allocation that attempts to spread randomly over blocks to prevent wear of physical devices
///    in the front section (that may be rewritten many times before allocating to the back).
pub struct NextAvailableAllocation {
    /// Keeps track of the next starting place for looking for available blocks.
    marker: usize,
    /// A simple bitmap tracking which blocks are allocated and which are free.
    bitmap: Bitmap,
    /// The maximum allocatable value available in hardware. For example, if you have 80 inode blocks
    /// available on disk, this value would be 80.
    cap: usize,
}

impl NextAvailableAllocation {
    pub fn new(bitmap: Bitmap, cap: Option<usize>) -> Self {
        let cap = cap.unwrap_or_else(|| BLOCK_SIZE / 8);
        Self {
            marker: 0,
            bitmap,
            cap,
        }
    }
}

impl Iterator for NextAvailableAllocation {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        for i in self.marker..self.cap {
            if let State::Free = self.bitmap.get(i) {
                self.marker += 1;
                return Some(i);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_read_and_write_values_to_bitmap() {
        let mut bmp = Bitmap::new();

        bmp.set_reserved(2);

        assert_eq!(bmp.get(0), State::Free);
        assert_eq!(bmp.get(2), State::Used);
    }

    #[test]
    fn can_set_values_at_ends_of_bitmap() {
        let mut bmp = Bitmap::new();

        bmp.set_reserved(0);
        bmp.set_reserved(4095);

        assert_eq!(bmp.get(0), State::Used);
        assert_eq!(bmp.get(4095), State::Used);
    }

    #[test]
    fn can_toggle_block_between_free_and_used() {
        let mut bmp = Bitmap::new();

        bmp.set_reserved(10);
        assert_eq!(bmp.get(10), State::Used);

        bmp.set_free(10);
        assert_eq!(bmp.get(10), State::Free);
    }

    #[test]
    fn can_serialize_and_deserialize_state() {
        let mut bmp = Bitmap::new();
        bmp.set_reserved(10);
        bmp.set_reserved(11);
        bmp.set_reserved(12);

        let read_bmp = Bitmap::parse(bmp.serialize());
        // This is a dumb way of testing equality between two arrays of different
        // lengths. I can't derive debug for the arrays because they exceed the max
        // trait implementation limit, see: https://doc.rust-lang.org/std/primitive.array.html.
        bmp.bitmap.iter().zip(read_bmp.bitmap.iter()).all(|(a, b)| {
            assert_eq!(a, b);
            true
        });
    }
}
