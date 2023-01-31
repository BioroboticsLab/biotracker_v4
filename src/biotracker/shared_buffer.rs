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

    pub unsafe fn as_slice(&self) -> &[u8] {
        self.shmem.as_slice()
    }

    pub unsafe fn as_slice_mut(&mut self) -> &mut [u8] {
        assert!(self.shmem.is_owner());
        self.shmem.as_slice_mut()
    }
}

pub struct DoubleBuffer {
    data: VecDeque<SharedBuffer>,
}

impl DoubleBuffer {
    pub fn new() -> Self {
        Self { data: [].into() }
    }

    pub fn push(&mut self, buffer: SharedBuffer) {
        if self.data.len() >= 2 {
            self.data.pop_front();
        }
        self.data.push_back(buffer);
    }
}
