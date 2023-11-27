use crate::structs::Store;

pub trait Start {
    fn start(&self);
}

pub trait Truncate {
    fn truncate(&self);
}

pub trait Periodical {
    fn poll(&self);
}

pub trait Update {
    fn update(&self);
}

pub trait Truncator: Start + Truncate + Periodical + Update {}
impl<T: Start + Truncate + Periodical + Update> Truncator for T {}

pub struct SimpleTruncator {
    _time_elapsed: i32,
    _store: Option<Store>,
}

impl SimpleTruncator {
    pub fn new() -> Self {
        SimpleTruncator {
            _time_elapsed: 0,
            _store: None,
        }
    }
}

impl Start for SimpleTruncator {
    fn start(&self) {}
}

impl Truncate for SimpleTruncator {
    fn truncate(&self) {}
}

impl Periodical for SimpleTruncator {
    fn poll(&self) {}
}

impl Update for SimpleTruncator {
    fn update(&self) {}
}
