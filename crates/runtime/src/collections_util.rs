//! `Collections` utility functions -- mirrors `java.util.Collections` static methods.

use crate::list::JList;

/// `Collections.sort(list)` -- natural ordering.
pub fn collections_sort<T: Ord + Clone>(list: &mut JList<T>) {
    list.sort();
}

/// `Collections.sort(list, comparator)` -- custom comparator.
pub fn collections_sort_with<T: Clone>(list: &mut JList<T>, cmp: impl Fn(&T, &T) -> i32) {
    list.sort_with(cmp);
}

/// `Collections.reverse(list)`.
pub fn collections_reverse<T: Clone>(list: &mut JList<T>) {
    list.reverse();
}

/// `Collections.singletonList(item)`.
pub fn collections_singleton_list<T: Clone>(item: T) -> JList<T> {
    JList::singleton(item)
}
