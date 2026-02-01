use parking_lot::Mutex;
use bytes::BytesMut;

/// A simple buffer pool to reuse `BytesMut` allocations and reduce GC pressure/allocations.
pub struct BufferPool {
    pool: Mutex<Vec<BytesMut>>,
    buffer_size: usize,
    max_buffers: usize,
}

impl BufferPool {
    /// Creates a new buffer pool.
    /// `max_buffers`: Maximum number of buffers to keep in the pool.
    /// `buffer_size`: Default capacity for new buffers.
    pub fn new(max_buffers: usize, buffer_size: usize) -> Self {
        Self {
            pool: Mutex::new(Vec::with_capacity(max_buffers)),
            buffer_size,
            max_buffers,
        }
    }

    /// Acquires a buffer with at least `size` capacity.
    /// Returns a `BytesMut` from the pool or creates a new one if the pool is not full.
    pub fn acquire(&self, size: usize) -> BytesMut {
        let mut pool = self.pool.lock();
        if let Some(i) = pool.iter().position(|b| b.capacity() >= size) {
            let mut b = pool.remove(i);
            b.clear();
            b
        } else {
            // If pool is empty or no buffer has enough capacity, create a new one
            BytesMut::with_capacity(size.max(self.buffer_size))
        }
    }

    /// Releases a buffer back to the pool if it has space.
    pub fn release(&self, mut buf: BytesMut) {
        let mut pool = self.pool.lock();
        if pool.len() < self.max_buffers {
            buf.clear();
            pool.push(buf);
        }
    }

    /// Clears all buffers from the pool.
    pub fn clear(&self) {
        self.pool.lock().clear();
    }
}
