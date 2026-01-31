// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

//! Kernel task scheduler
//!
//! Brainstorming ideas and concepts:
//! - Round-robin scheduling / first-come-first-serve
//! - we need a one or multiple queues to hold the tasks
//! - what are the possible states of tasks?
//! - context switching (arch specific and arch agnostic parts)
//! - time slicing and preemption (alarm timer interrupts)
//! - the dispatcher selects the next task to run and performs the context switch
//! - in contrast the scheduler manages the ready queue but does not perform context switches
//! - what about the IO scheduler?

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;

use crate::arch::cpu::ArchContext;
use crate::kernel::sync::SpinLock;

const KERNEL_STACK_SIZE: usize = 0x10000;

struct ProcessControlBlock {
    pid: usize,
    tasks: Vec<Arc<Task>>,
    parent: Option<Arc<ProcessControlBlock>>,
    children: Vec<Arc<ProcessControlBlock>>,
    // open files
    // io devices and other resources
    // memory management info
    // scheduling info
}

static PROCESS_TABLE: SpinLock<Vec<ProcessControlBlock>> = SpinLock::new(Vec::new());

enum TaskState {
    New,
    Ready,
    Running,
    Waiting,
    Terminated,
}

pub struct Task {
    tid: usize,
    state: TaskState,
    pcb: Option<Arc<ProcessControlBlock>>,
    context: ArchContext,
    kernel_stack: Box<[u8; KERNEL_STACK_SIZE]>,
}

pub struct Scheduler {
    new_queue: SpinLock<VecDeque<Arc<Task>>>,
    ready_queue: SpinLock<VecDeque<Arc<Task>>>,
    terminated_queue: SpinLock<VecDeque<Arc<Task>>>,
}

pub const SCHEDULER: Scheduler = Scheduler::new();

impl Scheduler {
    const fn new() -> Self {
        Scheduler {
            new_queue: SpinLock::new(VecDeque::new()),
            ready_queue: SpinLock::new(VecDeque::new()),
            terminated_queue: SpinLock::new(VecDeque::new()),
        }
    }

    pub fn add_task(&self, task: Arc<Task>) {
        let mut new_queue = self.new_queue.lock();
        new_queue.push_back(task);
    }
}

pub(super) fn init() -> Result<(), ()> {
    // Initialize the scheduler here
    // e.g., set up timer interrupts for preemption
    Ok(())
}