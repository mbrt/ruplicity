use signatures::EntryId;

#[allow(dead_code)]
pub const BLOCK_SIZE: usize = 64 * 1024;

// Id for entry block ((path, snapshot), block)
pub type BlockId = (EntryId, usize);
