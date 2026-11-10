//! Collections that mirror `std::collections` for ArceOS applications.
//!
//! The tree based containers come straight from [`alloc::collections`]. The
//! standard library's `HashMap` lives in `std` because it needs a random hasher
//! (`std::collections::hash_map::RandomState`) fed by the OS random source,
//! which a `no_std` kernel application does not have. This module therefore
//! ships a small hash table of its own so that `axstd::collections::HashMap`
//! behaves like the one applications know from `std`.
//!
//! The table is an open-addressing table with linear probing: keys are stored
//! in a power-of-two sized array of slots, a slot is either empty, deleted
//! (tombstone) or occupied. Tombstones let `remove` stay O(1) without breaking
//! the probe chains built by `insert`, and a growth policy keeps the probe
//! sequences short.

pub use alloc::collections::*;

use core::borrow::Borrow;
use core::fmt;
use core::hash::{BuildHasher, Hash, Hasher};
use core::sync::atomic::{AtomicU64, Ordering};

use alloc::vec::Vec;

/// The initial number of slots. Always a power of two.
const INITIAL_CAPACITY: usize = 8;

/// Grow when the table is 3/4 full (counting tombstones).
const LOAD_FACTOR_NUM: usize = 3;
const LOAD_FACTOR_DEN: usize = 4;

/// FNV-1a based hasher, with a seed and a final avalanche step so that the low
/// bits (the ones used to index the table) are well distributed.
#[derive(Clone, Copy)]
pub struct DefaultHasher {
    state: u64,
}

impl DefaultHasher {
    /// Creates a hasher with the given seed.
    pub const fn with_seed(seed: u64) -> Self {
        // Offset basis of FNV-1a 64, mixed with the seed.
        Self {
            state: 0xcbf2_9ce4_8422_2325 ^ seed,
        }
    }
}

impl Hasher for DefaultHasher {
    fn write(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= u64::from(*byte);
            self.state = self.state.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }

    fn finish(&self) -> u64 {
        // splitmix64 style finalizer: spreads the high bits into the low ones.
        let mut x = self.state;
        x ^= x >> 33;
        x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
        x ^= x >> 33;
        x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
        x ^= x >> 33;
        x
    }
}

impl fmt::Debug for DefaultHasher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DefaultHasher")
            .field("state", &self.state)
            .finish()
    }
}

/// The default hasher builder used by [`HashMap`].
///
/// It is the `no_std` counterpart of `std::collections::hash_map::RandomState`:
/// every instance gets a different seed so that different maps do not share the
/// same iteration order or collision pattern. The seed mixes a global counter
/// with the address of a stack local, which is the only entropy source a kernel
/// application can count on without pulling in a random number generator.
#[derive(Clone, Copy)]
pub struct RandomState {
    seed: u64,
}

impl RandomState {
    /// Creates a builder with a fresh, process-local seed.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let local = 0u8;
        // Address of a stack slot: varies between threads and runs.
        let entropy = &local as *const u8 as u64;
        Self {
            seed: mix(entropy.rotate_left(17) ^ count.wrapping_mul(0x9e37_79b9_7f4a_7c15)),
        }
    }

    /// Creates a builder with an explicit seed, useful for reproducible maps.
    pub const fn with_seed(seed: u64) -> Self {
        Self { seed }
    }
}

impl Default for RandomState {
    fn default() -> Self {
        Self::new()
    }
}

impl BuildHasher for RandomState {
    type Hasher = DefaultHasher;

    fn build_hasher(&self) -> Self::Hasher {
        DefaultHasher::with_seed(self.seed)
    }
}

fn mix(mut x: u64) -> u64 {
    x ^= x >> 33;
    x = x.wrapping_mul(0xff51_afd7_ed55_8ccd);
    x ^= x >> 33;
    x = x.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    x ^ (x >> 33)
}

/// One slot of the hash table.
enum Slot<K, V> {
    /// Never used, or usable by the current probe chain.
    Empty,
    /// Holds a removed entry: the probe chain must continue past it.
    Deleted,
    /// Holds a live entry.
    Full(K, V),
}

/// A hash map implemented with linear probing.
///
/// The interface follows `std::collections::HashMap`, so applications written
/// against the standard library keep working when built on top of `axstd`.
pub struct HashMap<K, V, S = RandomState> {
    slots: Vec<Slot<K, V>>,
    len: usize,
    /// Number of `Deleted` slots, they are reused by later insertions.
    tombstones: usize,
    hasher: S,
}

impl<K, V, S> HashMap<K, V, S> {
    /// Creates an empty map that uses `hasher`.
    pub fn with_hasher(hasher: S) -> Self {
        Self {
            slots: (0..INITIAL_CAPACITY).map(|_| Slot::Empty).collect(),
            len: 0,
            tombstones: 0,
            hasher,
        }
    }

    /// Returns the number of entries the map can hold without reallocating.
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Returns the number of entries in the map.
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` if the map is empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Removes every entry, keeping the allocated slots.
    pub fn clear(&mut self) {
        for slot in self.slots.iter_mut() {
            *slot = Slot::Empty;
        }
        self.len = 0;
        self.tombstones = 0;
    }

    /// Iterates over the entries in an unspecified order.
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter {
            inner: self.slots.iter(),
        }
    }

    /// Iterates over the entries with mutable values.
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut {
            inner: self.slots.iter_mut(),
        }
    }

    /// Iterates over the keys.
    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.iter().map(|(key, _)| key)
    }

    /// Iterates over the values.
    pub fn values(&self) -> impl Iterator<Item = &V> {
        self.iter().map(|(_, value)| value)
    }

    /// Iterates over the values.
    pub fn values_mut(&mut self) -> impl Iterator<Item = &mut V> {
        self.iter_mut().map(|(_, value)| value)
    }
}

impl<K, V> HashMap<K, V, RandomState> {
    /// Creates an empty map.
    pub fn new() -> Self {
        Self::with_hasher(RandomState::new())
    }
}

impl<K: Hash + Eq, V> HashMap<K, V, RandomState> {
    /// Creates an empty map with room for at least `capacity` entries.
    pub fn with_capacity(capacity: usize) -> Self {
        Self::with_capacity_and_hasher(capacity, RandomState::new())
    }
}

impl<K: Hash + Eq, V, S: BuildHasher> HashMap<K, V, S> {
    /// Creates an empty map with room for `capacity` entries and `hasher`.
    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> Self {
        let mut map = Self::with_hasher(hasher);
        map.reserve(capacity);
        map
    }

    /// Makes room for at least `additional` more entries.
    pub fn reserve(&mut self, additional: usize) {
        let needed = self.len + additional;
        if needed * LOAD_FACTOR_DEN <= self.slots.len() * LOAD_FACTOR_NUM {
            return;
        }
        let mut capacity = self.slots.len();
        while needed * LOAD_FACTOR_DEN > capacity * LOAD_FACTOR_NUM {
            capacity *= 2;
        }
        self.resize(capacity);
    }

    /// Inserts a key-value pair, returning the previous value if the key was
    /// already present.
    pub fn insert(&mut self, key: K, value: V) -> Option<V> {
        if let Some(index) = self.find_slot(&key) {
            if let Slot::Full(_, slot_value) = &mut self.slots[index] {
                return Some(core::mem::replace(slot_value, value));
            }
            unreachable!("find_slot returned a slot without an entry");
        }

        // Grow (and thereby drop all tombstones) before probing for a new home.
        if (self.len + self.tombstones + 1) * LOAD_FACTOR_DEN > self.slots.len() * LOAD_FACTOR_NUM
        {
            self.resize(self.slots.len() * 2);
        }

        let mask = self.slots.len() - 1;
        let mut index = (self.hash_key(&key) & mask) as usize;
        let mut first_deleted = None;
        loop {
            if matches!(self.slots[index], Slot::Empty) {
                // The key is absent: reuse the first tombstone of this probe
                // chain when there is one, otherwise use the empty slot.
                let target = first_deleted.unwrap_or(index);
                if matches!(self.slots[target], Slot::Deleted) {
                    self.tombstones -= 1;
                }
                self.slots[target] = Slot::Full(key, value);
                self.len += 1;
                return None;
            }
            if first_deleted.is_none() && matches!(self.slots[index], Slot::Deleted) {
                first_deleted = Some(index);
            }
            index = (index + 1) & mask;
        }
    }

    /// Returns a reference to the value associated with `key`.
    pub fn get<Q>(&self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.find_slot(key).and_then(|index| match &self.slots[index] {
            Slot::Full(_, value) => Some(value),
            _ => None,
        })
    }

    /// Returns a mutable reference to the value associated with `key`.
    pub fn get_mut<Q>(&mut self, key: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let index = self.find_slot(key)?;
        match &mut self.slots[index] {
            Slot::Full(_, value) => Some(value),
            _ => None,
        }
    }

    /// Returns `true` if the map contains `key`.`
    pub fn contains_key<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.find_slot(key).is_some()
    }

    /// Removes a key from the map, returning its value.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let index = self.find_slot(key)?;
        match core::mem::replace(&mut self.slots[index], Slot::Deleted) {
            Slot::Full(_, value) => {
                self.len -= 1;
                self.tombstones += 1;
                Some(value)
            }
            _ => None,
        }
    }

    /// Returns the slot index holding `key`, if any.
    fn find_slot<Q>(&self, key: &Q) -> Option<usize>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let mask = self.slots.len() - 1;
        let mut index = (self.hash_key(key) & mask) as usize;
        for _ in 0..self.slots.len() {
            match &self.slots[index] {
                Slot::Empty => return None,
                Slot::Deleted => {}
                Slot::Full(stored, _) => {
                    if stored.borrow() == key {
                        return Some(index);
                    }
                }
            }
            index = (index + 1) & mask;
        }
        None
    }

    fn hash_key<Q: Hash + ?Sized>(&self, key: &Q) -> usize {
        let mut hasher = self.hasher.build_hasher();
        key.hash(&mut hasher);
        hasher.finish() as usize
    }

    /// Rebuilds the table with `capacity` slots.
    fn resize(&mut self, capacity: usize) {
        let mut slots: Vec<Slot<K, V>> = (0..capacity).map(|_| Slot::Empty).collect();
        let mask = capacity - 1;
        for slot in core::mem::replace(&mut self.slots, Vec::new()) {
            if let Slot::Full(key, value) = slot {
                let mut index = (self.hash_key(&key) & mask) as usize;
                while !matches!(slots[index], Slot::Empty) {
                    index = (index + 1) & mask;
                }
                slots[index] = Slot::Full(key, value);
            }
        }
        self.slots = slots;
        self.tombstones = 0;
    }
}

impl<K: Hash + Eq, V> Default for HashMap<K, V, RandomState> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Hash + Eq + fmt::Debug, V: fmt::Debug, S: BuildHasher> fmt::Debug for HashMap<K, V, S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K: Hash + Eq, V, S: BuildHasher> Extend<(K, V)> for HashMap<K, V, S> {
    fn extend<I: IntoIterator<Item = (K, V)>>(&mut self, iter: I) {
        for (key, value) in iter {
            self.insert(key, value);
        }
    }
}

impl<K: Hash + Eq, V, S: BuildHasher + Default> FromIterator<(K, V)> for HashMap<K, V, S> {
    fn from_iter<I: IntoIterator<Item = (K, V)>>(iter: I) -> Self {
        let mut map = Self::with_hasher(S::default());
        map.extend(iter);
        map
    }
}

impl<K: Hash + Eq, V, S: BuildHasher> PartialEq for HashMap<K, V, S>
where
    V: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().all(|(k, v)| other.get(k) == Some(v))
    }
}

impl<K: Hash + Eq, V> Clone for HashMap<K, V, RandomState>
where
    K: Clone,
    V: Clone,
{
    fn clone(&self) -> Self {
        let mut map = Self::with_hasher(self.hasher);
        map.extend(self.iter().map(|(k, v)| (k.clone(), v.clone())));
        map
    }
}

/// Immutable iterator over a [`HashMap`].
pub struct Iter<'a, K, V> {
    inner: core::slice::Iter<'a, Slot<K, V>>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        for slot in self.inner.by_ref() {
            if let Slot::Full(key, value) = slot {
                return Some((key, value));
            }
        }
        None
    }
}

/// Mutable iterator over a [`HashMap`].
pub struct IterMut<'a, K, V> {
    inner: core::slice::IterMut<'a, Slot<K, V>>,
}

impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    fn next(&mut self) -> Option<Self::Item> {
        for slot in self.inner.by_ref() {
            if let Slot::Full(key, value) = slot {
                return Some((&*key, value));
            }
        }
        None
    }
}

impl<'a, K, V, S> IntoIterator for &'a HashMap<K, V, S> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a, K, V, S> IntoIterator for &'a mut HashMap<K, V, S> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
