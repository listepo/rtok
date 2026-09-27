//! T299 fixture: a realistic-sized Rust source used to exercise the `read` plugin's
//! `map`, `signatures` and `search` modes against one shared file (tests/plugins_e2e.rs).
//! Kept verbose (doc comments, longer bodies) so each mode's savings vs. the raw file
//! are meaningful rather than trivial on a handful of lines.

use std::collections::HashMap;
use std::fmt;

/// A single item tracked by the warehouse inventory.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub sku: String,
    pub name: String,
    pub quantity: u32,
    pub unit_price_cents: u64,
    pub category: Category,
}

/// Coarse grouping used for reporting and reorder thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Category {
    Perishable,
    Electronics,
    Furniture,
    Miscellaneous,
}

impl fmt::Display for Category {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Category::Perishable => "perishable",
            Category::Electronics => "electronics",
            Category::Furniture => "furniture",
            Category::Miscellaneous => "misc",
        };
        write!(f, "{label}")
    }
}

/// In-memory inventory keyed by SKU. Not persisted; callers own durability.
#[derive(Debug, Default)]
pub struct Inventory {
    items: HashMap<String, Item>,
}

impl Inventory {
    /// A fresh, empty inventory.
    pub fn new() -> Self {
        Self {
            items: HashMap::new(),
        }
    }

    /// Insert or replace an item by its SKU. Returns the previous entry, if any,
    /// so a caller can log what was overwritten instead of losing it silently.
    pub fn upsert(&mut self, item: Item) -> Option<Item> {
        self.items.insert(item.sku.clone(), item)
    }

    /// Look up an item by SKU. Borrowed, so a hot loop over many lookups does
    /// not pay a clone per call.
    pub fn get(&self, sku: &str) -> Option<&Item> {
        self.items.get(sku)
    }

    /// Adjust quantity by `delta`, clamping at zero rather than going negative
    /// (a warehouse can sell down to zero, never below it). Returns the new
    /// quantity, or `None` when the SKU is unknown.
    pub fn adjust_quantity(&mut self, sku: &str, delta: i64) -> Option<u32> {
        let item = self.items.get_mut(sku)?;
        let current = i64::from(item.quantity);
        let next = (current + delta).max(0) as u32;
        item.quantity = next;
        Some(next)
    }

    /// Total value of the inventory, in cents, across every tracked item.
    /// Uses `u128` for the running sum: a large enough warehouse with high
    /// unit prices could otherwise overflow `u64` during the multiply-sum.
    pub fn total_value_cents(&self) -> u128 {
        self.items
            .values()
            .map(|it| u128::from(it.quantity) * u128::from(it.unit_price_cents))
            .sum()
    }

    /// Items at or below `threshold`, grouped by category, sorted by SKU
    /// within each group for a stable report ordering.
    pub fn low_stock_by_category(&self, threshold: u32) -> HashMap<Category, Vec<&Item>> {
        let mut out: HashMap<Category, Vec<&Item>> = HashMap::new();
        for item in self.items.values() {
            if item.quantity <= threshold {
                out.entry(item.category).or_default().push(item);
            }
        }
        for group in out.values_mut() {
            group.sort_by(|a, b| a.sku.cmp(&b.sku));
        }
        out
    }

    /// Remove an item entirely. Used when a SKU is discontinued rather than
    /// merely out of stock (quantity 0 still tracks the item; removal drops it).
    pub fn discontinue(&mut self, sku: &str) -> Option<Item> {
        self.items.remove(sku)
    }
}

/// A simple, hard cap on any single reorder line so a fat-fingered quantity
/// cannot silently blow a purchasing budget.
const MAX_REORDER_QUANTITY: u32 = 10_000;

/// Build a reorder plan: for every item at or below its own threshold, order
/// enough to reach `target`. Fails closed on the first invalid line rather
/// than applying a partial plan.
pub fn plan_reorder(
    inventory: &Inventory,
    threshold: u32,
    target: u32,
) -> Result<Vec<(String, u32)>, String> {
    let mut plan = Vec::new();
    for item in inventory.items.values() {
        if item.quantity > threshold {
            continue;
        }
        let needed = target.saturating_sub(item.quantity);
        if needed == 0 {
            continue;
        }
        if needed > MAX_REORDER_QUANTITY {
            return Err(format!("reorder of {needed} for {} exceeds the cap", item.sku));
        }
        plan.push((item.sku.clone(), needed));
    }
    plan.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(plan)
}
