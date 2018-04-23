use std::mem::transmute;
use std::fs;
use std::path::Path;
use persistence::*;
use type_info::TypeInfo;
use half::f16;
use std;

use heapsize::HeapSizeOf;

use memmap::Mmap;
use memmap::MmapOptions;
use util::*;

pub mod token_to_anchor_score;
pub mod token_to_anchor_score_vint;

pub use self::token_to_anchor_score::*;
pub use self::token_to_anchor_score_vint::*;

const U31_MAX: u32 = (1 << 31) - 1;

const SIZE_OF_ANCHOR_SCORE: usize = std::mem::size_of::<AnchorScore>();
const SIZE_OF_NUM_ELEM: usize = std::mem::size_of::<(u32)>();


struct CompactHit {
    id: [u8; 3],
    score: u8,
}
impl CompactHit {
    pub fn new(id: u32, score: u8) -> Self {
        let bytes: [u8; 4] = unsafe { transmute(id) };
        let id: [u8; 3] = [bytes[0], bytes[1], bytes[2]];
        CompactHit { id, score }
    }
    pub fn get_id(&self) -> u32 {
        let bytes: [u8; 4] = [self.id[0], self.id[1], self.id[2], 0];
        unsafe { transmute(bytes) }
    }
}
#[test]
fn test_compact_hit() {
    let hit = CompactHit::new(100, 1);
    assert_eq!(hit.get_id(), 100);
    assert_eq!(hit.score, 1);
}

#[repr(packed)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnchorScore {
    pub id: u32,
    pub score: f16,
}
impl AnchorScore {
    pub fn new(id: u32, score: f16) -> AnchorScore {
        AnchorScore { id: id, score: score }
    }
}

#[repr(packed)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnchorScoreSerialize {
    pub id: u32,
    pub score: u16,
}

impl AnchorScoreSerialize {
    pub fn new(id: u32, score: u16) -> AnchorScoreSerialize {
        AnchorScoreSerialize { id: id, score: score }
    }
}

