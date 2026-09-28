//! Shared pagination helper for Soroban contract list queries.
//!
//! Provides a single, tested implementation of offset+limit slicing over a
//! `soroban_sdk::Vec<T>` so that every contract in the workspace uses identical
//! cursor-handling semantics, and we can test the edge cases in one place.
//!
//! # Usage
//!
//! ```rust,ignore
//! use brain_storm_shared::pagination::paginate;
//!
//! let page = paginate(&env, &my_vec, offset, limit);
//! ```
//!
//! # Page-size cap (Issue #1168)
//!
//! Callers may ask for an unbounded number of items, which would exceed the
//! contract resource limits. `paginate` therefore clamps every request to
//! [`MAX_PAGE_SIZE`] centrally, so every consumer of this helper is capped
//! without having to remember to do it itself.
//!
//! # Edge cases handled
//!
//! | Scenario | Behaviour |
//! |---|---|
//! | `offset` ≥ `total` | Returns empty `Vec` |
//! | `offset + limit` > `total` | Clamps to `total` (partial page) |
//! | `limit` > `MAX_PAGE_SIZE` | Truncated to `MAX_PAGE_SIZE` (Issue #1168) |
//! | `limit == 0` | Returns empty `Vec` |
//! | Empty source `Vec` | Returns empty `Vec` |

use soroban_sdk::{Env, Vec};

/// Maximum number of items a single page may contain (Issue #1168).
///
/// This is the workspace-wide page-size cap: callers cannot request more than
/// `MAX_PAGE_SIZE` items in one call, no matter what `limit` they pass.
/// [`paginate`] enforces it through [`clamp_page_size`].
pub const MAX_PAGE_SIZE: u32 = 100;

/// Clamp a requested page size to [`MAX_PAGE_SIZE`].
///
/// Over-limit requests are *truncated* rather than rejected, so existing
/// callers keep working while still being bounded (Issue #1168).
pub fn clamp_page_size(limit: u32) -> u32 {
    limit.min(MAX_PAGE_SIZE)
}

/// Return a page of items from `list` starting at `offset` with at most `limit` items.
///
/// All four cursor-boundary cases are handled safely (see module-level docs),
/// and `limit` is capped at [`MAX_PAGE_SIZE`] before slicing (Issue #1168).
pub fn paginate<T: soroban_sdk::TryFromVal<Env, soroban_sdk::Val> + soroban_sdk::IntoVal<Env, soroban_sdk::Val> + Clone>(
    env: &Env,
    list: &Vec<T>,
    offset: u32,
    limit: u32,
) -> Vec<T> {
    // Central page-size enforcement (#1168): no caller can ask for more than
    // MAX_PAGE_SIZE items in a single page.
    let limit = clamp_page_size(limit);
    let total = list.len();
    let start = offset.min(total);
    let end = (offset.checked_add(limit).unwrap_or(total)).min(total);

    let mut page = Vec::new(env);
    let mut i = start;
    while i < end {
        page.push_back(list.get(i).unwrap());
        i += 1;
    }
    page
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::Env;

    fn make_list(env: &Env, n: u32) -> Vec<u32> {
        let mut v: Vec<u32> = Vec::new(env);
        for i in 0..n {
            v.push_back(i);
        }
        v
    }

    // ── empty source ─────────────────────────────────────────────────────────

    #[test]
    fn test_paginate_empty_list_returns_empty() {
        let env = Env::default();
        let list: Vec<u32> = Vec::new(&env);
        let page = paginate(&env, &list, 0, 10);
        assert_eq!(page.len(), 0);
    }

    // ── zero limit ───────────────────────────────────────────────────────────

    #[test]
    fn test_paginate_zero_limit_returns_empty() {
        let env = Env::default();
        let list = make_list(&env, 5);
        let page = paginate(&env, &list, 0, 0);
        assert_eq!(page.len(), 0);
    }

    // ── page-size cap (#1168) ─────────────────────────────────────────────────

    #[test]
    fn test_clamp_page_size_caps_at_max_page_size() {
        assert_eq!(clamp_page_size(0), 0);
        assert_eq!(clamp_page_size(MAX_PAGE_SIZE), MAX_PAGE_SIZE);
        assert_eq!(clamp_page_size(MAX_PAGE_SIZE + 1), MAX_PAGE_SIZE);
        assert_eq!(clamp_page_size(u32::MAX), MAX_PAGE_SIZE);
    }

    #[test]
    fn test_paginate_over_limit_is_truncated_to_max_page_size() {
        let env = Env::default();
        let list = make_list(&env, 500);
        // Unbounded request — the classic resource-limit attack.
        let page = paginate(&env, &list, 0, u32::MAX);
        assert_eq!(page.len(), MAX_PAGE_SIZE);
        assert_eq!(page.get(0).unwrap(), 0_u32);
        assert_eq!(page.get(MAX_PAGE_SIZE - 1).unwrap(), MAX_PAGE_SIZE - 1);
    }

    #[test]
    fn test_paginate_exactly_max_page_size_is_not_truncated() {
        let env = Env::default();
        let list = make_list(&env, 150);
        let page = paginate(&env, &list, 0, MAX_PAGE_SIZE);
        assert_eq!(page.len(), MAX_PAGE_SIZE);
    }

    #[test]
    fn test_paginate_over_limit_on_small_list_clamps_to_available() {
        let env = Env::default();
        let list = make_list(&env, 5);
        let page = paginate(&env, &list, 0, 10_000);
        assert_eq!(page.len(), 5);
    }

    #[test]
    fn test_paginate_over_limit_mid_list_stays_capped() {
        let env = Env::default();
        let list = make_list(&env, 500);
        // offset 500 is past the end → empty regardless of the (capped) limit.
        let page = paginate(&env, &list, 500, u32::MAX);
        assert_eq!(page.len(), 0);
        // A page starting at 50 still returns at most MAX_PAGE_SIZE items.
        let page = paginate(&env, &list, 50, u32::MAX);
        assert_eq!(page.len(), MAX_PAGE_SIZE);
    }

    #[test]
    fn test_paginate_zero_limit_with_over_long_offset_returns_empty() {
        let env = Env::default();
        let list = make_list(&env, 5);
        let page = paginate(&env, &list, 3, 0);
        assert_eq!(page.len(), 0);
    }

    // ── full page ────────────────────────────────────────────────────────────

    #[test]
    fn test_paginate_full_page() {
        let env = Env::default();
        let list = make_list(&env, 5); // [0, 1, 2, 3, 4]
        let page = paginate(&env, &list, 0, 5);
        assert_eq!(page.len(), 5);
        for i in 0..5_u32 {
            assert_eq!(page.get(i).unwrap(), i);
        }
    }

    // ── partial page (limit larger than remaining) ────────────────────────────

    #[test]
    fn test_paginate_limit_clamps_to_available() {
        let env = Env::default();
        let list = make_list(&env, 3); // [0, 1, 2]
        // Ask for 10, only 3 available
        let page = paginate(&env, &list, 0, 10);
        assert_eq!(page.len(), 3);
    }

    // ── offset at boundary ────────────────────────────────────────────────────

    #[test]
    fn test_paginate_offset_at_end_returns_empty() {
        let env = Env::default();
        let list = make_list(&env, 5); // length = 5
        // offset == total → nothing to return
        let page = paginate(&env, &list, 5, 10);
        assert_eq!(page.len(), 0);
    }

    #[test]
    fn test_paginate_offset_past_end_returns_empty() {
        let env = Env::default();
        let list = make_list(&env, 3);
        let page = paginate(&env, &list, 100, 5);
        assert_eq!(page.len(), 0);
    }

    // ── mid-list page ─────────────────────────────────────────────────────────

    #[test]
    fn test_paginate_second_page() {
        let env = Env::default();
        let list = make_list(&env, 5); // [0, 1, 2, 3, 4]
        let page = paginate(&env, &list, 2, 2); // expect [2, 3]
        assert_eq!(page.len(), 2);
        assert_eq!(page.get(0).unwrap(), 2_u32);
        assert_eq!(page.get(1).unwrap(), 3_u32);
    }

    #[test]
    fn test_paginate_last_partial_page() {
        let env = Env::default();
        let list = make_list(&env, 5); // [0, 1, 2, 3, 4]
        let page = paginate(&env, &list, 4, 10); // only element 4 remains
        assert_eq!(page.len(), 1);
        assert_eq!(page.get(0).unwrap(), 4_u32);
    }

    // ── single-element list ────────────────────────────────────────────────────

    #[test]
    fn test_paginate_single_element_list() {
        let env = Env::default();
        let list = make_list(&env, 1); // [0]
        let page = paginate(&env, &list, 0, 1);
        assert_eq!(page.len(), 1);
        assert_eq!(page.get(0).unwrap(), 0_u32);
    }

    #[test]
    fn test_paginate_single_element_offset_1_returns_empty() {
        let env = Env::default();
        let list = make_list(&env, 1); // [0]
        let page = paginate(&env, &list, 1, 1);
        assert_eq!(page.len(), 0);
    }
}
