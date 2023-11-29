use std::time::SystemTime;

use log::debug;

use crate::{structs::Store, widgets::RenderWidget};

pub trait Truncatorix {
    fn set_time(&mut self, now: SystemTime);
    fn get_time(&self) -> SystemTime;
    fn truncate(&mut self, store: &mut Store);
    fn poll(&mut self) -> Option<()> {
        if self.get_time().elapsed().unwrap().as_secs() > 5 {
            self.set_time(SystemTime::now());
            Some(())
        } else {
            None
        }
    }
    fn start(&mut self) {
        self.set_time(SystemTime::now())
    }
}

pub struct TopTruncator {
    now: Option<SystemTime>,
    from_to_top: i16,
}

impl TopTruncator {
    pub fn new(from_to_top: i16) -> Self {
        TopTruncator {
            now: None,
            from_to_top,
        }
    }
}

impl Truncatorix for TopTruncator {
    fn set_time(&mut self, now: SystemTime) {
        self.now = Some(now)
    }

    fn get_time(&self) -> SystemTime {
        self.now.unwrap()
    }

    fn truncate(&mut self, store: &mut Store) {
        if let Some(widget) = &mut store.logs_widget {
            if let Some(Some(data)) = widget.get_data().data.get_mut("logs") {
                let truncate_index = data.len() as i16 - self.from_to_top;
                if truncate_index > 0 {
                    widget.set_data("logs".to_string(), data.split_off(truncate_index as usize));
                }
            }
        }
    }
}

pub struct NoopTruncator {
    _time_elapsed: i32,
    _store: Option<Store>,
}

impl NoopTruncator {
    pub fn new() -> Self {
        NoopTruncator {
            _time_elapsed: 0,
            _store: None,
        }
    }
}

impl Truncatorix for NoopTruncator {
    fn set_time(&mut self, now: SystemTime) {}

    fn get_time(&self) -> SystemTime {
        SystemTime::now()
    }

    fn truncate(&mut self, store: &mut Store) {}
}
