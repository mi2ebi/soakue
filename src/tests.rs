use crate::toadua::Toa;
use itertools::Itertools as _;
use std::collections::{hash_map::Entry, HashMap};

/// Used to test all possible orders of a slice. This is important because we are performing sorting
/// tests, and we want to ensure that the orders picked for testing are not biased for success
fn permute<T: Clone, F: Fn(Vec<T>) + Copy>(slice: &[T], fun: F) {
    let mut current_perm = slice.to_vec();

    let len = current_perm.len();
    heap_permute(&mut current_perm, len, fun);
}

/// This does the actual permuation. I just didn't want to be using a function with a parameter that
/// will always be `something.len()`.
fn heap_permute<T: Clone, F: Fn(Vec<T>) + Copy>(slice: &mut Vec<T>, n: usize, fun: F) {
    if n == 1 {
        fun(slice.clone());
    }
    for i in 0..n {
        heap_permute(slice, n - 1, fun);

        if n % 2 == 0 {
            slice.swap(i, n - 1);
        } else {
            slice.swap(0, n - 1);
        }
    }
}

/// Little implementation used for creating test entries
impl Toa {
    pub fn test_new(head: String) -> Self {
        Self {
            id: String::new(),
            date: String::new(),
            head,
            body: String::new(),
            user: String::new(),
            notes: Vec::new(),
            score: 0,
            scope: String::new(),
            warn: false,
        }
    }
}

/// A macro to make test entries even easier to make
macro_rules! toa {
    ($head:literal) => {
        Toa::test_new($head.to_string())
    };
    [$($head:literal),+] => (
        vec![$(Toa::test_new($head.to_string())),+]
    );
}

#[test]
fn tone_ordering() {
    let words = toa!["é", "e", "ê", "ë", "è", "e-", "a"];

    permute(&words, |words| {
        let words = words
            .into_iter()
            .sorted_by(Toa::cmp)
            .map(|x| x.head)
            .collect_vec();

        assert_eq!(&["a", "e-", "e", "è", "é", "ë", "ê"], words.as_slice());
    });
}

#[test]
fn word_ordering() {
    let words = toa!["é", "éshea", "e", "eshea", "naq", "nä"];

    permute(&words, |words| {
        let words = words
            .into_iter()
            .sorted_by(Toa::cmp)
            .map(|x| x.head)
            .collect_vec();

        assert_eq!(&["e", "é", "eshea", "éshea", "nä", "naq"], words.as_slice());
    });
}

#[test]
fn error_ordering() {
    let words = toa!["Usona mí Lısa da.", "uatı / uakı / (uakytı?)", "x"];

    permute(&words, |words| {
        let words = words
            .into_iter()
            .sorted_by(Toa::cmp)
            .map(|x| x.head)
            .collect_vec();

        assert_eq!(
            &["Usona mí Lısa da.", "x", "uatı / uakı / (uakytı?)"],
            words.as_slice()
        );
    });
}

#[test]
fn sentence_ordering() {
    let words = toa!["ana", "ina", "ana da", "ina da", "x", "x x"];

    permute(&words, |words| {
        let words = words
            .into_iter()
            .sorted_by(Toa::cmp)
            .map(|x| x.head)
            .collect_vec();

        assert_eq!(
            &["ana", "ina", "ana da", "ina da", "x", "x x"],
            words.as_slice()
        );
    });
}

#[test]
fn strict_ordering() {
    // A bunch of tests designed for every possible situation that the sort might find
    // - Both words finish at the same time, and one is a prefix
    // - Both words finish at the same time, no prefixes
    // - Both words fail at the same time
    // - Neither word finishes, but that is enough for a success. One word would eventually fail if
    //   its parsing continued.
    // - Neither word finishes, but that is enough for a success. No word fails and the success is
    //   left in the hands of the tone.
    // - One word ends, the other one fails at the same time
    // - One word ends, the other one does not, but *will* eventually fail
    let mut words =
        toa!["e-", "a", "e", "é", "ë", "ae", "ea", "e a", "x", "ax", "aax", "a x", "e e"];

    // `tuple_combinations` does not give repeats, so I made it manually repeat.
    words.append(&mut words.clone());

    let mut equalities = HashMap::new();

    words.into_iter().tuple_combinations().for_each(|(a, b)| {
        // Check first order
        let key = (a.head.clone(), b.head.clone());
        let cmp = a.cmp(&b);

        match equalities.entry(key) {
            Entry::Occupied(entry) => {
                let val = entry.get();
                assert!(
                    *val == cmp,
                    "{:?} was previously {:?} wrt {:?} but is now {cmp:?}",
                    a.head,
                    val,
                    b.head
                );
            }
            Entry::Vacant(_) => {
                // Make sure the sorting hasn't been done before in the opposite order
                let key = (b.head.clone(), a.head.clone());

                // CANNOT be the invert() of the existing comparison because we are *verifying*,
                // among other things, that `a.cmp(b).invert()` is the same as `b.cmp(a)`
                let cmp = b.cmp(&a);

                match equalities.entry(key) {
                    Entry::Occupied(entry) => {
                        let val = entry.get();
                        assert!(
                            *val == cmp,
                            "{:?} was previously {:?} wrt {:?} but is now {cmp:?}",
                            b.head,
                            val,
                            a.head
                        );
                    }
                    Entry::Vacant(vacant_entry) => {
                        vacant_entry.insert(cmp);
                    }
                }
            }
        }

        // It is possible
    });
}
