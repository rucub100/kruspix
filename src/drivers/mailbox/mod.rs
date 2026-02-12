// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Ruslan Curbanov <info@ruslan-curbanov.de>

use crate::common::ring_array::RingArray;
use crate::kernel::sync::{OnceLock, SpinLock, SpinLockGuard};
use alloc::collections::VecDeque;
use alloc::sync::Arc;
use alloc::vec::Vec;

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
    fn send(&self, data: Self::Message) -> MailboxResult<()>;
    fn ready(&self) -> bool;
}

pub struct MailboxClient {
    mbox: Arc<dyn Mailbox<Message = u32>>,
    // SAFETY: lock_irq() must be held when accessing this buffer
    buffer: Arc<SpinLock<VecDeque<u32>>>,
}

impl MailboxClient {
    const fn new(
        mbox: Arc<dyn Mailbox<Message = u32>>,
        buffer: Arc<SpinLock<VecDeque<u32>>>,
    ) -> Self {
        Self { mbox, buffer }
    }

    pub fn send(&self, data: u32) -> MailboxResult<()> {
        if !self.mbox.ready() {
            return Err(MailboxError::MailboxFull);
        }

        self.mbox.send(data)
    }

    // FIXME: revisit this API once we have actual clients using it
    pub fn try_receive(&self) -> MailboxResult<u32> {
        let buffer = self.buffer.try_lock_irq();

        if let Some(mut buffer) = buffer {
            let msg = buffer.pop_front();
            msg.ok_or(MailboxError::MailboxEmpty)
        } else {
            Err(MailboxError::MailboxBusy)
        }
    }
}

static SYSTEM_MAILBOX: OnceLock<SpinLock<MailboxClient>> = OnceLock::new();

pub fn register_mailbox(
    mbox: Arc<dyn Mailbox<Message = u32>>,
    buffer: Arc<SpinLock<VecDeque<u32>>>,
) -> MailboxResult<()> {
    SYSTEM_MAILBOX
        .set(SpinLock::new(MailboxClient::new(mbox, buffer)))
        .map_err(|_| MailboxError::MailboxAlreadyExists)
}

pub fn lock_mailbox() -> MailboxResult<SpinLockGuard<'static, MailboxClient>> {
    let guard = SYSTEM_MAILBOX
        .get()
        .ok_or(MailboxError::MailboxNotInitialized)?
        .lock_irq();

    Ok(guard)
}

pub fn try_lock_mailbox() -> MailboxResult<SpinLockGuard<'static, MailboxClient>> {
    SYSTEM_MAILBOX
        .get()
        .ok_or(MailboxError::MailboxNotInitialized)?
        .try_lock_irq()
        .ok_or(MailboxError::MailboxBusy)
}
