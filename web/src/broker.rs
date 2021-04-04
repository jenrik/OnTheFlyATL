use atl_checker::common::{Message, VertexAssignment};
use std::collections::VecDeque;
use std::error::Error;
use std::hash::Hash;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone)]
pub struct SimpleBroker<V: Hash + Eq + PartialEq + Clone> {
    inner: Arc<Mutex<SimpleBrokerInner<V>>>,
}

#[derive(Debug, Clone)]
struct SimpleBrokerInner<V: Hash + Eq + PartialEq + Clone> {
    queue: VecDeque<Message<V>>,
    result: Option<VertexAssignment>,
}

impl<V: Hash + Eq + PartialEq + Clone> atl_checker::com::Broker<V> for SimpleBroker<V> {
    fn send(&self, to: u64, msg: Message<V>) {
        debug_assert_eq!(to, 0);
        let temp = self.inner.clone();
        let mut inner = temp.lock().unwrap();
        inner.queue.push_back(msg);
    }

    fn return_result(&self, assignment: VertexAssignment) {
        let temp = self.inner.clone();
        let mut inner = temp.lock().unwrap();
        debug_assert!(inner.result.is_none());
        inner.result = Some(assignment);

        inner.queue.push_front(Message::TERMINATE);
    }

    fn release(&self, depth: usize) {
        self.send(0u64, Message::RELEASE(depth));
    }

    fn terminate(&self) {
        self.send(0u64, Message::TERMINATE);
    }

    fn receive(&self) -> Result<Option<Message<V>>, Box<dyn Error>> {
        let temp = self.inner.clone();
        let mut inner = temp.lock().unwrap();
        Ok(inner.queue.pop_front())
    }
}

impl<V: Hash + Eq + PartialEq + Clone> SimpleBroker<V> {
    pub fn get_result(&self) -> Option<VertexAssignment> {
        let temp = self.inner.clone();
        let mut inner = temp.lock().unwrap();
        inner.result.clone()
    }

    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(SimpleBrokerInner::<V> {
                queue: Default::default(),
                result: None,
            })),
        }
    }
}
