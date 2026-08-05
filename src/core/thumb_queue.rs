use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};
#[derive(Clone, Default)]
pub struct ThumbQueue {
    inner: Arc<ThumbQueueInner>,
}

struct ThumbQueueInner {
    queue: Mutex<VecDeque<PathBuf>>,
    cv: Condvar,
    closed: AtomicBool,
}

impl Default for ThumbQueueInner {
    fn default() -> Self {
        Self {
            queue: Mutex::new(VecDeque::new()),
            cv: Condvar::new(),
            closed: AtomicBool::new(false),
        }
    }
}

impl ThumbQueue {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn from_paths<I>(paths: I) -> Self
    where
        I: IntoIterator<Item = PathBuf>,
    {
        let queue = Self::new();
        let mut inner = queue.inner.queue.lock().unwrap_or_else(|p| p.into_inner());
        inner.extend(paths);
        drop(inner);
        queue
    }
    pub fn push(&self, path: PathBuf) {
        let mut inner = self.inner.queue.lock().unwrap_or_else(|p| p.into_inner());
        inner.push_back(path);
        self.inner.cv.notify_one();
    }
    pub fn pop(&self) -> Option<PathBuf> {
        let mut inner = self.inner.queue.lock().unwrap_or_else(|p| p.into_inner());
        loop {
            if let Some(path) = inner.pop_front() {
                return Some(path);
            }
            if self.inner.closed.load(Ordering::Acquire) {
                return None;
            }
            inner = self.inner.cv.wait(inner).unwrap_or_else(|p| p.into_inner());
        }
    }
    pub fn drain(&self) {
        let mut inner = self.inner.queue.lock().unwrap_or_else(|p| p.into_inner());
        inner.clear();
    }
    pub fn close(&self) {
        let inner = self.inner.queue.lock().unwrap_or_else(|p| p.into_inner());
        self.inner.closed.store(true, Ordering::Release);
        drop(inner);
        self.inner.cv.notify_all();
    }
    pub fn len(&self) -> usize {
        let inner = self.inner.queue.lock().unwrap_or_else(|p| p.into_inner());
        inner.len()
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn p(name: &str) -> PathBuf {
        Path::new("tests/fixtures").join(name)
    }

    #[test]
    fn pushing_then_popping_in_fifo_order() {
        let queue = ThumbQueue::new();
        queue.push(p("a.jpg"));
        queue.push(p("b.jpg"));
        queue.push(p("c.jpg"));
        assert_eq!(queue.pop(), Some(p("a.jpg")));
        assert_eq!(queue.pop(), Some(p("b.jpg")));
        assert_eq!(queue.pop(), Some(p("c.jpg")));
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn drain_discards_pending_paths() {
        let queue = ThumbQueue::new();
        queue.push(p("a.jpg"));
        queue.push(p("b.jpg"));
        queue.drain();
        assert_eq!(queue.len(), 0);
        assert!(queue.is_empty());
    }

    #[test]
    fn drain_does_not_affect_future_pushes() {
        let queue = ThumbQueue::new();
        queue.push(p("a.jpg"));
        queue.drain();
        queue.push(p("b.jpg"));
        assert_eq!(queue.pop(), Some(p("b.jpg")));
    }

    #[test]
    fn close_wakes_waiting_workers_with_none() {
        let queue = ThumbQueue::new();
        let handle = std::thread::spawn({
            let queue = queue.clone();
            move || queue.pop()
        });
        std::thread::sleep(std::time::Duration::from_millis(20));
        queue.close();
        let result = handle.join().expect("worker thread panicked");
        assert_eq!(result, None);
    }

    #[test]
    fn pop_returns_pending_paths_even_after_close() {
        let queue = ThumbQueue::from_paths(vec![p("a.jpg"), p("b.jpg")]);
        queue.close();
        assert_eq!(queue.pop(), Some(p("a.jpg")));
        assert_eq!(queue.pop(), Some(p("b.jpg")));
        assert_eq!(queue.pop(), None);
    }

    #[test]
    fn from_paths_preserves_order() {
        let queue = ThumbQueue::from_paths(vec![p("x.jpg"), p("y.jpg"), p("z.jpg")]);
        assert_eq!(queue.pop(), Some(p("x.jpg")));
        assert_eq!(queue.pop(), Some(p("y.jpg")));
        assert_eq!(queue.pop(), Some(p("z.jpg")));
    }

    #[test]
    fn concurrent_pushes_are_never_lost() {
        let queue = ThumbQueue::new();
        const N: usize = 200;
        let handles: Vec<_> = (0..4)
            .map(|thread_id| {
                let queue = queue.clone();
                std::thread::spawn(move || {
                    for i in 0..N {
                        queue.push(p(&format!("{thread_id}-{i}.jpg")));
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("producer thread panicked");
        }
        assert_eq!(queue.len(), 4 * N);
    }
}
