use std::{fs::File, path::Path};

use crate::db::error::StorageResult;

pub struct FileObject {
    file: Option<File>,
    size: u64,
}

impl FileObject {
    pub fn new(file: Option<File>, size: u64) -> Self {
        Self { file, size }
    }

    pub fn read(&self, offset: u64, len: u64) -> StorageResult<Vec<u8>> {
        use std::os::unix::fs::FileExt;
        let mut data = vec![0; len as usize];
        self.file
            .as_ref()
            .unwrap()
            .read_exact_at(&mut data[..], offset)?;
        Ok(data)
    }

    pub fn read_to(&self, buf: &mut [u8], offset: u64) -> StorageResult<()> {
        use std::os::unix::fs::FileExt;
        self.file.as_ref().unwrap().read_exact_at(buf, offset)?;
        Ok(())
    }

    pub fn create(path: &impl AsRef<Path>, data: impl AsRef<[u8]>) -> StorageResult<Self> {
        std::fs::write(path, &data)?;
        File::open(path)?.sync_all()?;
        Ok(Self {
            file: Some(File::options().read(true).write(false).open(path)?),
            size: data.as_ref().len() as u64,
        })
    }

    pub fn open(path: &impl AsRef<Path>) -> StorageResult<Self> {
        let file = File::options().read(true).write(false).open(path)?;
        let size = file.metadata()?.len();
        Ok(FileObject {
            file: Some(file),
            size,
        })
    }

    #[inline]
    pub fn size(&self) -> u64 {
        self.size
    }
}
