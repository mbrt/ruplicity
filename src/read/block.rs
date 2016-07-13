use signatures::EntryId;

// max size of a block in volume tars
pub const BLOCK_SIZE: usize = 64 * 1024;

// Id for entry block ((path, snapshot), block)
pub type BlockId = (EntryId, usize);
