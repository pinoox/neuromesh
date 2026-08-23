use crate::queue::{dequeue, enqueue};

pub fn process_job(name: &str) -> String {
    let mut q = Vec::new();
    enqueue(name, &mut q);
    dequeue(&mut q).unwrap_or_default()
}
