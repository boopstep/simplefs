use std::convert::TryInto;
/// Keeps track of free vs allocated memory blocks with a space efficient bitmap
/// representation.

// TODO(allancalix): Replace hard coded block size with a parameter.
const BLOCK_SIZE: usize = 4096 / 8;

#[derive(Debug, PartialEq)]
pub enum State {
    Free,
    Used,
}

pub struct BitmapBlock {
    /// Stores 4096 bits mapping each bit to a logical block on disk. A 4K bitmap
    /// supports tracking up to 4096 * 8 logical blocks for a total of 32,768 blocks
    /// per bitmap block.
    bitmap: [u64; BLOCK_SIZE],
}

impl BitmapBlock {
    pub fn new() -> Self {
        Self {
            bitmap: [0; BLOCK_SIZE],
        }
    }

    pub fn from(buf: &[u8]) -> Self {
        assert_eq!(buf.len(), BLOCK_SIZE * 8);

        let bitmap: Vec<u64> = buf
            .chunks(8)
            .map(|v| u64::from_be_bytes(v.try_into().unwrap()))
            .collect();

        let mut decoded_block = [0; BLOCK_SIZE];
        decoded_block.copy_from_slice(&bitmap);
        Self {
            bitmap: decoded_block,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut encoded_buf = vec![];

        for &b in self.bitmap.iter() {
            encoded_buf.extend_from_slice(&b.to_be_bytes());
        }
        assert_eq!(encoded_buf.len(), 4096);
        encoded_buf
    }

    pub fn get_state(&self, blocknr: usize) -> State {
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

    pub fn set_used(&mut self, blocknr: usize) {
        assert!(blocknr < (4096 * 8 - 1));
        // Grab of the u64 containing the significant bit.
        let outer_offset = self.bitmap[blocknr / 64];

        let inner_offset = blocknr % 64;
        let mask = 0b01_u64 << inner_offset;
        self.bitmap[blocknr / 64] = outer_offset | mask;
    }

    pub fn set_free(&mut self, blocknr: usize) {
        assert!(blocknr < (4096 * 8 - 1));
        // Grab of the u64 containing the significant bit.
        let outer_offset = self.bitmap[blocknr / 64];

        let inner_offset = blocknr % 64;
        let mask = 0b00_u64 << inner_offset;
        self.bitmap[blocknr / 64] = outer_offset & mask;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_read_and_write_values_to_bitmap() {
        let mut bmp = BitmapBlock::new();

        bmp.set_used(2);

        assert_eq!(bmp.get_state(0), State::Free);
        assert_eq!(bmp.get_state(2), State::Used);
    }

    #[test]
    fn can_set_values_at_ends_of_bitmap() {
        let mut bmp = BitmapBlock::new();

        bmp.set_used(0);
        bmp.set_used(4095);

        assert_eq!(bmp.get_state(0), State::Used);
        assert_eq!(bmp.get_state(4095), State::Used);
    }

    #[test]
    fn can_toggle_block_between_free_and_used() {
        let mut bmp = BitmapBlock::new();

        bmp.set_used(10);
        assert_eq!(bmp.get_state(10), State::Used);

        bmp.set_free(10);
        assert_eq!(bmp.get_state(10), State::Free);
    }

    #[test]
    fn can_serialize_and_deserialize_state() {
        let mut bmp = BitmapBlock::new();
        bmp.set_used(10);
        bmp.set_used(11);
        bmp.set_used(12);

        let read_bmp = BitmapBlock::from(&bmp.serialize());
        // This is a dumb way of testing equality between two arrays of different
        // lengths. I can't derive debug for the arrays because they exceed the max
        // trait implementation limit, see: https://doc.rust-lang.org/std/primitive.array.html.
        bmp.bitmap.iter().zip(read_bmp.bitmap.iter()).all(|(a, b)| {
            assert_eq!(a, b);
            true
        });
    }
}
