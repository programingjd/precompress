use highway::{HighwayHash, HighwayHasher, Key};

const VERSION: [u64; 4] = [2024u64, 4u64, 6u64, 1u64];
pub fn hash(data: &[u8]) -> String {
    let hash = HighwayHasher::new(Key(VERSION)).hash128(data);
    format!("{:0>16x}{:0>16x}", hash[0], hash[1])
}
