use crate::FamilyId;

/// Unique identifer for a specific font
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct FontId(u64);

impl FontId {
    /// Create a new `FontId` from a `FamilyId` and index.
    #[inline(always)]
    pub(crate) fn new(family_id: FamilyId, index: usize) -> Self {
        // The index is packed into the high 8 bits of the u64 from the FamilyId
        Self((index as u64) << 48 | (family_id.to_u64() & 0x00FFFFFF))
    }

    /// The `FamilyId` of the family that the font belongs to
    #[inline(always)]
    pub fn family_id(self) -> FamilyId {
        FamilyId(self.0 & 0x00FFFFFF)
    }

    /// The index of the font within the family that the font belongs to
    #[inline(always)]
    pub fn index(self) -> usize {
        (self.0 >> 48) as usize
    }
}
