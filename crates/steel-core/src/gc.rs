use crate::rerrs::SteelErr;
use crate::rvals::SteelVal;
use crate::stop;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{ffi::OsStr, fmt};
use std::{ops::Deref, rc::Weak};

pub static OBJECT_COUNT: AtomicUsize = AtomicUsize::new(0);
pub(crate) static MAXIMUM_OBJECTS: usize = 50000;

// TODO: Consider triomphe for a drop in replacement of Arc

/// Used for automatic detection of ref cycle
pub enum MaybeWeak<T: Clone> {
    StrongRef(Gc<T>),
    WeakRef(Gc<T>),
}

/// This is simply a newtype around the `Rc` type
/// When enabled, this allows for complete sandboxing of data types
/// It does not expose the full functionality of the `Rc` type
/// but it does allow for some
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct Gc<T: ?Sized>(pub(crate) Rc<T>);

/// Newtype around the `Weak` type.
/// Enables the detection of reference cycles in mutable memory locations
pub struct WeakGc<T>(Weak<T>);

impl fmt::Display for Gc<SteelVal> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for Gc<String> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub fn get_object_count() -> usize {
    OBJECT_COUNT.fetch_add(0, Ordering::SeqCst)
}

impl<T: Clone> Gc<T> {
    /// Deep clone the object to remove it from the GC
    pub fn unwrap(&self) -> T {
        (*self.0).clone()
    }

    pub fn make_mut(&mut self) -> &mut T {
        Rc::make_mut(&mut self.0)
    }
}

impl<T> Gc<T> {
    // in order to fully sandbox, I have to check the memory limit
    pub fn new(val: T) -> Gc<T> {
        // OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);
        Gc(Rc::new(val))
    }

    pub fn try_new(val: T) -> Result<Gc<T>, SteelErr> {
        let mem: usize = OBJECT_COUNT.fetch_add(1, Ordering::SeqCst);
        if mem > MAXIMUM_OBJECTS {
            stop!(Generic => "ran out of memory!")
        }
        Ok(Gc(Rc::new(val)))
    }

    pub fn checked_allocate(allocations: usize) -> Result<(), SteelErr> {
        let mem: usize = OBJECT_COUNT.fetch_add(0, Ordering::SeqCst);
        if mem + allocations > MAXIMUM_OBJECTS {
            stop!(Generic => "allocation would exceed maximum allowed memory")
        }
        Ok(())
    }

    pub fn downgrade(this: &Self) -> Weak<T> {
        Rc::downgrade(&this.0)
    }

    pub fn get_mut(&mut self) -> Option<&mut T> {
        Rc::get_mut(&mut self.0)
    }

    pub fn ptr_eq(this: &Self, other: &Self) -> bool {
        Rc::ptr_eq(&this.0, &other.0)
    }

    pub fn as_ptr(&self) -> *const T {
        Rc::as_ptr(&self.0)
    }

    pub fn try_unwrap_(self) -> Result<T, Gc<T>> {
        Rc::try_unwrap(self.0).map_err(|x| Gc(x))
    }

    // this does not match the original semantics of Rc::try_unwrap
    // in order to match this, we would need some unsafe rust
    // instead, I take a _slight_ performance hit in order to
    // match the original functionality, and the specific use case
    // for me, which is unwinding lists in the drop for SteelVal
    // pub fn try_unwrap(this: Self) -> Result<T, SteelErr> {
    //     let inner = Rc::clone(&this.0);
    //     drop(this);
    //     Rc::try_unwrap(inner)
    //         .map_err(|_| SteelErr::new(ErrorKind::Generic, "value still has reference".to_string()))
    //     // .map(|x| {
    //     //     OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
    //     //     x
    //     // })
    // }

    pub fn check_memory() -> Result<usize, SteelErr> {
        let mem: usize = OBJECT_COUNT.fetch_add(0, Ordering::SeqCst);
        if mem > MAXIMUM_OBJECTS {
            stop!(Generic => "ran out of memory!")
        }
        Ok(mem)
    }
}

impl<T> AsRef<T> for Gc<T> {
    fn as_ref(&self) -> &T {
        self.0.as_ref()
    }
}

impl<T> Deref for Gc<T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.0.deref()
    }
}

// impl<T> Drop for Gc<T> {
//     fn drop(&mut self) {
//         // println!("Strong count: {}", Rc::strong_count(&self.0));

//         // if Rc::strong_count(&self.0) == 1 {
//         //     OBJECT_COUNT.fetch_sub(1, Ordering::SeqCst);
//         // }
//     }
// }

impl<T> Clone for Gc<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Gc(Rc::clone(&self.0))
    }
}

impl AsRef<OsStr> for Gc<String> {
    fn as_ref(&self) -> &OsStr {
        self.0.as_ref().as_ref()
    }
}

impl From<&str> for Gc<String> {
    fn from(val: &str) -> Self {
        Gc::new(val.to_string())
    }
}

impl From<String> for Gc<String> {
    fn from(val: String) -> Self {
        Gc::new(val)
    }
}

impl From<&String> for Gc<String> {
    fn from(val: &String) -> Self {
        Gc::new(val.clone())
    }
}

impl AsRef<str> for Gc<String> {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[cfg(feature = "unsafe-internals")]
pub mod unsafe_roots {

    use super::Gc;
    use std::ptr::NonNull;

    #[derive(Clone)]
    pub enum MaybeRooted<T> {
        Rooted(Rooted<T>),
        Reference(Gc<T>),
    }

    impl<T: std::fmt::Debug> std::fmt::Debug for MaybeRooted<T> {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                Self::Rooted(v) => write!(f, "{:?}", unsafe { v.value.as_ref() }),
                Self::Reference(v) => write!(f, "{:?}", v),
            }
        }
    }

    impl<T> MaybeRooted<T> {
        pub fn from_root(value: &Gc<T>) -> Self {
            Self::Rooted(Rooted::from_ref(value))
        }
    }

    impl<T> AsRef<T> for MaybeRooted<T> {
        fn as_ref(&self) -> &T {
            match self {
                Self::Rooted(v) => unsafe { v.value.as_ref() },
                Self::Reference(v) => &v,
            }
        }
    }

    impl<T> std::ops::Deref for MaybeRooted<T> {
        type Target = T;

        fn deref(&self) -> &Self::Target {
            match self {
                Self::Rooted(v) => unsafe { v.value.as_ref() },
                Self::Reference(v) => &v,
            }
        }
    }

    #[derive(Clone)]
    pub struct Rooted<T> {
        value: NonNull<T>,
    }

    impl<T> Rooted<T> {
        pub fn from_ref(value: &Gc<T>) -> Self {
            Rooted {
                value: NonNull::new(Gc::as_ptr(value) as _).expect("Given pointer was null!"),
            }
        }
    }

    #[test]
    fn test_rooting() {
        use crate::SteelVal;

        let root = Gc::new(SteelVal::ListV(im_lists::list![]));

        let rooted_reference = Rooted::from_ref(&root);

        println!("{:?}", unsafe { rooted_reference.value.as_ref() });
    }

    #[test]
    fn recover_original_gc() {
        use crate::SteelVal;

        let root = Gc::new(SteelVal::ListV(im_lists::list![]));

        let rooted_reference = Rooted::from_ref(&root);

        let recovered = unsafe { rooted_reference.value.as_ref() };

        println!("{:?}", recovered);
    }
}

// #[cfg(feature = "unsafe-internals")]
pub mod unsafe_erased_pointers {

    use std::{any::Any, cell::RefCell, marker::PhantomData};
    use std::{
        cell::RefMut,
        rc::{Rc, Weak},
    };

    use crate::{
        rerrs::ErrorKind,
        rvals::{AsRefMutSteelVal, AsRefMutSteelValFromRef, Custom, IntoSteelVal},
        SteelErr, SteelVal,
    };

    pub struct MutableReferenceNursery<T> {
        _phantom: PhantomData<T>, // pointers: Vec<Rc<RefCell<*mut T>>>,
    }

    impl<T> MutableReferenceNursery<T> {
        pub fn new() -> Self {
            Self {
                _phantom: PhantomData, // pointers: Vec::new(),
            }
        }

        pub fn retain_reference(
            &mut self,
            original: &mut T,
            mut thunk: impl FnMut(BorrowedObject<T>) -> crate::rvals::Result<()>,
        ) -> crate::rvals::Result<()> {
            let erased = original as *mut _;

            // Wrap the original mutable pointer in an object that respects borrowing
            // rules for runtime borrow checking
            let wrapped = Rc::new(RefCell::new(erased));
            let weak_ptr = Rc::downgrade(&wrapped);

            let borrowed = BorrowedObject { ptr: weak_ptr };

            thunk(borrowed)
        }
    }

    pub trait CustomReference {}

    pub trait ReferenceCustomType {
        fn as_any_ref(&self) -> &dyn Any;
        fn as_any_ref_mut(&mut self) -> &mut dyn Any;
        fn name(&self) -> &str {
            std::any::type_name::<Self>()
        }
        fn display(&self) -> std::result::Result<String, std::fmt::Error> {
            Ok(format!("#<{}>", self.name().to_string()))
        }
    }

    impl<'a, T: CustomReference + 'static> ReferenceCustomType for T {
        fn as_any_ref(&self) -> &dyn Any {
            self as &dyn Any
        }
        fn as_any_ref_mut(&mut self) -> &mut dyn Any {
            self as &mut dyn Any
        }
        fn display(&self) -> std::result::Result<String, std::fmt::Error> {
            // if let Some(formatted) = self.fmt() {
            //     formatted
            // } else {
            Ok(format!("#<{}>", self.name().to_string()))
            // }
        }
    }

    // impl<T: ReferenceCustomType + 'static> IntoSteelVal for T {
    //     fn into_steelval(self) -> crate::rvals::Result<SteelVal> {
    //         // Ok(self.new_steel_val())
    //         Ok(SteelVal::Custom(Rc::new(RefCell::new(Box::new(self)))))
    //     }
    // }

    // pub struct

    // unsafe fn extend_lifetime<'b>(r: R<'b>) -> R<'static> {
    //     std::mem::transmute::<R<'b>, R<'static>>(r)
    // }

    pub struct BorrowedObject<T> {
        ptr: Weak<RefCell<*mut T>>,
    }

    impl<T> CustomReference for BorrowedObject<T> {}

    impl<'a, T> Clone for BorrowedObject<T> {
        fn clone(&self) -> Self {
            Self {
                ptr: Weak::clone(&self.ptr),
            }
        }
    }

    impl<T: 'static> BorrowedObject<T> {
        pub fn into_opaque_reference<'a>(self) -> OpaqueReference<'a> {
            // unsafe {
            //     std::mem::transmute::<R<'b>, R<'static>>(r)
            // }

            // let extended = std::mem::transmute::<BorrowedObject<T>

            OpaqueReference {
                inner: Rc::new(self),
            }
        }
    }

    // impl<T> BorrowedObject {
    //     pub fn borrow_mut()
    // }

    #[derive(Clone)]
    pub struct OpaqueReference<'a> {
        inner: Rc<dyn ReferenceCustomType + 'a>,
    }

    impl OpaqueReference<'static> {
        pub fn format(&self) -> std::result::Result<String, std::fmt::Error> {
            self.display()
        }
    }

    impl CustomReference for OpaqueReference<'static> {}

    // struct Applesauce

    // Erase the type, continue on with our lives
    // impl<T: 'static> Custom for BorrowedObject<T> {}

    impl<T: ReferenceCustomType + 'static> AsRefMutSteelValFromRef for T {
        fn as_mut_ref_from_ref<'a>(val: &'a SteelVal) -> crate::rvals::Result<&'a mut T> {
            // todo!()

            if let SteelVal::Reference(v) = val {
                let res = v.inner.as_any_ref();

                if res.is::<BorrowedObject<T>>() {
                    let borrowed_object = res.downcast_ref::<BorrowedObject<T>>().unwrap();

                    // return Ok(borrowed_object.clone());

                    let guard = borrowed_object.ptr.upgrade().ok_or_else(
                        throw!(Generic => "opaque reference pointer dropped before use!"),
                    );

                    return guard.map(|x| unsafe { &mut *(*x.borrow_mut()) });
                } else {
                    let error_message = format!(
                        "Type Mismatch: Type of SteelVal: {} did not match the given type: {}",
                        val,
                        std::any::type_name::<Self>()
                    );
                    Err(SteelErr::new(ErrorKind::ConversionError, error_message))
                }
            } else {
                let error_message = format!(
                    "Type Mismatch: Type of SteelVal: {} did not match the given type: {}",
                    val,
                    std::any::type_name::<Self>()
                );

                Err(SteelErr::new(ErrorKind::ConversionError, error_message))
            }
        }
    }

    #[derive(Debug)]
    struct FooBar {
        baz: String,
    }

    struct Baz<'a> {
        foo_bar: &'a mut FooBar,
    }

    impl FooBar {
        fn append_str(&mut self, suffix: &str) {
            self.baz.push_str(suffix);
        }
    }

    #[test]
    fn test() {
        let mut nursery = MutableReferenceNursery::new();

        let mut object = FooBar {
            baz: "hello world!".to_string(),
        };

        let mut baz = Baz {
            foo_bar: &mut object,
        };

        // HAS to be around the whole time
        nursery
            .retain_reference(&mut baz, |erased| unsafe {
                let guard = erased.ptr.upgrade().unwrap();

                let ref_mut: &mut Baz = &mut *(*guard.borrow_mut());

                ref_mut.foo_bar.append_str("bananas");

                Ok(())
            })
            .unwrap();

        object.append_str("foobar");

        dbg!(object);
    }

    // #[test]
    // fn test_registering_functions_engine
}
