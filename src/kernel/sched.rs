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
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::time::Duration;

use crate::arch::cpu::{
    ArchContext, idle_task, local_disable_irq_fiq, load_context, switch_context,
};
use crate::kernel::cpu::get_local_data;
use crate::kernel::irq::register_handler;
use crate::kernel::sync::{OnceLock, SpinLock, without_irq_fiq};
use crate::kprintln;

const KERNEL_STACK_SIZE: usize = 0x4000;
const TASK_QUANTUM: Duration = Duration::from_millis(10);

static NEXT_PID: AtomicUsize = AtomicUsize::new(0);
static NEXT_TID: AtomicUsize = AtomicUsize::new(0);

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

struct Task {
    tid: usize,
    name: String,
    state: TaskState,
    pcb: Option<Arc<ProcessControlBlock>>,
    context: ArchContext,
    kernel_stack: Box<[u8; KERNEL_STACK_SIZE]>,
}

impl Task {
    pub fn new(name: &str, entry: fn()) -> Arc<Self> {
        let tid = NEXT_TID.fetch_add(1, Ordering::SeqCst);
        let kernel_stack = Box::new([0u8; KERNEL_STACK_SIZE]);
        let context = ArchContext::new(&kernel_stack, entry, terminate_task);

        Arc::new(Self {
            tid,
            name: name.to_string(),
            state: TaskState::New,
            pcb: None,
            context,
            kernel_stack,
        })
    }
}

struct Scheduler {
    new_queue: SpinLock<VecDeque<Arc<Task>>>,
    ready_queue: SpinLock<VecDeque<Arc<Task>>>,
    terminated_queue: SpinLock<VecDeque<Arc<Task>>>,
    current_task: SpinLock<Arc<Task>>,
    idle_task: Arc<Task>,
}

static SCHEDULER: OnceLock<Scheduler> = OnceLock::new();

impl Scheduler {
    fn new() -> Self {
        let idle_task = Task::new("idle_task", idle_task);

        Self {
            new_queue: SpinLock::new(VecDeque::new()),
            ready_queue: SpinLock::new(VecDeque::new()),
            terminated_queue: SpinLock::new(VecDeque::new()),
            current_task: SpinLock::new(idle_task.clone()),
            idle_task,
        }
    }

    pub fn start(&self) -> ! {
        let first_task = { self.current_task.lock().clone() };
        let context = &first_task.context;

        load_context(context)
    }

    pub fn add_task(&self, task: Arc<Task>) {
        let mut new_queue = self.new_queue.lock();
        new_queue.push_back(task);
    }

    pub fn schedule(&self) {
        // move new tasks to ready queue
        {
            let mut new_queue = self.new_queue.lock();
            let mut ready_queue = self.ready_queue.lock();
            while let Some(task) = new_queue.pop_front() {
                ready_queue.push_back(task);
            }
        }

        // determine the next task to run
        let next_task = { self.ready_queue.lock().pop_front() };

        if next_task.is_none() {
            // no ready tasks, continue with the current task
            return;
        }

        let next_task = next_task.unwrap();

        // put the current task back to the ready queue if it's not the idle task
        let current_task = { self.current_task.lock().clone() };
        if current_task.tid != self.idle_task.tid {
            self.ready_queue.lock().push_back(current_task.clone());
        }

        // update the current task with the next task
        {
            *self.current_task.lock() = next_task.clone();
        }

        switch_context(&current_task.context, &next_task.context);
    }
}

pub fn add_task(name: &str, entry: fn()) {
    if let Some(scheduler) = SCHEDULER.get() {
        let task = Task::new(name, entry);
        scheduler.add_task(task);
    } else {
        kprintln!("[ERROR] Scheduler not initialized");
    }
}

pub fn yield_task() {
    if let Some(scheduler) = SCHEDULER.get() {
        without_irq_fiq(|| {
            scheduler.schedule();
        });
    } else {
        kprintln!("[ERROR] Scheduler not initialized");
    }
}

pub fn terminate_task() {
    todo!()
}

pub fn start_sched() -> ! {
    local_disable_irq_fiq();

    let local = get_local_data();
    kprintln!("[INFO] Starting scheduler on core {}", local.core_id());

    let scheduler = SCHEDULER.get().expect("Scheduler not initialized");
    if let Some(alarm) = local.get_alarm() {
        kprintln!(
            "[INFO] Setting up scheduler timer alarm (VIRQ {})",
            alarm.virq()
        );

        let alarm_cloned = alarm.clone();
        match register_handler(
            alarm.virq(),
            Arc::new(move |_| {
                alarm_cloned.schedule_after(alarm_cloned.duration_to_ticks(TASK_QUANTUM));
                scheduler.schedule();
            }),
        ) {
            Ok(_) => kprintln!("[INFO] Scheduler timer alarm registered successfully"),
            Err(e) => {
                panic!(
                    "[ERROR] Failed to register scheduler alarm handler (VIRQ {}): {:?}",
                    alarm.virq(),
                    e
                );
            }
        }

        alarm.schedule_after(alarm.duration_to_ticks(alarm.min_duration()));
    } else {
        panic!("[ERROR] No local alarm timer available for scheduler");
    }

    scheduler.start()
}

pub(super) fn init() -> Result<(), ()> {
    if SCHEDULER.set(Scheduler::new()).is_err() {
        kprintln!("[WARNING] Scheduler already initialized");
    }

    Ok(())
}
