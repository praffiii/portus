use std::collections::{HashMap, VecDeque};

use super::ProcessSnapshot;

pub fn descendants_of(root_pid: u32, processes: &[ProcessSnapshot]) -> Vec<u32> {
    let mut children_by_parent: HashMap<u32, Vec<u32>> = HashMap::new();
    for process in processes {
        if let Some(parent_pid) = process.parent_pid {
            children_by_parent
                .entry(parent_pid)
                .or_default()
                .push(process.pid);
        }
    }
    for children in children_by_parent.values_mut() {
        children.sort_unstable();
    }

    let mut descendants = Vec::new();
    let mut pending = VecDeque::from([root_pid]);
    while let Some(parent_pid) = pending.pop_front() {
        if let Some(children) = children_by_parent.get(&parent_pid) {
            descendants.extend(children);
            pending.extend(children);
        }
    }
    descendants
}
