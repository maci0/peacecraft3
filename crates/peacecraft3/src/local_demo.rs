//! Menu capabilities from a successfully hydrated local asset transaction.
//! These describe available renderer content, not a detected game release.

use crate::launch::Addon;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct MenuEditions {
    pub roc: bool,
    pub tft: bool,
}

impl MenuEditions {
    pub fn allows(self, addon: Addon) -> bool {
        match addon {
            Addon::Roc => self.roc,
            Addon::Tft => self.tft,
        }
    }

    pub fn preferred(self) -> Option<Addon> {
        if self.tft {
            Some(Addon::Tft)
        } else if self.roc {
            Some(Addon::Roc)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base_content_never_allows_the_expansion() {
        let editions = MenuEditions {
            roc: true,
            tft: false,
        };
        assert_eq!(editions.preferred(), Some(Addon::Roc));
        assert!(editions.allows(Addon::Roc));
        assert!(!editions.allows(Addon::Tft));
    }

    #[test]
    fn empty_content_allows_no_menu() {
        let editions = MenuEditions::default();
        assert_eq!(editions.preferred(), None);
        assert!(!editions.allows(Addon::Roc));
        assert!(!editions.allows(Addon::Tft));
    }

    #[test]
    fn expansion_requires_its_own_validated_content() {
        let editions = MenuEditions {
            roc: false,
            tft: true,
        };
        assert_eq!(editions.preferred(), Some(Addon::Tft));
        assert!(!editions.allows(Addon::Roc));
        assert!(editions.allows(Addon::Tft));
    }
}
