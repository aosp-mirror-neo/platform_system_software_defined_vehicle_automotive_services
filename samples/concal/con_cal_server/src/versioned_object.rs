// Copyright 2024 Google LLC

// This data structure provides the ability to take a snapshot of an object
// and rollback to an earlier value. When dereferencing a versionedObject,
// the latest value is returned as a mutable reference allowing the consumer to modify its value.
// To persist a value, call the snapshot method in order to persist the current value and allow rollbacks
// to it later on.

use std::clone::Clone;
use std::ops::{Deref, DerefMut};
use std::vec::Vec;

pub struct VersionedObject<T: Clone> {
    num_versions_kept: usize,
    versions: Vec<T>,
}

impl<T: Clone> VersionedObject<T> {
    pub fn new(value: T, num_versions_kept: usize) -> Self {
        VersionedObject { num_versions_kept, versions: vec![value] }
    }

    pub fn rollback(&mut self) -> bool {
        if self.versions.len() > 1 {
            self.versions.remove(self.versions.len() - 1);
            return true;
        }
        false
    }

    pub fn snapshot(&mut self) {
        self.versions.push(self.versions.last().unwrap().clone());
        if self.versions.len() > self.num_versions_kept {
            self.versions.remove(0);
        }
    }
}

impl<T: Clone> Deref for VersionedObject<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.versions.last().unwrap()
    }
}

impl<T: Clone> DerefMut for VersionedObject<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.versions.last_mut().unwrap()
    }
}

mod tests {
    #![allow(unused_imports)]
    use super::*;

    #[test]
    fn can_read_initial_value() {
        let val = VersionedObject::new(5, 1);
        assert_eq!(*val, 5);
    }

    #[test]
    fn can_override_initial_value() {
        let mut val = VersionedObject::new(5, 1);
        *val = 10;
        assert_eq!(*val, 10);
    }

    #[test]
    fn preserves_snapshotted_values() {
        let values = [1, 2, 4];
        let mut val = VersionedObject::new(*values.first().unwrap(), 3);

        for i in 1..values.len() {
            val.snapshot();
            *val = *values.get(i).unwrap();
        }

        for i in (0..values.len()).rev() {
            assert_eq!(*val, values[i]);
            val.rollback();
        }
    }

    #[test]
    fn rollback_returns_true_when_successful() {
        let mut val = VersionedObject::new(1, 2);
        val.snapshot();

        assert!(val.rollback());
    }

    #[test]
    fn rollback_returns_false_when_unable_to_rollback() {
        let mut val = VersionedObject::new(1, 1);

        assert!(!val.rollback());
    }
}
