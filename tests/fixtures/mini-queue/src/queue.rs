pub fn enqueue(item: &str, queue: &mut Vec<String>) {
    queue.push(item.to_string());
}

pub fn dequeue(queue: &mut Vec<String>) -> Option<String> {
    if queue.is_empty() {
        return None;
    }
    Some(queue.remove(0))
}
