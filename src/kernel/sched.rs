// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::boxed::Box;
use alloc::collections::VecDeque;
use alloc::string::{String, ToString};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicUsize, Ordering};
use core::time::Duration;

use crate::arch::cpu::{
    ArchContext, load_context, local_disable_irq_fiq, switch_context, wait_for_interrupt,
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

#[derive(PartialEq, Eq)]
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
    state: SpinLock<TaskState>,
    pcb: Option<Arc<ProcessControlBlock>>,
    context: ArchContext,
    kernel_stack: Box<[u8]>,
}

impl Task {
    fn new(name: &str, entry: fn()) -> Arc<Self> {
        let tid = NEXT_TID.fetch_add(1, Ordering::SeqCst);
        let kernel_stack = vec![0u8; KERNEL_STACK_SIZE].into_boxed_slice();
        let context = ArchContext::new(&kernel_stack, entry, terminate_task);

        Arc::new(Self {
            tid,
            name: name.to_string(),
            state: SpinLock::new(TaskState::New),
            pcb: None,
            context,
            kernel_stack,
        })
    }

    fn has_state(&self, state: TaskState) -> bool {
        *self.state.lock() == state
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

    fn start(&self) -> ! {
        let first_task = { self.current_task.lock().clone() };
        *first_task.state.lock() = TaskState::Running;
        let context = &first_task.context;

        load_context(context)
    }

    fn add_task(&self, name: &str, entry: fn()) {
        let mut new_queue = self.new_queue.lock();
        new_queue.push_back(Task::new(name, entry));
    }

    fn exit_current(&self) {
        let current_task = { self.current_task.lock().clone() };
        *current_task.state.lock() = TaskState::Terminated;
        self.terminated_queue.lock().push_back(current_task);
        self.schedule();
    }

    fn schedule(&self) {
        self._prepare_new_tasks();

        let current_task = { self.current_task.lock().clone() };
        let current_may_continue = current_task.has_state(TaskState::Running);

        // determine the next task to run
        let mut next_task = { self.ready_queue.lock().pop_front() };
        if next_task.is_none() {
            if current_may_continue {
                return;
            }

            next_task = Some(self.idle_task.clone());
        }
        let next_task = next_task.unwrap();

        // put the current task back to the ready queue if it's not the idle task
        if current_may_continue && current_task.tid != self.idle_task.tid {
            self.ready_queue.lock().push_back(current_task.clone());
        }

        // update the current task with the next task
        {
            *self.current_task.lock() = next_task.clone();
        }

        if current_may_continue {
            *current_task.state.lock() = TaskState::Ready;
        }
        *next_task.state.lock() = TaskState::Running;

        switch_context(&current_task.context, &next_task.context);
    }

    #[inline(always)]
    fn _prepare_new_tasks(&self) {
        let mut new_queue = self.new_queue.lock();
        let mut ready_queue = self.ready_queue.lock();
        while let Some(task) = new_queue.pop_front() {
            *task.state.lock() = TaskState::Ready;
            ready_queue.push_back(task);
        }
    }
}

#[inline(never)]
fn idle_task() {
    loop {
        wait_for_interrupt();
        yield_task();
    }
}

#[inline]
pub fn add_task(name: &str, entry: fn()) {
    if let Some(scheduler) = SCHEDULER.get() {
        let task = Task::new(name, entry);
        without_irq_fiq(|| {
            scheduler.add_task(name, entry);
        });
    } else {
        panic!("[ERROR] Scheduler not initialized");
    }
}

#[inline]
pub fn yield_task() {
    if let Some(scheduler) = SCHEDULER.get() {
        without_irq_fiq(|| {
            scheduler.schedule();
        });
    } else {
        panic!("[ERROR] Scheduler not initialized");
    }
}

#[inline]
pub fn terminate_task() -> ! {
    if let Some(scheduler) = SCHEDULER.get() {
        without_irq_fiq(|| {
            scheduler.exit_current();
        });
    } else {
        panic!("[ERROR] Scheduler not initialized");
    }

    unreachable!()
}

#[inline]
pub fn exit() -> ! {
    terminate_task()
}

#[inline]
pub fn task_id() -> usize {
    if let Some(scheduler) = SCHEDULER.get() {
        without_irq_fiq(|| {
            let current_task = scheduler.current_task.lock();
            current_task.tid
        })
    } else {
        0
    }
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
                get_local_data().set_schedule_flag();
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
