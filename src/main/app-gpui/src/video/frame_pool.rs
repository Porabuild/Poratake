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

    pub fn recycle_arc(&self, buffer: Arc<Vec<u8>>) {
        if let Ok(bytes) = Arc::try_unwrap(buffer) {
            let _ = self.tx.try_send(bytes);
        }
    }

    pub fn clear(&self) {
        while self.rx.try_recv().is_ok() {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recycles_only_unshared_buffers_with_matching_size() {
        let pool = FramePool::new(2);
        let shared = Arc::new(vec![7u8; 64]);
        let keep = shared.clone();
        pool.recycle_arc(shared);
        drop(keep);

        let fresh = pool.take(64);
        assert!(fresh.iter().all(|&byte| byte == 0));

        let owned = Arc::new(vec![42u8; 64]);
        let ptr = owned.as_ptr();
        pool.recycle_arc(owned);
        let reused = pool.take(64);
        assert_eq!(reused.as_ptr(), ptr);
    }

    #[test]
    fn clear_releases_pooled_buffers() {
        let pool = FramePool::new(2);
        let owned = Arc::new(vec![9u8; 128]);
        pool.recycle_arc(owned);
        pool.clear();
        let next = pool.take(128);
        assert!(next.iter().all(|&byte| byte == 0));
    }
}
