use std::{
    collections::VecDeque,
    sync::{Barrier, Mutex},
    thread::ThreadId,
};

static WORK_QUEUE: Mutex<VecDeque<WorkItem>> = Mutex::new(VecDeque::new());

struct WorkItem {
    #[cfg(debug_assertions)]
    issuer: ThreadId,
    kind: WorkItemKind,
}

enum WorkItemKind {
    Blocking(Barrier),
    Send(),
}
