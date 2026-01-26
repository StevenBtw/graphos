//! Property storage for the LPG model.
//!
//! This module provides columnar property storage optimized for
//! efficient scanning and filtering.

use graphos_common::types::{EdgeId, NodeId, PropertyKey, Value};
use graphos_common::utils::hash::FxHashMap;
use parking_lot::RwLock;
use std::hash::Hash;
use std::marker::PhantomData;

/// Trait for entity IDs that can be used as property storage keys.
pub trait EntityId: Copy + Eq + Hash + 'static {}

impl EntityId for NodeId {}
impl EntityId for EdgeId {}

/// Columnar property storage.
///
/// Properties are stored in a columnar format where each property key
/// has its own column. This enables efficient filtering and scanning
/// of specific properties across many entities.
///
/// Generic over the entity ID type (NodeId or EdgeId).
pub struct PropertyStorage<Id: EntityId = NodeId> {
    /// Map from property key to column.
    columns: RwLock<FxHashMap<PropertyKey, PropertyColumn<Id>>>,
    _marker: PhantomData<Id>,
}

impl<Id: EntityId> PropertyStorage<Id> {
    /// Creates a new property storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            columns: RwLock::new(FxHashMap::default()),
            _marker: PhantomData,
        }
    }

    /// Sets a property value for an entity.
    pub fn set(&self, id: Id, key: PropertyKey, value: Value) {
        let mut columns = self.columns.write();
        columns
            .entry(key)
            .or_insert_with(PropertyColumn::new)
            .set(id, value);
    }

    /// Gets a property value for an entity.
    #[must_use]
    pub fn get(&self, id: Id, key: &PropertyKey) -> Option<Value> {
        let columns = self.columns.read();
        columns.get(key).and_then(|col| col.get(id))
    }

    /// Removes a property value for an entity.
    pub fn remove(&self, id: Id, key: &PropertyKey) -> Option<Value> {
        let mut columns = self.columns.write();
        columns.get_mut(key).and_then(|col| col.remove(id))
    }

    /// Removes all properties for an entity.
    pub fn remove_all(&self, id: Id) {
        let mut columns = self.columns.write();
        for col in columns.values_mut() {
            col.remove(id);
        }
    }

    /// Gets all properties for an entity.
    #[must_use]
    pub fn get_all(&self, id: Id) -> FxHashMap<PropertyKey, Value> {
        let columns = self.columns.read();
        let mut result = FxHashMap::default();
        for (key, col) in columns.iter() {
            if let Some(value) = col.get(id) {
                result.insert(key.clone(), value);
            }
        }
        result
    }

    /// Returns the number of property columns.
    #[must_use]
    pub fn column_count(&self) -> usize {
        self.columns.read().len()
    }

    /// Returns the keys of all columns.
    #[must_use]
    pub fn keys(&self) -> Vec<PropertyKey> {
        self.columns.read().keys().cloned().collect()
    }

    /// Gets a column by key for bulk access.
    #[must_use]
    pub fn column(&self, key: &PropertyKey) -> Option<PropertyColumnRef<'_, Id>> {
        let columns = self.columns.read();
        if columns.contains_key(key) {
            Some(PropertyColumnRef {
                _guard: columns,
                key: key.clone(),
                _marker: PhantomData,
            })
        } else {
            None
        }
    }
}

impl<Id: EntityId> Default for PropertyStorage<Id> {
    fn default() -> Self {
        Self::new()
    }
}

/// A single property column.
///
/// Stores values for a specific property key across all entities.
pub struct PropertyColumn<Id: EntityId = NodeId> {
    /// Sparse storage: entity ID -> value.
    /// For dense properties, this could be optimized to a flat vector.
    values: FxHashMap<Id, Value>,
}

impl<Id: EntityId> PropertyColumn<Id> {
    /// Creates a new empty column.
    #[must_use]
    pub fn new() -> Self {
        Self {
            values: FxHashMap::default(),
        }
    }

    /// Sets a value for an entity.
    pub fn set(&mut self, id: Id, value: Value) {
        self.values.insert(id, value);
    }

    /// Gets a value for an entity.
    #[must_use]
    pub fn get(&self, id: Id) -> Option<Value> {
        self.values.get(&id).cloned()
    }

    /// Removes a value for an entity.
    pub fn remove(&mut self, id: Id) -> Option<Value> {
        self.values.remove(&id)
    }

    /// Returns the number of values in this column.
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Returns true if this column is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Iterates over all (id, value) pairs.
    pub fn iter(&self) -> impl Iterator<Item = (Id, &Value)> {
        self.values.iter().map(|(&id, v)| (id, v))
    }
}

impl<Id: EntityId> Default for PropertyColumn<Id> {
    fn default() -> Self {
        Self::new()
    }
}

/// A reference to a property column for bulk access.
pub struct PropertyColumnRef<'a, Id: EntityId = NodeId> {
    _guard: parking_lot::RwLockReadGuard<'a, FxHashMap<PropertyKey, PropertyColumn<Id>>>,
    key: PropertyKey,
    _marker: PhantomData<Id>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_storage_basic() {
        let storage = PropertyStorage::new();

        let node1 = NodeId::new(1);
        let node2 = NodeId::new(2);
        let name_key = PropertyKey::new("name");
        let age_key = PropertyKey::new("age");

        storage.set(node1, name_key.clone(), "Alice".into());
        storage.set(node1, age_key.clone(), 30i64.into());
        storage.set(node2, name_key.clone(), "Bob".into());

        assert_eq!(
            storage.get(node1, &name_key).and_then(|v| v.as_str()),
            Some("Alice")
        );
        assert_eq!(
            storage.get(node1, &age_key).and_then(|v| v.as_int64()),
            Some(30)
        );
        assert_eq!(
            storage.get(node2, &name_key).and_then(|v| v.as_str()),
            Some("Bob")
        );
        assert!(storage.get(node2, &age_key).is_none());
    }

    #[test]
    fn test_property_storage_remove() {
        let storage = PropertyStorage::new();

        let node = NodeId::new(1);
        let key = PropertyKey::new("name");

        storage.set(node, key.clone(), "Alice".into());
        assert!(storage.get(node, &key).is_some());

        let removed = storage.remove(node, &key);
        assert!(removed.is_some());
        assert!(storage.get(node, &key).is_none());
    }

    #[test]
    fn test_property_storage_get_all() {
        let storage = PropertyStorage::new();

        let node = NodeId::new(1);
        storage.set(node, PropertyKey::new("name"), "Alice".into());
        storage.set(node, PropertyKey::new("age"), 30i64.into());
        storage.set(node, PropertyKey::new("active"), true.into());

        let props = storage.get_all(node);
        assert_eq!(props.len(), 3);
    }

    #[test]
    fn test_property_storage_remove_all() {
        let storage = PropertyStorage::new();

        let node = NodeId::new(1);
        storage.set(node, PropertyKey::new("name"), "Alice".into());
        storage.set(node, PropertyKey::new("age"), 30i64.into());

        storage.remove_all(node);

        assert!(storage.get(node, &PropertyKey::new("name")).is_none());
        assert!(storage.get(node, &PropertyKey::new("age")).is_none());
    }

    #[test]
    fn test_property_column() {
        let mut col = PropertyColumn::new();

        col.set(NodeId::new(1), "Alice".into());
        col.set(NodeId::new(2), "Bob".into());

        assert_eq!(col.len(), 2);
        assert!(!col.is_empty());

        assert_eq!(
            col.get(NodeId::new(1)).and_then(|v| v.as_str()),
            Some("Alice")
        );

        col.remove(NodeId::new(1));
        assert!(col.get(NodeId::new(1)).is_none());
        assert_eq!(col.len(), 1);
    }
}
