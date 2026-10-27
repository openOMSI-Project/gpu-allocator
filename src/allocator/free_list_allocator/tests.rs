#![allow(clippy::unwrap_used)]

use super::*;
use core::num::NonZeroU64;

fn allocate(
    allocator: &mut FreeListAllocator,
    size: u64,
    alignment: u64,
    kind: AllocationType,
    granularity: u64,
) -> Result<(u64, NonZeroU64)> {
    allocator.allocate(
        size,
        alignment,
        kind,
        granularity,
        "test",
        #[cfg(feature = "std")]
        Arc::new(Backtrace::disabled()),
    )
}

fn check_partition(allocator: &FreeListAllocator) {
    let mut chunks: Vec<_> = allocator.chunks.values().collect();
    chunks.sort_unstable_by_key(|chunk| chunk.offset);
    let mut end = 0;
    let mut allocated = 0;
    for (index, chunk) in chunks.iter().enumerate() {
        assert_eq!(
            chunk.offset, end,
            "chunks must cover the block without gaps or overlaps"
        );
        assert_ne!(chunk.size, 0);
        assert_eq!(chunk.prev, index.checked_sub(1).map(|i| chunks[i].chunk_id));
        assert_eq!(chunk.next, chunks.get(index + 1).map(|c| c.chunk_id));
        let free = chunk.allocation_type == AllocationType::Free;
        assert_eq!(allocator.free_chunks.contains(&chunk.chunk_id), free);
        if !free {
            allocated += chunk.size;
        }
        end += chunk.size;
    }
    assert_eq!(end, allocator.size);
    assert_eq!(allocated, allocator.allocated);
    assert_eq!(
        allocator.free_chunks.len(),
        chunks
            .iter()
            .filter(|c| c.allocation_type == AllocationType::Free)
            .count()
    );
}

#[test]
fn aligned_allocations_do_not_overlap_and_release_the_whole_block() {
    let mut allocator = FreeListAllocator::new(8192);
    let mut live = Vec::new();
    for (size, alignment) in [(3, 1), (17, 64), (255, 256), (9, 1024)] {
        let (offset, id) =
            allocate(&mut allocator, size, alignment, AllocationType::Linear, 1).unwrap();
        assert_eq!(offset % alignment, 0);
        for &(other, length, _) in &live {
            assert!(offset + size <= other || other + length <= offset);
        }
        live.push((offset, size, id));
        check_partition(&allocator);
    }
    for index in [1, 3, 0, 2] {
        allocator.free(Some(live[index].2)).unwrap();
        check_partition(&allocator);
    }
    assert_eq!(allocator.chunks.len(), 1);
    assert_eq!(
        allocate(&mut allocator, 8192, 1, AllocationType::Linear, 1)
            .unwrap()
            .0,
        0
    );
}

#[test]
fn conflicting_resources_use_separate_granularity_pages() {
    let mut allocator = FreeListAllocator::new(4096);
    let (a, aid) = allocate(&mut allocator, 64, 16, AllocationType::Linear, 512).unwrap();
    let (b, bid) = allocate(&mut allocator, 64, 16, AllocationType::NonLinear, 512).unwrap();
    let (c, cid) = allocate(&mut allocator, 64, 16, AllocationType::Linear, 512).unwrap();
    assert_ne!((a + 63) / 512, b / 512);
    assert_ne!((b + 63) / 512, c / 512);
    check_partition(&allocator);
    for id in [bid, aid, cid] {
        allocator.free(Some(id)).unwrap();
        check_partition(&allocator);
    }
    assert_eq!(allocator.chunks.len(), 1);
    assert_eq!(allocator.allocated, 0);
}

#[test]
fn bounded_churn_preserves_accounting_and_coalesces_free_space() {
    let mut allocator = FreeListAllocator::new(16384);
    let mut live = Vec::new();
    let mut state = 0x1234_5678u32;
    let mut failures = 0;
    for _ in 0..4096 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        if !live.is_empty() && state & 3 == 0 {
            let index = state as usize % live.len();
            allocator.free(Some(live.swap_remove(index))).unwrap();
        } else {
            let size = 1 + u64::from((state >> 8) & 1023);
            let alignment = 1 << ((state >> 20) & 7);
            match allocate(&mut allocator, size, alignment, AllocationType::Linear, 1) {
                Ok((offset, id)) => {
                    assert_eq!(offset % alignment, 0);
                    live.push(id);
                }
                Err(AllocationError::OutOfMemory) => failures += 1,
                Err(error) => panic!("unexpected allocator error: {error}"),
            }
        }
        check_partition(&allocator);
    }
    assert!(
        failures > 0,
        "the workload must exercise allocation failure"
    );
    for id in live {
        allocator.free(Some(id)).unwrap();
        check_partition(&allocator);
    }
    assert_eq!(allocator.chunks.len(), 1);
    assert_eq!(allocator.allocated, 0);
    assert_eq!(
        allocate(&mut allocator, 16384, 1, AllocationType::Linear, 1)
            .unwrap()
            .0,
        0
    );
}
