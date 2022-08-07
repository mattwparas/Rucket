// pub trait UseCallbacks {
//     fn use_callbacks() -> bool;
// }

// pub struct UseCallback;

// impl UseCallbacks for UseCallback {
//     #[inline(always)]
//     fn use_callbacks() -> bool {
//         true
//     }
// }

// pub struct DoNotUseCallback;

// impl UseCallbacks for DoNotUseCallback {
//     #[inline(always)]
//     fn use_callbacks() -> bool {
//         false
//     }
// }

pub trait EnableJit {
    fn enable_jit() -> bool;
}

pub struct UseJit;

impl EnableJit for UseJit {
    #[inline(always)]
    fn enable_jit() -> bool {
        true
    }
}

pub struct DoNotUseJit;

impl EnableJit for DoNotUseJit {
    #[inline(always)]
    fn enable_jit() -> bool {
        false
    }
}

pub trait EnableProfiling {
    fn enable_profiling();
}
