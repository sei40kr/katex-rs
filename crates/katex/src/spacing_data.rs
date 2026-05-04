//! Inter-atom spacing tables. Mirrors upstream `spacingData.ts`:
//! [`SPACINGS`] for display / text style, [`TIGHT_SPACINGS`] for
//! script / scriptscript. `None` entries mean no inter-atom space.
//!
//! **Deviation from upstream.** Upstream uses partial JS objects keyed
//! by atom-class strings; we emit a fixed 8×8 array indexed by
//! [`AtomClass`] for `O(1)` lookup with no hash on the parser hot
//! path.

use crate::types::AtomClass;
use crate::units::{Measurement, Unit};

pub struct SpacingTable {
    pub rows: [[Option<Measurement>; 8]; 8],
}

impl SpacingTable {
    pub const fn get(&self, left: AtomClass, right: AtomClass) -> Option<Measurement> {
        self.rows[left as usize][right as usize]
    }
}

include!(concat!(env!("OUT_DIR"), "/spacing_data.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mord_mop_is_thinspace() {
        let m = SPACINGS.get(AtomClass::MOrd, AtomClass::MOp).unwrap();
        assert_eq!(m.unit, Unit::Mu);
        assert_eq!(m.number, 3.0);
    }

    #[test]
    fn mord_mbin_is_mediumspace() {
        let m = SPACINGS.get(AtomClass::MOrd, AtomClass::MBin).unwrap();
        assert_eq!(m.number, 4.0);
    }

    #[test]
    fn mopen_has_no_outgoing_spaces() {
        for right in [
            AtomClass::MOrd,
            AtomClass::MOp,
            AtomClass::MBin,
            AtomClass::MRel,
            AtomClass::MOpen,
            AtomClass::MClose,
            AtomClass::MPunct,
            AtomClass::MInner,
        ] {
            assert!(
                SPACINGS.get(AtomClass::MOpen, right).is_none(),
                "mopen→{right:?} should be None"
            );
        }
    }

    #[test]
    fn tight_spacings_drop_most_entries() {
        for right in [
            AtomClass::MOrd,
            AtomClass::MOp,
            AtomClass::MBin,
            AtomClass::MRel,
        ] {
            assert!(TIGHT_SPACINGS.get(AtomClass::MBin, right).is_none());
            assert!(TIGHT_SPACINGS.get(AtomClass::MRel, right).is_none());
        }
        assert_eq!(
            TIGHT_SPACINGS
                .get(AtomClass::MOrd, AtomClass::MOp)
                .unwrap()
                .number,
            3.0
        );
    }
}
