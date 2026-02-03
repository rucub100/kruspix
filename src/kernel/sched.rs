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

use crate::arch::cpu::{
    ArchContext, idle_task, local_disable_interrupts, local_disable_irq_fiq, switch_context,
};
use crate::kernel::cpu::get_local_data;
use crate::kernel::irq::register_handler;
use crate::kernel::sync::{OnceLock, SpinLock, without_irq_fiq};
use crate::kprintln;
use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::AtomicUsize;
use core::time::Duration;

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
    state: TaskState,
    pcb: Option<Arc<ProcessControlBlock>>,
    context: ArchContext,
    kernel_stack: Box<[u8; KERNEL_STACK_SIZE]>,
}

impl Task {
    pub fn new(entry: fn()) -> Arc<Self> {
        let tid = NEXT_TID.fetch_add(1, core::sync::atomic::Ordering::SeqCst);
        let kernel_stack = Box::new([0u8; KERNEL_STACK_SIZE]);
        let context = ArchContext::new(&kernel_stack, entry, terminate_task);

        Arc::new(Self {
            tid,
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
    current_task: SpinLock<Option<Arc<Task>>>,
    idle_task: Arc<Task>,
}

static SCHEDULER: OnceLock<Scheduler> = OnceLock::new();

impl Scheduler {
    fn new() -> Self {
        Scheduler {
            new_queue: SpinLock::new(VecDeque::new()),
            ready_queue: SpinLock::new(VecDeque::new()),
            terminated_queue: SpinLock::new(VecDeque::new()),
            current_task: SpinLock::new(None),
            idle_task: Task::new(idle_task),
        }
    }

    pub fn start(&self) -> ! {
        let mut boot_context = ArchContext::default();

        let first_task = {
            let mut ready_q = self.ready_queue.lock();
            ready_q.pop_front().unwrap_or(self.idle_task.clone())
        };

        self.current_task.lock().replace(first_task.clone());

        unsafe { switch_context(&mut boot_context, &first_task.context) }
    }

    pub fn add_task(&self, task: Arc<Task>) {
        let mut new_queue = self.new_queue.lock();
        new_queue.push_back(task);
    }

    pub fn schedule(&self) {
        // check if new tasks are available (and move to ready queue)
        // determine the next task to run
        // put the current task back to the ready queue
        // update the current task
        // call context switch
        // resume when scheduler decide to switch back
        todo!()
    }
}

pub fn add_task(entry: fn()) {
    if let Some(scheduler) = SCHEDULER.get() {
        let task = Task::new(entry);
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
