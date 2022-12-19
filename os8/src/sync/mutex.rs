use super::UPSafeCell;
use crate::task::TaskControlBlock;
use crate::task::{add_task, current_task, current_process};
use crate::task::{block_current_and_run_next, suspend_current_and_run_next};
use alloc::{collections::VecDeque, sync::Arc};

pub trait Mutex: Sync + Send {
    fn lock(&self);
    fn unlock(&self);
}

pub struct MutexSpin {
    locked: UPSafeCell<bool>,
    id: isize,
}

impl MutexSpin {
    pub fn new(id: isize) -> Self {
        Self {
            locked: unsafe { UPSafeCell::new(false) },
            id: id,
        }
    }
}

impl Mutex for MutexSpin {
    fn lock(&self) {
        loop {
            let mut locked = self.locked.exclusive_access();
            if *locked {
                drop(locked);
                suspend_current_and_run_next();
                continue;
            } else {
                *locked = true;
                return;
            }
        }
    }

    fn unlock(&self) {
        let mut locked = self.locked.exclusive_access();
        *locked = false;
    }
}

pub struct MutexBlocking {
    inner: UPSafeCell<MutexBlockingInner>,
    id: isize,
}

pub struct MutexBlockingInner {
    locked: bool,
    wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl MutexBlocking {
    pub fn new(id: isize) -> Self {
        Self {
            inner: unsafe {
                UPSafeCell::new(MutexBlockingInner {
                    locked: false,
                    wait_queue: VecDeque::new(),
                })
            },
            id: id,
        }
    }
}

impl Mutex for MutexBlocking {
    fn lock(&self) {
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        let task = current_task().unwrap();
        let task_inner = task.inner_exclusive_access();
        let tid = task_inner.res.as_ref().unwrap().tid;
        let mut mutex_inner = self.inner.exclusive_access();
        if mutex_inner.locked {
            mutex_inner.wait_queue.push_back(current_task().unwrap());
            drop(mutex_inner);
            drop(task_inner);
            drop(task);
            drop(process_inner);
            drop(process);
            block_current_and_run_next();
        } else {
            process_inner.mutex_allocation[tid][self.id as usize] += 1;
            process_inner.mutex_avaliable[self.id as usize] -= 1;
            process_inner.mutex_need[tid][self.id as usize] -= 1;
            mutex_inner.locked = true;
        }
    }

    fn unlock(&self) {
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        let task = current_task().unwrap();
        let task_inner = task.inner_exclusive_access();
        let tid = task_inner.res.as_ref().unwrap().tid;
        let mut mutex_inner = self.inner.exclusive_access();
        assert!(mutex_inner.locked);
        if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
            add_task(waking_task);
        } else {
            mutex_inner.locked = false;
        }
        process_inner.mutex_allocation[tid][self.id as usize] -= 1;
        process_inner.mutex_avaliable[self.id as usize] += 1;
    }
}
