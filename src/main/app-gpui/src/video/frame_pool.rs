use std::sync::Arc;

#[derive(Clone)]
pub struct FramePool {
    tx: smol::channel::Sender<Vec<u8>>,
    rx: smol::channel::Receiver<Vec<u8>>,
}

impl FramePool {
    pub fn new(capacity: usize) -> Self {
        let (tx, rx) = smol::channel::bounded(capacity.max(1));
        Self { tx, rx }
    }

    pub fn take(&self, byte_len: usize) -> Vec<u8> {
        while let Ok(mut buffer) = self.rx.try_recv() {
            if buffer.len() == byte_len {
                return buffer;
            }
            if buffer.capacity() >= byte_len {
                buffer.resize(byte_len, 0);
                return buffer;
            }
        }
        vec![0u8; byte_len]
    }

    #[cfg(all(feature = "parity", target_os = "macos"))]
    pub fn try_take(&self, byte_len: usize) -> Result<Vec<u8>, Vec<u8>> {
        while let Ok(mut buffer) = self.rx.try_recv() {
            if buffer.len() == byte_len {
                return Ok(buffer);
            }
            if buffer.capacity() >= byte_len {
                buffer.resize(byte_len, 0);
                return Ok(buffer);
            }
        }
        Err(vec![0u8; byte_len])
    }

    pub fn share(&self, bytes: Vec<u8>) -> FrameBytes {
        FrameBytes::pooled(bytes, self.tx.clone())
    }

    pub fn clear(&self) {
        while self.rx.try_recv().is_ok() {}
    }
}

struct StoredFrame {
    bytes: Vec<u8>,
    pool: Option<smol::channel::Sender<Vec<u8>>>,
}

impl Drop for StoredFrame {
    fn drop(&mut self) {
        let Some(pool) = self.pool.take() else {
            return;
        };
        let bytes = std::mem::take(&mut self.bytes);
        let _ = pool.try_send(bytes);
    }
}

#[derive(Clone)]
pub struct FrameBytes {
    inner: Arc<StoredFrame>,
}

pub struct FrameWeak {
    inner: std::sync::Weak<StoredFrame>,
}

impl FrameBytes {
    pub fn pooled(bytes: Vec<u8>, pool: smol::channel::Sender<Vec<u8>>) -> Self {
        Self {
            inner: Arc::new(StoredFrame {
                bytes,
                pool: Some(pool),
            }),
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.inner.bytes
    }

    #[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
    pub fn as_ptr(&self) -> *const u8 {
        self.inner.bytes.as_ptr()
    }

    pub fn downgrade(&self) -> FrameWeak {
        FrameWeak {
            inner: Arc::downgrade(&self.inner),
        }
    }

    #[cfg(test)]
    pub fn strong_count(&self) -> usize {
        Arc::strong_count(&self.inner)
    }
}

impl FrameWeak {
    #[cfg(all(test, any(target_os = "macos", target_os = "linux")))]
    pub fn upgrade(&self) -> Option<FrameBytes> {
        self.inner.upgrade().map(|inner| FrameBytes { inner })
    }

    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.inner.ptr_eq(&other.inner)
    }
}

impl From<Vec<u8>> for FrameBytes {
    fn from(bytes: Vec<u8>) -> Self {
        Self {
            inner: Arc::new(StoredFrame { bytes, pool: None }),
        }
    }
}

impl std::ops::Deref for FrameBytes {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl std::fmt::Debug for FrameBytes {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FrameBytes")
            .field("len", &self.len())
            .finish()
    }
}

impl PartialEq for FrameBytes {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_shared_frame_storage_when_the_last_clone_drops() {
        let pool = FramePool::new(1);
        let mut original = pool.take(4);
        original.copy_from_slice(&[9, 8, 7, 6]);
        let address = original.as_ptr();
        let frame = pool.share(original);
        let held = frame.clone();
        drop(frame);
        let while_shared = pool.take(4);
        assert_ne!(while_shared.as_ptr(), address);
        drop(while_shared);
        drop(held);
        let reused = pool.take(4);
        assert_eq!(reused.as_ptr(), address);
        assert_eq!(reused.as_slice(), &[9, 8, 7, 6]);
    }

    #[test]
    fn clear_releases_pooled_buffers() {
        let pool = FramePool::new(2);
        drop(pool.share(vec![9u8; 128]));
        pool.clear();
        let next = pool.take(128);
        assert!(next.iter().all(|&byte| byte == 0));
    }
}
