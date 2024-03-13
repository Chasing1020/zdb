use super::error::StorageError;
use byteorder::{LittleEndian, WriteBytesExt};
use crc32fast::Hasher;
use lazy_static::lazy_static;
use num_enum::FromPrimitive;
use std::io::Write;
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, FromPrimitive, EnumIter)]
pub enum RecordType {
    // 0 is reserved for errors
    #[default]
    Zero = 0,
    // Full means the record is fully contained in this block
    Full = 1,
    // First means the record is the first in a sequence of blocks
    First = 2,
    // Middle means the record is neither the first nor the last in a sequence of blocks
    Middle = 3,
    // Last means the record is the last in a sequence of blocks
    Last = 4,
}

pub const BLOCK_SIZE: usize = 32768;
// HEADER = CRC(4B) + LEN(2B) + Type(1B)
pub const HEADER_SIZE: usize = 7;

lazy_static! {
    // TYPE_CRC is crc32c values for all supported record types.
    // These are pre-computed to reduce the overhead of computing the crc of the
    // record type stored in the header.
    pub static ref TYPE_CRC: Vec<u32> = RecordType::iter()
        .map(|t| crc32fast::hash(&[t as u8]))
        .collect();
}

pub struct LogWriter<W: Write> {
    dest: W,
    offset: usize,
}

impl<W: Write> LogWriter<W> {
    pub fn new(dest: W) -> Self {
        LogWriter { dest, offset: 0 }
    }

    pub fn add_record(&mut self, data: &[u8]) -> Result<(), StorageError> {
        let mut record = &data[..];

        let mut begin = true;
        while !record.is_empty() {
            let leftover = BLOCK_SIZE - self.offset;
            if leftover < HEADER_SIZE {
                self.dest.write_all(&vec![0; leftover])?;
                self.offset = 0;
            }
            let avail = BLOCK_SIZE - self.offset - HEADER_SIZE;
            let (record_type, fragment_length) = match (begin, record.len() <= avail) {
                (true, true) => (RecordType::Full, record.len()),
                (true, false) => (RecordType::First, avail),
                (false, true) => (RecordType::Last, record.len()),
                (false, false) => (RecordType::Middle, avail),
            };

            self.emit_physical_record(record_type, record, fragment_length)?;
            record = &record[fragment_length..];
            begin = false;
        }
        Ok(())
    }

    fn emit_physical_record(
        &mut self,
        t: RecordType,
        data: &[u8],
        length: usize,
    ) -> Result<(), StorageError> {
        debug_assert!(length < 0xffff);
        debug_assert!(self.offset + HEADER_SIZE + length <= BLOCK_SIZE);

        let mut header = [0u8; HEADER_SIZE];
        let mut h = Hasher::new_with_initial(TYPE_CRC[t as usize]);
        h.update(data);
        let checksum = h.finalize();
        // let mut checksum_buf = header[..4].as_mut();
        let mut checksum_buf = header[..4].as_mut();
        checksum_buf.write_u32::<LittleEndian>(checksum)?;
        let mut length_buf = header[4..6].as_mut();
        length_buf.write_u16::<LittleEndian>(data.len() as u16)?;
        let mut record_type_buf = header[6..].as_mut();
        record_type_buf.write_u8(t as u8)?;

        self.dest.write(header.as_ref())?;
        self.dest.write(data.to_vec().as_slice())?;
        self.dest.flush()?;
        self.offset += HEADER_SIZE + data.len();

        Ok(())
    }
}



#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_log_writer() {
        let mut writer = LogWriter::new(Cursor::new(Vec::new()));
        let data = vec![1u8; 100];
        writer.add_record(data.as_slice()).unwrap();
    }

    // TODO: add more tests
}