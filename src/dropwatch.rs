use std::time;

/// Measures time from the creation of this watch to when it exists scope
pub struct Dropwatch {
    start : Option<time::Instant>,
    id : String
}

impl Dropwatch {
    pub fn new(id: &str) -> Self {
        Dropwatch { start: None, id: id.to_string() }
    }

    pub fn begin(&mut self) {
        self.start = Some(time::Instant::now());
    }

    pub fn new_begin(id: &str) -> Self {
        let mut s = Self::new(id);
        s.begin();
        return s;
    }
}

impl Drop for Dropwatch {
    fn drop(&mut self) {
        if let Some(start) = self.start {
            println!("{}: {} seconds elapsed", self.id, (time::Instant::now() - start).as_secs_f32());
        }
    }
}