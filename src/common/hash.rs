// SPDX-License-Identifier: MIT
// Copyright (c) 2025 Ruslan Curbanov <info@ruslan-curbanov.de>

const GOLDEN_RATIO_MUL: usize = 0x9e3779b97f4a7c15u64 as usize;

pub trait FibonacciHash {
    fn fibonacci_hash(&self) -> Self;
}

impl FibonacciHash for usize {
    #[inline(always)]
    fn fibonacci_hash(&self) -> Self {
        (self >> 2usize).wrapping_mul(GOLDEN_RATIO_MUL)
    }
}
