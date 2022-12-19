use crate::sync::UPSafeCell;
use crate::task::{add_task, block_current_and_run_next, current_task, TaskControlBlock, current_process};
use alloc::{collections::VecDeque, sync::Arc};

pub struct Semaphore {
    pub inner: UPSafeCell<SemaphoreInner>,
    pub id: isize,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl Semaphore {
    pub fn new(res_count: usize, id: isize) -> Self {
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                })
            },
            id: id,
        }
    }

    pub fn up(&self) {
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        let task = current_task().unwrap();
        let task_inner = task.inner_exclusive_access();
        let tid = task_inner.res.as_ref().unwrap().tid;
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                add_task(task);
            }
        }
        process_inner.semaphore_allocation[tid][self.id as usize] -= 1;
        process_inner.semaphore_avaliable[self.id as usize] += 1;
    }

    pub fn down(&self) {
        let process = current_process();
        let mut process_inner = process.inner_exclusive_access();
        let task = current_task().unwrap();
        let task_inner = task.inner_exclusive_access();
        let tid = task_inner.res.as_ref().unwrap().tid;
        drop(task_inner);
        drop(task);
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner); 
            drop(process_inner);
            drop(process);
            block_current_and_run_next();
        } else {
            process_inner.semaphore_need[tid][self.id as usize] -= 1;
            process_inner.semaphore_allocation[tid][self.id as usize] += 1;
            process_inner.semaphore_avaliable[self.id as usize] -= 1;
        }
    }
}
