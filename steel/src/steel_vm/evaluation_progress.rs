#![allow(unused)]

use std::cell::Cell;

pub type Callback = Box<dyn Fn(usize) -> bool>;

trait CallbackFunc {
    fn call(&self) -> Option<bool>;
}

impl CallbackFunc for () {
    fn call(&self) -> Option<bool> {
        None
    }
}

pub(crate) struct EvaluationProgress {
    instruction_count: Cell<usize>,
    callback: Option<Callback>,
}

impl EvaluationProgress {
    pub fn new() -> Self {
        EvaluationProgress {
            instruction_count: Cell::new(1),
            callback: None,
        }
    }

    pub fn with_callback(&mut self, callback: Callback) {
        self.callback.replace(callback);
    }

    pub fn callback(&self) -> Option<bool> {
        self.callback
            .as_ref()
            .map(|callback| callback(self.instruction_count.get()))
    }

    pub fn increment(&self) {
        self.instruction_count.set(self.instruction_count.get() + 1);
    }

    #[inline(always)]
    pub fn call_and_increment(&self) -> Option<bool> {
        let b = self.callback();
        self.increment();
        b
    }
}
