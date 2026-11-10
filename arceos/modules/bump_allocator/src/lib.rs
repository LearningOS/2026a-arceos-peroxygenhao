#![no_std]

use allocator::{AllocError, AllocResult, BaseAllocator, ByteAllocator, PageAllocator};
use core::alloc::Layout;
use core::ptr::NonNull;

/// Early memory allocator
/// Use it before formal bytes-allocator and pages-allocator can work!
/// This is a double-end memory range:
/// - Alloc bytes forward
/// - Alloc pages backward
///
/// [ bytes-used | avail-area | pages-used ]
/// |            | -->    <-- |            |
/// start       b_pos        p_pos       end
///
/// For bytes area, 'count' records number of allocations.
/// When it goes down to ZERO, free bytes-used area.
/// For pages area, it will never be freed!
pub struct EarlyAllocator<const SIZE: usize> {
    /// Start of the managed memory region.
    start: usize,
    /// End of the managed memory region.
    end: usize,
    /// Current position of the byte area, it grows upwards from `start`.
    b_pos: usize,
    /// Current position of the page area, it grows downwards from `end`.
    p_pos: usize,
    /// Number of live byte allocations. The byte area can be rewound only when
    /// every one of them has been released.
    count: usize,
}

impl<const SIZE: usize> EarlyAllocator<SIZE> {
    pub const fn new() -> Self {
        Self {
            start: 0,
            end: 0,
            b_pos: 0,
            p_pos: 0,
            count: 0,
        }
    }

    /// Rounds `pos` down to a multiple of `align` (`align` must be a power of 2).
    const fn align_down(pos: usize, align: usize) -> usize {
        pos & !(align - 1)
    }

    /// Rounds `pos` up to a multiple of `align` (`align` must be a power of 2).
    const fn align_up(pos: usize, align: usize) -> usize {
        (pos + align - 1) & !(align - 1)
    }

    /// Size of the memory region that is still untouched.
    const fn free_bytes(&self) -> usize {
        self.p_pos.saturating_sub(self.b_pos)
    }
}

impl<const SIZE: usize> Default for EarlyAllocator<SIZE> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const SIZE: usize> BaseAllocator for EarlyAllocator<SIZE> {
    fn init(&mut self, start: usize, size: usize) {
        assert!(SIZE.is_power_of_two(), "page size must be a power of two");
        // Pages are handed out from the top, so the top must be page aligned.
        // The bottom is aligned on demand when a byte allocation happens.
        let end = Self::align_down(start + size, SIZE);
        assert!(end > start, "memory region is smaller than one page");
        self.start = start;
        self.end = end;
        self.b_pos = start;
        self.p_pos = end;
        self.count = 0;
    }

    fn add_memory(&mut self, start: usize, size: usize) -> AllocResult {
        // A bump allocator owns a single contiguous range, so a new region can
        // only be accepted when it directly extends that range. Anything else
        // is reported back as unusable rather than being silently merged into
        // the wrong place.
        let end = Self::align_down(start + size, SIZE);
        if start == self.end && end > start {
            // Append at the high end.
            self.end = end;
            self.p_pos = end;
            Ok(())
        } else if end == self.start && start < end {
            // Prepend at the low end.
            self.start = start;
            self.b_pos = start;
            Ok(())
        } else {
            Err(AllocError::MemoryOverlap)
        }
    }
}

impl<const SIZE: usize> ByteAllocator for EarlyAllocator<SIZE> {
    fn alloc(&mut self, layout: Layout) -> AllocResult<NonNull<u8>> {
        let align = layout.align().max(1);
        let start = Self::align_up(self.b_pos, align);
        let end = start
            .checked_add(layout.size())
            .ok_or(AllocError::InvalidParam)?;
        // The byte area may not run into the page area.
        if end > self.p_pos {
            return Err(AllocError::NoMemory);
        }
        self.b_pos = end;
        self.count += 1;
        NonNull::new(start as *mut u8).ok_or(AllocError::InvalidParam)
    }

    fn dealloc(&mut self, _pos: NonNull<u8>, _layout: Layout) {
        // Freeing a single byte block is not supported (the bump pointer can
        // only move backwards). Instead, all the bytes that were handed out are
        // reclaimed once the last allocation has been released.
        if self.count > 0 {
            self.count -= 1;
            if self.count == 0 {
                self.b_pos = self.start;
            }
        }
    }

    fn total_bytes(&self) -> usize {
        self.end - self.start
    }

    fn used_bytes(&self) -> usize {
        self.b_pos - self.start
    }

    fn available_bytes(&self) -> usize {
        self.free_bytes()
    }
}

impl<const SIZE: usize> PageAllocator for EarlyAllocator<SIZE> {
    const PAGE_SIZE: usize = SIZE;

    fn alloc_pages(&mut self, num_pages: usize, align_pow2: usize) -> AllocResult<usize> {
        if num_pages == 0 {
            return Err(AllocError::InvalidParam);
        }
        if align_pow2 != 0 && (!align_pow2.is_power_of_two() || align_pow2 < SIZE) {
            return Err(AllocError::InvalidParam);
        }
        let align = align_pow2.max(SIZE);
        let size = num_pages
            .checked_mul(SIZE)
            .ok_or(AllocError::InvalidParam)?;

        // Pages grow downwards from `end`, and the returned start address must
        // satisfy the requested alignment.
        let start = Self::align_down(
            self.p_pos.checked_sub(size).ok_or(AllocError::NoMemory)?,
            align,
        );
        // Keep the page area above the byte area.
        if start < self.b_pos || start + size > self.p_pos {
            return Err(AllocError::NoMemory);
        }
        self.p_pos = start;
        Ok(start)
    }

    fn dealloc_pages(&mut self, pos: usize, num_pages: usize) {
        // Only the most recently allocated pages can be given back, because the
        // bump pointer is the only bookkeeping this allocator keeps.
        let size = num_pages.saturating_mul(SIZE);
        if size != 0 && pos.saturating_add(size) == self.p_pos {
            self.p_pos = pos;
        }
    }

    fn total_pages(&self) -> usize {
        (self.end - self.start) / SIZE
    }

    fn used_pages(&self) -> usize {
        (self.end - self.p_pos) / SIZE
    }

    fn available_pages(&self) -> usize {
        (self.p_pos - self.b_pos) / SIZE
    }
}
