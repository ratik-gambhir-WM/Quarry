//! Enforce the PPTX output cap while ZIP writes and seeks, before response allocation.
use std::io::{self, Cursor, Seek, SeekFrom, Write};
pub(super) struct BoundedCursor(Cursor<Vec<u8>>);
impl BoundedCursor {
    pub fn new() -> Self {
        Self(Cursor::new(Vec::new()))
    }
    pub fn into_inner(self) -> Vec<u8> {
        self.0.into_inner()
    }
}
impl Write for BoundedCursor {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.0.position().saturating_add(bytes.len() as u64) > 64 * 1024 * 1024 {
            return Err(io::Error::other("PowerPoint output exceeds 64 MiB"));
        }
        self.0.write(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Seek for BoundedCursor {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let old = self.0.position();
        let value = self.0.seek(position)?;
        if value > 64 * 1024 * 1024 {
            self.0.set_position(old);
            return Err(io::Error::other("PowerPoint output seek exceeds 64 MiB"));
        }
        Ok(value)
    }
}
