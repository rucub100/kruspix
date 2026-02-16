// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicBool, Ordering};

use crate::kernel::sync::{OnceLock, SpinLock};

pub mod bcm2835_mbox;

#[derive(Debug, Clone, Copy)]
pub enum MailboxError {
    MailboxFull,
    MailboxEmpty,
    MailboxDisabled,
    MailboxAlreadyExists,
    MailboxNotInitialized,
    MailboxBusy,
}

pub type MailboxResult<T> = Result<T, MailboxError>;

pub trait Mailbox: Send + Sync {
    type Message;

    fn enable(&self);
    fn disable(&self);
    fn ready(&self) -> bool;
    fn send(&self, data: Self::Message) -> MailboxResult<()>;
    fn receive(&self) -> Option<Self::Message>;
    fn queue(&self) -> Arc<SpinLock<VecDeque<Self::Message>>>;
}

pub struct MailboxClient {
    mbox: Arc<dyn Mailbox<Message = u32>>,
    lock: AtomicBool,
}

impl MailboxClient {
    const fn new(mbox: Arc<dyn Mailbox<Message = u32>>) -> Self {
        Self {
            mbox,
            lock: AtomicBool::new(false),
        }
    }

    pub fn send(&self, data: u32) -> MailboxResult<()> {
        if !self.mbox.ready() {
            return Err(MailboxError::MailboxFull);
        }

        self.mbox.send(data)
    }

    pub fn receive_blocking(&self) -> MailboxResult<u32> {
        let queue_arc = self.mbox.queue();

        loop {
            // this will ensure we make progress when interrupts are disabled
            self.mbox.receive();
            let mut queue = queue_arc.lock();
            if let Some(msg) = queue.pop_front() {
                return Ok(msg);
            }
            drop(queue);
            core::hint::spin_loop();
        }
    }
}

static SYSTEM_MAILBOX: OnceLock<MailboxClient> = OnceLock::new();

pub fn register_mailbox(mbox: Arc<dyn Mailbox<Message = u32>>) -> MailboxResult<()> {
    SYSTEM_MAILBOX
        .set(MailboxClient::new(mbox))
        .map_err(|_| MailboxError::MailboxAlreadyExists)
}

pub fn take_mailbox() -> MailboxResult<&'static MailboxClient> {
    let client = SYSTEM_MAILBOX
        .get()
        .ok_or(MailboxError::MailboxNotInitialized)?;

    match client
        .lock
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
    {
        Ok(_) => {
            client.mbox.enable();
            Ok(client)
        }
        Err(_) => Err(MailboxError::MailboxBusy),
    }
}
