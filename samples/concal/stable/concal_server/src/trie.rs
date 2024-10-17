// Copyright 2024 Google LLC

// This data structure provides the ability of storing objects
// in a filesystem like manner, where each object is stored at a specific
// path. When retrieving objects, a caller can set some of the paths as empty
// to retrieve everything under this path. For instance having the following data in the trie
//      A                  B
//     /  \               /  \
//    X    Y             X    W
//   /      \           /      \
//  Object1  Object2  Object3  Object4
// Calling get(["", "X"]) returns Object1 and Object3 since all paths at root level are visited and
// then at level 2 of the tree only X nodes are visited

use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Default, Debug)]
pub struct Node<Type: Clone> {
    children: HashMap<String, Arc<Mutex<Node<Type>>>>,
    value: Option<Type>,
}

impl<Type: Clone> Node<Type> {
    pub fn new() -> Self {
        Node { children: HashMap::new(), value: None }
    }
}

#[derive(Default, Debug)]
pub struct Trie<Type: Clone> {
    pub root: Arc<Mutex<Node<Type>>>,
}

impl<Type: Clone> Trie<Type> {
    pub fn new() -> Self {
        Trie { root: Arc::new(Mutex::new(Node::new())) }
    }

    pub fn add(&mut self, path: Vec<String>, val: Type) {
        Self::recursive_add_value(&mut self.root, &mut path.iter(), val);
    }

    fn recursive_add_value(
        node: &mut Arc<Mutex<Node<Type>>>,
        path: &mut std::slice::Iter<'_, String>,
        val: Type,
    ) {
        if let Some(key) = path.next() {
            let mut node = node.lock().unwrap();
            let next_node =
                node.children.entry(key.clone()).or_insert(Arc::new(Mutex::new(Node::new())));
            Self::recursive_add_value(next_node, path, val)
        } else {
            node.borrow_mut().lock().unwrap().value = Some(val);
        }
    }

    pub fn get(&self, path: &[String]) -> Vec<Type> {
        let mut result: Vec<Type> = Vec::new();

        Self::recursive_get_values(&self.root, &mut path.iter(), &mut result);

        result
    }

    fn recursive_get_values(
        node: &Arc<Mutex<Node<Type>>>,
        path: &mut std::slice::Iter<'_, String>,
        result: &mut Vec<Type>,
    ) {
        if let Some(next_node_name) = path.next() {
            if next_node_name.is_empty() {
                for (_, child) in node.lock().unwrap().children.iter() {
                    Self::recursive_get_values(child, &mut path.clone(), result);
                }
            } else if let Some(next_node) = node.lock().unwrap().children.get(next_node_name) {
                Self::recursive_get_values(next_node, path, result);
            }
        } else if let Some(leave_node) = &node.lock().unwrap().value {
            result.push(leave_node.clone());
        }
    }
}

mod tests {
    #![allow(unused_imports)]
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn returns_no_objects_when_structure_is_empty() {
        let trie: Trie<i32> = Trie::new();

        let search_path: Vec<String> = [""].iter().map(|s| s.to_string()).collect();
        let result = trie.get(&search_path);

        assert!(result.is_empty());
    }

    #[test]
    fn can_get_item() {
        let root_node: Arc<Mutex<Node<i32>>> = Arc::new(Mutex::new(Node::new()));
        let level_0_node: Arc<Mutex<Node<i32>>> = Arc::new(Mutex::new(Node::new()));
        let leaf_node: Arc<Mutex<Node<i32>>> = Arc::new(Mutex::new(Node::new()));
        leaf_node.lock().unwrap().value = Some(123);

        level_0_node.lock().unwrap().children.insert(String::from("B"), leaf_node);
        root_node.lock().unwrap().children.insert(String::from("A"), level_0_node);

        let trie: Trie<i32> = Trie { root: root_node };

        let path: Vec<String> = ["A", "B"].iter().map(|s| s.to_string()).collect();
        assert_eq!(trie.get(&path), [123]);
    }

    #[test]
    fn can_insert_item() {
        let mut trie: Trie<i32> = Trie::new();

        let path: Vec<String> = ["A", "B", "C"].iter().map(|s| s.to_string()).collect();
        let value = 123;
        trie.add(path.clone(), value);

        let result = trie.get(&path);

        assert_eq!(result.len(), 1);
        assert_eq!(result.first().unwrap(), &value);
    }

    #[test]
    fn retrieves_all_objects_at_path_matching_expr() {
        let mut trie: Trie<i32> = Trie::new();

        let path1: Vec<String> = ["A", "B", "A"].iter().map(|s| s.to_string()).collect();
        let value1 = 1;
        trie.add(path1, value1);

        let path2: Vec<String> = ["A", "C", "A"].iter().map(|s| s.to_string()).collect();
        let value2 = 2;
        trie.add(path2, value2);

        let path3: Vec<String> = ["A", "C", "C"].iter().map(|s| s.to_string()).collect();
        let value3 = 3;
        trie.add(path3, value3);

        let search_path: Vec<String> = ["A", "", "A"].iter().map(|s| s.to_string()).collect();
        let mut result = trie.get(&search_path);
        result.sort();

        assert_eq!(result.len(), 2);
        assert_eq!(result, [value1, value2]);
    }

    #[test]
    fn returns_nothing_when_path_doesnt_match_any_object() {
        let mut trie: Trie<i32> = Trie::new();

        let path1: Vec<String> = ["A"].iter().map(|s| s.to_string()).collect();
        trie.add(path1, 1);

        let search_path: Vec<String> = ["B"].iter().map(|s| s.to_string()).collect();
        let result = trie.get(&search_path);

        assert!(result.is_empty());
    }

    #[test]
    fn returns_all_objects_when_path_is_empty() {
        let mut trie: Trie<i32> = Trie::new();

        let path1: Vec<String> = ["A"].iter().map(|s| s.to_string()).collect();
        let value1 = 1;
        trie.add(path1, value1);

        let path2: Vec<String> = ["X"].iter().map(|s| s.to_string()).collect();
        let value2 = 2;
        trie.add(path2, value2);

        let search_path: Vec<String> = [""].iter().map(|s| s.to_string()).collect();
        let mut result = trie.get(&search_path);
        result.sort();

        assert_eq!(result.len(), 2);
        assert_eq!(result, [value1, value2]);
    }
}
