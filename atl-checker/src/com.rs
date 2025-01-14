use crate::common::{Message, VertexAssignment, WorkerId};
use crossbeam_channel::{unbounded, Receiver, Sender, TryRecvError};
use std::error::Error;
use std::hash::Hash;

/// Broker implement the function of W_E, W_N, M_R and M_A
pub trait Broker<V: Hash + Eq + PartialEq + Clone> {
    /// Send message to worker with id `to`
    fn send(&self, to: WorkerId, msg: Message<V>);

    /// Send result to the main thread
    fn return_result(&self, assignment: VertexAssignment);

    /// Signal all workers to release the given depth
    fn release(&self, depth: usize);

    /// Signal to all workers to terminate
    fn terminate(&self);

    /// Attempts to retrieve a message destined for the current worker.
    ///
    /// Returns `Ok(Some(V))` when a message available, and `Ok(None)` when no message is currently available.
    /// Returns `Err` in case of unrecoverable error.
    fn receive(&self) -> Result<Option<Message<V>>, Box<dyn Error>>;
}

pub trait BrokerManager {
    fn receive_result(&self) -> Result<VertexAssignment, Box<dyn Error>>;
}

pub struct ChannelBrokerManager {
    result: Receiver<VertexAssignment>,
}

impl BrokerManager for ChannelBrokerManager {
    fn receive_result(&self) -> Result<VertexAssignment, Box<dyn Error>> {
        match self.result.recv() {
            Ok(msg) => Ok(msg),
            Err(err) => Err(Box::new(err)),
        }
    }
}

/// Implements Broker using channels from crossbeam_channel
#[derive(Debug)]
pub struct ChannelBroker<V: Hash + Eq + PartialEq + Clone> {
    workers: Vec<Sender<Message<V>>>,
    result: Sender<VertexAssignment>,
    receiver: Receiver<Message<V>>,
}

impl<V: Hash + Eq + PartialEq + Clone> Broker<V> for ChannelBroker<V> {
    fn send(&self, to: WorkerId, msg: Message<V>) {
        debug!("send");
        self.workers
            .get(to as usize)
            .expect("receiver id out of bounds")
            .send(msg)
            .expect(&*format!("Send to worker {} failed", to));
    }

    fn return_result(&self, assignment: VertexAssignment) {
        self.result
            .send(assignment)
            .expect("Failed to send result to main thread");
        self.terminate();
    }

    fn release(&self, depth: usize) {
        for i in 0..self.workers.len() {
            self.send(i as u64, Message::RELEASE(depth))
        }
    }

    fn terminate(&self) {
        for to in 0..self.workers.len() {
            // Ignore send error, because the error means the worker have already terminated
            #[warn(unused_must_use)]
            self.workers
                .get(to as usize)
                .expect("receiver id out of bounds")
                .send(Message::TERMINATE);
        }
    }

    fn receive(&self) -> Result<Option<Message<V>>, Box<dyn Error>> {
        match self.receiver.try_recv() {
            Ok(msg) => Ok(Some(msg)),
            Err(err) => match err {
                TryRecvError::Empty => {
                    debug!("nothing to receive");
                    Ok(None)
                }
                TryRecvError::Disconnected => Err(Box::new(err)),
            },
        }
    }
}

impl<V: Hash + Eq + PartialEq + Clone> ChannelBroker<V> {
    pub fn new(worker_count: u64) -> (Vec<Self>, ChannelBrokerManager) {
        // Create a message channel foreach worker
        let mut msg_senders = Vec::with_capacity(worker_count as usize);
        let mut msg_receivers = Vec::with_capacity(worker_count as usize);

        for _ in 0..worker_count {
            let (sender, receiver) = unbounded();
            msg_senders.push(sender);
            msg_receivers.push(receiver);
        }

        let (result_tx, result_rx) = unbounded();

        let brokers = msg_receivers
            .drain(..)
            .map(|receiver| Self {
                workers: msg_senders.clone(),
                result: result_tx.clone(),
                receiver,
            })
            .collect();

        let broker_manager = ChannelBrokerManager { result: result_rx };

        (brokers, broker_manager)
    }
}
