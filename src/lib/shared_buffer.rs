use anyhow::Result;
use shared_memory::*;
use std::collections::HashMap;

pub struct SharedBuffer {
    shmem: Shmem,
}

impl SharedBuffer {
    pub fn new(len: usize) -> Result<Self> {
        let shmem = ShmemConf::new().size(len).create()?;
        Ok(Self { shmem })
    }

    pub fn open(id: &str) -> Result<Self> {
        let shmem = ShmemConf::new().os_id(id).open()?;
        Ok(Self { shmem })
    }

    pub fn id(&self) -> &str {
        self.shmem.get_os_id()
    }

    pub unsafe fn as_slice(&self) -> &[u8] {
        self.shmem.as_slice()
    }

    pub unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
        assert!(self.shmem.is_owner());
        self.shmem.as_slice_mut()
    }
}

pub struct BufferManager {
    data: HashMap<String, SharedBuffer>,
}

impl BufferManager {
    pub fn new() -> Self {
        Self { data: [].into() }
    }
    pub fn get(&mut self, id: &str) -> Option<&SharedBuffer> {
        if self.data.contains_key(id) {
            return self.data.get(id);
        }
        match SharedBuffer::open(id) {
            Ok(buffer) => {
                self.data.insert(id.to_owned(), buffer);
                return self.data.get(id);
            }
            Err(e) => {
                eprintln!("Failed to access buffer {}: {}", id, e);
                return None;
            }
        }
    }

    pub fn allocate(&mut self, len: usize) -> Result<&mut SharedBuffer> {
        let mut reuse_buf_id: Option<String> = None;
        let mut erase_buf_id: Option<String> = None;
        if self.data.len() >= 16 {
            for (id, buf) in &self.data {
                if buf.shmem.is_owner() {
                    // TODO: implement distributed refcount, recycle oldest buffers
                    if buf.shmem.len() == len {
                        reuse_buf_id = Some(id.to_owned());
                    } else {
                        erase_buf_id = Some(id.to_owned());
                    }
                }
            }
        }

        if let Some(id) = reuse_buf_id {
            return Ok(self.data.get_mut(&id).unwrap());
        }

        if let Some(id) = erase_buf_id {
            self.data.remove(&id);
        }
        let buf = SharedBuffer::new(len)?;
        let id = buf.id().to_owned();
        self.data.insert(buf.id().to_owned(), buf);
        return Ok(self.data.get_mut(&id).unwrap());
    }
}
