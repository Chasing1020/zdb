// use super::error::StorageError;
// use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
// use crc32fast::Hasher;
// use lazy_static::lazy_static;
// use num_enum::FromPrimitive;
// use std::io::{Read, Write};
// use strum::IntoEnumIterator;
// use strum_macros::EnumIter;

// #[repr(u8)]
// #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default, FromPrimitive, EnumIter)]
// pub enum RecordType {
//     // 0 is reserved for errors
//     #[default]
//     Zero = 0,
//     // Full means the record is fully contained in this block
//     Full = 1,
//     // First means the record is the first in a sequence of blocks
//     First = 2,
//     // Middle means the record is neither the first nor the last in a sequence of blocks
//     Middle = 3,
//     // Last means the record is the last in a sequence of blocks
//     Last = 4,
//     // Eof means the record is a synthetic record that contains no data
//     Eof = 5,
//     // BadRecord means the record is corrupted
//     BadRecord = 6,
// }

// pub const BLOCK_SIZE: usize = 32768;
// // HEADER = CRC(4B) + LEN(2B) + Type(1B)
// pub const HEADER_SIZE: usize = 7;

// lazy_static! {
//     // TYPE_CRC is crc32c values for all supported record types.
//     // These are pre-computed to reduce the overhead of computing the crc of the
//     // record type stored in the header.
//     pub static ref TYPE_CRC: Vec<u32> = RecordType::iter()
//         .map(|t| crc32fast::hash(&[t as u8]))
//         .collect();
// }

// pub struct LogWriter<W: Write> {
//     writer: W,
//     offset: usize,
// }

// impl<W: Write> LogWriter<W> {
//     pub fn new(dest: W) -> Self {
//         LogWriter {
//             writer: dest,
//             offset: 0,
//         }
//     }

//     pub fn add_record(&mut self, data: &[u8]) -> Result<(), StorageError> {
//         let mut record = data;

//         let mut begin = true;
//         while !record.is_empty() {
//             let leftover: isize = BLOCK_SIZE as isize - self.offset as isize;
//             debug_assert!(leftover >= 0);
//             if (leftover as usize) < HEADER_SIZE {
//                 self.writer.write_all(&vec![0; leftover as usize])?;
//                 self.offset = 0;
//             }
//             let avail = BLOCK_SIZE - self.offset - HEADER_SIZE;
//             let (record_type, fragment_length) = match (begin, record.len() <= avail) {
//                 (true, true) => (RecordType::Full, record.len()),
//                 (true, false) => (RecordType::First, avail),
//                 (false, true) => (RecordType::Last, record.len()),
//                 (false, false) => (RecordType::Middle, avail),
//             };

//             self.emit_physical_record(record_type, record, fragment_length)?;
//             record = &record[fragment_length..];
//             begin = false;
//         }
//         Ok(())
//     }

//     fn emit_physical_record(
//         &mut self,
//         t: RecordType,
//         data: &[u8],
//         length: usize,
//     ) -> Result<(), StorageError> {
//         debug_assert!(length < 0xffff);
//         debug_assert!(self.offset + HEADER_SIZE + length <= BLOCK_SIZE);

//         let mut header = [0u8; HEADER_SIZE];
//         let mut h = Hasher::new_with_initial(TYPE_CRC[t as usize]);
//         h.update(data);
//         let checksum = h.finalize();
//         // let mut checksum_buf = header[..4].as_mut();
//         let mut checksum_buf = header[..4].as_mut();
//         checksum_buf.write_u32::<LittleEndian>(checksum)?;
//         let mut length_buf = header[4..6].as_mut();
//         length_buf.write_u16::<LittleEndian>(length as u16)?;
//         let mut record_type_buf = header[6..].as_mut();
//         record_type_buf.write_u8(t as u8)?;

//         self.writer.write_all(header.as_ref())?;
//         self.writer.write_all(&data[..length])?;
//         self.writer.flush()?;
//         self.offset += HEADER_SIZE + length;

//         Ok(())
//     }
// }

// // LogReader is a reader for log files.
// pub struct LogReader<R: Read> {
//     reader: R,
//     buffer: Vec<u8>,
//     // checksum indicates whether to verify checksums when reading
//     end_of_buffer_offset: usize,
//     initial_offset: usize,
//     resyncing: bool,
//     checksum: bool,
//     eof: bool,
// }

// impl<R: Read> LogReader<R> {
//     pub fn new(reader: R, checksum: bool, initial_offset: usize) -> Self {
//         LogReader {
//             reader,
//             buffer: vec![],
//             end_of_buffer_offset: 0,
//             checksum,
//             initial_offset,
//             resyncing: initial_offset > 0,
//             eof: false,
//         }
//     }

//     pub fn read_record(&mut self) -> Result<Vec<u8>, StorageError> {
//         let mut record = Vec::new();
//         let mut in_fragmented_record = false;

//         loop {
//             let record_type = self.read_physical_record(&mut record);

//             if self.resyncing {
//                 match record_type {
//                     RecordType::Middle => continue,
//                     RecordType::Last => {
//                         self.resyncing = false;
//                         continue;
//                     }
//                     _ => (),
//                 }
//             }

//             match record_type {
//                 RecordType::Full => {
//                     if in_fragmented_record && !record.is_empty() {
//                         record.clear();
//                         return Err(StorageError::Corruption(
//                             "partial record without end".to_string(),
//                         ));
//                     }
//                     return Ok(record);
//                 }
//                 RecordType::First => {
//                     if in_fragmented_record && !record.is_empty() {
//                         record.clear();
//                         return Err(StorageError::Corruption("bad record".to_string()));
//                     }
//                     in_fragmented_record = true;
//                 }
//                 RecordType::Middle => {
//                     if !in_fragmented_record {
//                         let mut data = self.buffer.split_off(HEADER_SIZE);
//                         record.append(&mut data);
//                     } else {
//                         record.clear();
//                         return Err(StorageError::Corruption("bad record".to_string()));
//                     }
//                 }
//                 RecordType::Last => {
//                     if in_fragmented_record {
//                         return Ok(record);
//                     }
//                 }
//                 RecordType::Eof => return Err(StorageError::Eof),
//                 RecordType::BadRecord => {
//                     return Err(StorageError::Corruption("bad record".to_string()))
//                 }
//                 _ => return Err(StorageError::Corruption("unknown record type".to_string())),
//             }
//         }
//     }

//     pub fn read_physical_record(&mut self, restult: &mut Vec<u8>) -> RecordType {
//         loop {
//             if self.buffer.len() < HEADER_SIZE {
//                 if !self.eof {
//                     self.buffer.clear();
//                     match self.reader.read_to_end(self.buffer.as_mut()) {
//                         Ok(n) => {
//                             self.end_of_buffer_offset += n;
//                             if n < BLOCK_SIZE {
//                                 self.eof = true;
//                             }
//                         }
//                         Err(_) => {
//                             self.eof = true;
//                             return RecordType::Eof;
//                         }
//                     }
//                     continue;
//                 } else {
//                     // Note that if buffer is non-empty, we have a truncated header at the
//                     // end of the file, which can be caused by the writer crashing in the
//                     // middle of writing the header. Instead of considering this an error,
//                     // just report EOF.
//                     self.buffer.clear();
//                     return RecordType::Eof;
//                 }
//             }

//             let data = self.buffer.as_slice();
//             let length = u16::from_le_bytes([data[4], data[5]]);
//             let record_type = RecordType::from(data[6]);

//             if HEADER_SIZE + length as usize > self.buffer.len() {
//                 self.buffer.clear();
//                 if !self.eof {
//                     return RecordType::BadRecord;
//                 }
//                 // If the end of the file has been reached without reading |length| bytes
//                 // of payload, assume the writer died in the middle of writing the record.
//                 // Don't report a corruption.
//                 return RecordType::Eof;
//             }

//             if record_type == RecordType::Zero && length == 0 {
//                 self.buffer.clear();
//                 return RecordType::BadRecord;
//             }

//             if self.checksum {
//                 let mut crc_header = &data[..4];
//                 let expected = crc_header.read_u32::<LittleEndian>().unwrap();
//                 let mut h = Hasher::new_with_initial(TYPE_CRC[record_type as usize]);
//                 h.update(data[6..].as_ref());
//                 let actual = h.finalize();
//                 if expected != actual {
//                     self.buffer.clear();
//                     return RecordType::BadRecord;
//                 }
//                 // let dropped = self.cap - self.consumed;
//                 // self.consumed = 0;
//                 // self.cap = 0;
//                 // self.reporter.corruption(
//                 //     dropped,
//                 //     StatusError::Corruption("checksum mismatch".to_string()),
//                 // );
//                 // return Ok((RecordType::BadRecord, length as usize));
//             }

//             self.buffer.drain(..HEADER_SIZE);
//             restult.extend(self.buffer.drain(..(length as usize)));
//             if (self.end_of_buffer_offset - self.buffer.len() - HEADER_SIZE - length as usize)
//                 < self.initial_offset
//             {
//                 self.buffer.clear();
//                 return RecordType::BadRecord;
//             }

//             return record_type;
//         }
//     }
// }

// #[cfg(test)]
// mod tests {
//     use super::*;
//     use std::io::Cursor;

//     #[test]
//     fn test_log_writer() {
//         let mut writer = LogWriter::new(Cursor::new(Vec::new()));
//         let data = vec![1u8; 1000];
//         writer.add_record(data.as_slice()).unwrap();
//         let data = vec![1u8; 50000];
//         writer.add_record(data.as_slice()).unwrap();
//     }

//     // TODO: add more LogReader tests
//     #[test]
//     fn test_log_reader() {
//         let file = Vec::new();
//         let mut writer = LogWriter::new(file);
//         let data1 = vec![1u8; 1000];
//         writer.add_record(data1.as_slice()).unwrap();
//         let data2 = vec![1u8; 50000];
//         writer.add_record(data2.as_slice()).unwrap();

//         let mut reader = LogReader::new(writer.writer.as_slice(), false, 0);
//         let record1 = reader.read_record();
//         assert_eq!(record1.unwrap().as_slice(), data1.as_slice());
//         let record2 = reader.read_record();
//         assert_eq!(record2.unwrap().as_slice(), data2.as_slice());
//     }
// }
