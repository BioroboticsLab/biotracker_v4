use anyhow::Result;
use shared_memory::*;
use std::collections::VecDeque;

unsafe impl Send for SharedBuffer {}
unsafe impl Sync for SharedBuffer {}
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

    pub fn len(&self) -> usize {
        self.shmem.len()
    }

    pub fn as_ptr(&self) -> *mut u8 {
        self.shmem.as_ptr()
    }
}

pub struct DoubleBuffer {
    data: VecDeque<SharedBuffer>,
}

impl DoubleBuffer {
    pub fn new() -> Self {
        Self { data: [].into() }
    }

    pub fn get(&mut self, len: usize) -> &mut SharedBuffer {
        if self.data.len() >= 2 {
            let buffer = self.data.pop_front().unwrap();
            if buffer.len() == len {
                self.data.push_back(buffer);
                return self.data.back_mut().unwrap();
            }
        }

        let buffer = SharedBuffer::new(len).unwrap();
        self.data.push_back(buffer);
        return self.data.back_mut().unwrap();
    }
}
