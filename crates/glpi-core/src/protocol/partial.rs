// SPDX-License-Identifier: GPL-2.0-only

//! Category selection for inventories.
//!
//! Two configuration knobs shape which [`InventoryCategory`]s a run collects:
//!
//! - `no-category` — categories to drop, and
//! - `required-category` — categories that must always be present (notably in a
//!   *partial* inventory, where the server requests a subset).
//!
//! [`select_categories`] combines them with a deterministic, order-preserving
//! result so the same inputs always produce the same submission.

use crate::types::InventoryCategory;

/// Returns the categories to collect, given the candidate set and the
/// `no-category` / `required-category` filters.
///
/// Rules:
/// - the candidate order is preserved;
/// - a candidate is dropped if it is in `no_category`, **unless** it is also in
///   `required` (required wins over exclusion);
/// - any `required` category not already present is appended, in `required`
///   order.
#[must_use]
pub fn select_categories(
    candidates: &[InventoryCategory],
    no_category: &[InventoryCategory],
    required: &[InventoryCategory],
) -> Vec<InventoryCategory> {
    let mut selected: Vec<InventoryCategory> = candidates
        .iter()
        .copied()
        .filter(|c| required.contains(c) || !no_category.contains(c))
        .collect();

    for &req in required {
        if !selected.contains(&req) {
            selected.push(req);
        }
    }

    selected
}

#[cfg(test)]
mod tests {
    use super::select_categories;
    use crate::types::InventoryCategory as Cat;

    #[test]
    fn excludes_no_category() {
        let out = select_categories(
            &[Cat::Hardware, Cat::Software, Cat::Network],
            &[Cat::Software],
            &[],
        );
        assert_eq!(out, vec![Cat::Hardware, Cat::Network]);
    }

    #[test]
    fn required_overrides_exclusion() {
        let out = select_categories(
            &[Cat::Hardware, Cat::Software],
            &[Cat::Software],
            &[Cat::Software],
        );
        assert_eq!(out, vec![Cat::Hardware, Cat::Software]);
    }

    #[test]
    fn required_is_appended_when_missing() {
        let out = select_categories(&[Cat::Hardware], &[], &[Cat::Os]);
        assert_eq!(out, vec![Cat::Hardware, Cat::Os]);
    }

    #[test]
    fn preserves_candidate_order_without_duplicates() {
        let out = select_categories(&[Cat::Cpu, Cat::Memory, Cat::Cpu], &[], &[Cat::Memory]);
        assert_eq!(out, vec![Cat::Cpu, Cat::Memory, Cat::Cpu]);
    }
}
