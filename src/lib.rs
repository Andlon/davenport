//! Ergonomic thread-local work spaces for intermediate data.
//!
//! `davenport` is a microcrate with a simple API for working with thread-local data, like
//! buffers for intermediate data. Here's a brief example of the `davenport` API:
//!
//! ```rust
//! use davenport::{define_thread_local_workspace, with_thread_local_workspace};
//!
//! #[derive(Default)]
//! pub struct MyWorkspace {
//!     index_buffer: Vec<usize>
//! }
//!
//! define_thread_local_workspace!(WORKSPACE);
//!
//! fn median(indices: &[usize]) -> Option<usize> {
//!     with_thread_local_workspace(&WORKSPACE, |workspace: &mut MyWorkspace| {
//!         // Re-use buffer from previous call to this function
//!         let buffer = &mut workspace.index_buffer;
//!         buffer.clear();
//!         buffer.copy_from_slice(&indices);
//!         buffer.sort_unstable();
//!         // This is incorrect if the list has an even number of elements,
//!         // we allow this to avoid complicating the example
//!         buffer.get(indices.len() / 2).copied()
//!     })
//! }
//! ```
//! Thread local storage should be used with care. In the above example, if `indices` is large,
//! then a large buffer may be allocated and not freed for the duration of the program. Since
//! stand-alone functions that use thread local storage rarely have enough information to know
//! whether the buffer should be kept alive or not, this may easily lead to unnecessary
//! and redundant memory use.
//!
//!
//! ## Motivating example
//!
//! Let's say we have to compute the sum of a series of elements that are produced in
//! variable-sized "chunks" by a `Producer`. For a fixed element type like `u32`, our code
//! might for example look like this:
//!
//! ```rust
//! pub trait Producer {
//!     fn num_elements(&self) -> usize;
//!     fn populate_buffer(&self, buffer: &mut [u32]);
//! }
//!
//! fn compute_sum(producer: &dyn Producer) -> u32 {
//!     let mut buffer = vec![u32::MAX; producer.num_elements()];
//!     producer.populate_buffer(&mut buffer);
//!     buffer.iter().sum()
//! }
//! ```
//!
//! If we call this method over and over again, it might be prudent to try to avoid the constant
//! re-allocation of the vector. Ideally we'd be able to store some persistent buffer in
//! one of the function arguments, or have `compute_sum` be a method on an object with an
//! internal buffer. However, sometimes we do not have this luxury, perhaps if we're constrained
//! to fit into an existing API that does not allow for buffers to be passed in. An alternative
//! then might be to store the buffer in thread-local storage. Using thread-local storage,
//! the above `compute_sum` function might look like this:
//!
//! ```rust
//! # pub trait Producer {
//! #    fn num_elements(&self) -> usize;
//! #    fn populate_buffer(&self, buffer: &mut [u32]);
//! # }
//! fn compute_sum(producer: &dyn Producer) -> u32 {
//!     thread_local! { static BUFFER: std::cell::RefCell<Vec<u32>> = Default::default(); }
//!     BUFFER.with(|rc| {
//!         let mut buffer = rc.borrow_mut();
//!         producer.populate_buffer(&mut *buffer);
//!         buffer.iter().sum()
//!     })
//! }
//! ```
//! Now, let's imagine that we wanted our function to work with a more generic set of types,
//! rather than `u32` alone. We generalize the `Producer` trait, but quickly realize
//! that we cannot create a `thread_local!` buffer in the same way.
//!
//! ```ignore
//! use std::ops::{Add, AddAssign};
//!
//! pub trait Producer<T> {
//!    fn num_elements(&self) -> usize;
//!    fn populate_buffer(&self, buffer: &mut [T]);
//! }
//!
//! fn compute_sum<T>(producer: &dyn Producer<T>) -> T
//! where
//!     T: 'static + Default + Copy + std::iter::Sum
//! {
//!     // Does not compile!
//!     //  error[E0401]: can't use generic parameters from outer function
//!     thread_local! { static BUFFER: std::cell::RefCell<Vec<T>> = Default::default(); }
//!     BUFFER.with(|rc| {
//!         let mut buffer = rc.borrow_mut();
//!         buffer.resize(producer.num_elements(), T::default());
//!         producer.populate_buffer(&mut *buffer);
//!         buffer.iter()
//!               .copied()
//!               .sum()
//!     })
//! }
//! ```
//!
//! It turns out that it's generally difficult to construct a thread local workspace that's
//! *generic* in its type. However, we can do this with `davenport`:
//! ```rust
//! use davenport::{define_thread_local_workspace, with_thread_local_workspace};
//! use std::ops::{Add, AddAssign};
//!
//! # pub trait Producer<T> {
//! #   fn num_elements(&self) -> usize;
//! #   fn populate_buffer(&self, buffer: &mut [T]);
//! # }
//! #
//! fn compute_sum<T>(producer: &dyn Producer<T>) -> T
//! where
//!     T: 'static + Default + Copy + std::iter::Sum
//! {
//!     define_thread_local_workspace!(WORKSPACE);
//!     with_thread_local_workspace(&WORKSPACE, |buffer: &mut Vec<T>| {
//!         buffer.resize(producer.num_elements(), T::default());
//!         producer.populate_buffer(buffer);
//!         buffer.iter()
//!               .copied()
//!               .sum()
//!     })
//! }
//! ```
//!
//! `davenport` gets around the aforementioned restrictions because the *actual* thread-local
//! variable is an instance of [`Workspace`], which is a container for type-erased work spaces.
//! Thus, what is really happening in the example above is that a thread-local [`Workspace`] type
//! is constructed, which we ask for a mutable reference to `Vec<T>`. If the buffer does not
//! yet exist, it is default-constructed. Otherwise we obtain a previously-used instance.
//!

use std::any::Any;
use std::cell::RefCell;
use std::thread::LocalKey;

/// A workspace that contains type-erased objects.
///
/// The workspace is intended to hold intermediate data used as workspace in computations.
/// It is optimized particularly for the case where the same type is accessed many times in a row.
///
/// TODO: Tests
#[derive(Debug, Default)]
pub struct Workspace {
    workspaces: Vec<Box<dyn Any>>,
}

impl Workspace {
    pub fn get_or_insert_with<W, F>(&mut self, create: F) -> &mut W
        where
            W: 'static,
            F: FnOnce() -> W,
    {
        // Note: We treat the Vec as a stack, so we search from the end of the vector.
        let existing_ws_idx = self.workspaces.iter().rposition(|ws| ws.is::<W>());
        let idx = match existing_ws_idx {
            Some(idx) => idx,
            None => {
                let w = create();
                let idx = self.workspaces.len();
                self.workspaces.push(Box::new(w) as Box<dyn Any>);
                idx
            }
        };

        // We heuristically assume that the same object is likely to be accessed
        // many times in sequence. Therefore we make sure that the object is the last entry,
        // so that on the next lookup, we'll immediately find the correct object
        let last = self.workspaces.len() - 1;
        self.workspaces.swap(idx, last);

        let entry = &mut self.workspaces[last];
        entry
            .downcast_mut()
            .expect("Internal error: Downcasting can by definition not fail")
    }

    pub fn get_or_default<W>(&mut self) -> &mut W
        where
            W: 'static + Default,
    {
        self.get_or_insert_with(Default::default)
    }
}

/// Runs the provided closure with the thread-local workspace as an argument.
///
/// This simplifies working with [`Workspace`] when it's stored as a thread-local variable.
///
/// Note that the typed workspace must have a [`Default`] implementation.
///
/// # Examples
///
/// ```rust
/// # use std::cell::RefCell;
/// use davenport::{define_thread_local_workspace, with_thread_local_workspace, Workspace};
///
/// define_thread_local_workspace!(WORKSPACE);
///
/// // Workspace must implement Default
/// #[derive(Default)]
/// struct MyWorkspace {
///     buffer: Vec<usize>
/// }
///
/// fn main() {
///     let sum: usize = with_thread_local_workspace(&WORKSPACE, |ws: &mut MyWorkspace| {
///         // This is of course completely nonsense, we just show how you can easily use a thread-local workspace
///         // and produce a result which is returned.
///         ws.buffer.clear();
///         ws.buffer.extend_from_slice(&[1, 4, 3]);
///         ws.buffer.iter().sum()
///     });
///     println!("Sum = {}", sum);
/// }
/// ```
pub fn with_thread_local_workspace<W: 'static + Default, T>(
    workspace: &'static LocalKey<RefCell<Workspace>>,
    f: impl FnOnce(&mut W) -> T,
) -> T {
    workspace.with(|refcell_ws| {
        let mut type_erased_workspace = refcell_ws.borrow_mut();
        let workspace = type_erased_workspace.get_or_default();
        f(workspace)
    })
}

/// Helper macro for easily defining thread-local workspaces.
///
/// # Example
/// ```
/// use davenport::{define_thread_local_workspace, with_thread_local_workspace};
///
/// define_thread_local_workspace!(WORKSPACE);
///
/// // Workspace must implement Default
/// #[derive(Default)]
/// struct MyWorkspace {
///     buffer: Vec<usize>
/// }
///
/// fn foo() {
///     with_thread_local_workspace(&WORKSPACE, |workspace: &mut MyWorkspace| {
///         workspace.buffer.clear();
///         workspace.buffer.copy_from_slice(&[1, 2, 3]);
///         // Perform some computations that need intermediate storage
///     })
/// }
/// ```
///
#[macro_export]
macro_rules! define_thread_local_workspace {
    ($variable_name:ident) => {
        thread_local! {
            static $variable_name: std::cell::RefCell<$crate::Workspace>
                = std::cell::RefCell::new($crate::Workspace::default());
        }
    };
}
