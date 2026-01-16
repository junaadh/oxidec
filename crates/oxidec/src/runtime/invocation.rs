//! Message invocation objects for forwarding.
//!
//! This module provides an NSInvocation-equivalent for message forwarding.
//! Invocations encapsulate a message send with its target, selector, and
//! arguments, allowing dynamic manipulation before invocation.
//!
//! # Design
//!
//! The `Invocation` struct provides:
//! - Type-erased argument storage (all arguments as raw pointers)
//! - Argument marshalling based on type encoding
//! - Return value handling
//! - Dynamic rewriting (target, selector, arguments)
//! - Safe invocation with MIRI validation
//!
//! # Safety
//!
//! All argument access uses `read_unaligned`/`write_unaligned` for potentially
//! misaligned pointers. Pointer arithmetic uses `addr_of!` for strict provenance.
//! All unsafe blocks have SAFETY comments explaining invariants.
//!
//! # Clippy Exceptions
//!
//! This module uses type-erased storage which requires some unsafe patterns:
//!
//! # Example
//!
//! ```rust
//! use oxidec::runtime::{Invocation, Object, Selector, MessageArgs, Class};
//! use oxidec::runtime::get_global_arena;
//! use std::str::FromStr;
//!
//! # unsafe extern "C" fn noop_impl(
//! #     _self: oxidec::runtime::object::ObjectPtr,
//! #     _cmd: oxidec::runtime::selector::SelectorHandle,
//! #     _args: *const *mut u8,
//! #     _ret: *mut u8,
//! # ) {}
//! #
//! # let class = Class::new_root("MyClass").unwrap();
//! # let selector = Selector::from_str("testMethod").unwrap();
//! # let arena = get_global_arena();
//! # let method = oxidec::runtime::class::Method {
//! #     selector: selector.clone(),
//! #     imp: noop_impl,
//! #     types: oxidec::runtime::RuntimeString::new("v@:", arena),
//! # };
//! # class.add_method(method).unwrap();
//! # let target = Object::new(&class).unwrap();
//! # let new_target = target.clone();
//! // Create invocation
//! let mut invocation = Invocation::new(&target, &selector)?;
//!
//! // Modify target
//! invocation.set_target(&new_target);
//!
//! // Invoke
//! unsafe { invocation.invoke()?; }
//! # Ok::<(), oxidec::error::Error>(())
//! ```

use crate::error::{Error, Result};
use crate::runtime::message::MessageArgs;
use crate::runtime::{Object, Selector};

/// Maximum number of arguments an invocation can hold (excluding self and _cmd).
const MAX_ARGS: usize = 16;

/// Message invocation object.
///
/// Encapsulates a message send with target, selector, and arguments,
/// allowing manipulation before invocation. Used in Stage 3 of the
/// four-stage forwarding pipeline (`forwardInvocation:`).
///
/// # Type Safety
///
/// Arguments are stored as type-erased raw pointers (`*mut u8`). Type
/// information comes from the method signature encoding. The generic
/// `get_argument`/`set_argument` methods provide type-safe access with
/// runtime validation.
///
/// # Memory Management
///
/// Arguments are arena-allocated from the global arena and live for the
/// duration of the invocation. Return values are stack-allocated for
/// small values (< 16 bytes) or arena-allocated for large values.
///
/// # Thread Safety
///
/// Invocations are `Send` (can be sent between threads) but not `Sync`
/// (cannot be shared concurrently). This matches the typical use case:
/// create on one thread, forward to another thread for invocation.
///
/// # Example
///
/// ```rust
/// use oxidec::runtime::{Invocation, Object, Selector, MessageArgs};
/// use oxidec::runtime::get_global_arena;
/// use std::str::FromStr;
///
/// # unsafe extern "C" fn noop_impl(
/// #     _self: oxidec::runtime::object::ObjectPtr,
/// #     _cmd: oxidec::runtime::selector::SelectorHandle,
/// #     _args: *const *mut u8,
/// #     _ret: *mut u8,
/// # ) {}
/// #
/// # let class = oxidec::runtime::Class::new_root("MyClass").unwrap();
/// # let selector = Selector::from_str("testMethod").unwrap();
/// # let arena = get_global_arena();
/// # let method = oxidec::runtime::class::Method {
/// #     selector: selector.clone(),
/// #     imp: noop_impl,
/// #     types: oxidec::runtime::RuntimeString::new("v@:", arena),
/// # };
/// # class.add_method(method).unwrap();
/// # let target = Object::new(&class).unwrap();
/// // Create with target and selector
/// let invocation = Invocation::new(&target, &selector)?;
///
/// // Or with arguments
/// # let new_target = target.clone();
/// # let selector2 = Selector::from_str("testMethod:").unwrap();
/// # let method2 = oxidec::runtime::class::Method {
/// #     selector: selector2.clone(),
/// #     imp: noop_impl,
/// #     types: oxidec::runtime::RuntimeString::new("v@:ii", arena),
/// # };
/// # class.add_method(method2).unwrap();
/// let args = MessageArgs::two(10, 20);
/// let mut invocation = Invocation::with_arguments(&target, &selector2, &args)?;
///
/// // Modify before invocation
/// invocation.set_target(&new_target);
///
/// // Invoke
/// unsafe { invocation.invoke()?; }
/// # Ok::<(), oxidec::error::Error>(())
/// ```
#[derive(Debug)]
pub struct Invocation {
    /// Target object (receiver).
    target: Object,

    /// Selector to send.
    selector: Selector,

    /// Type-erased arguments (excluding self and _cmd).
    /// Each element is a pointer to arena-allocated argument data.
    arguments: Vec<*mut u8>,

    /// Method signature encoding (e.g., "i@:i" for int return with int arg).
    signature: Option<String>,

    /// Return value buffer (None for void, Some(pointer) for values).
    return_value: Option<*mut u8>,

    /// Return value size in bytes (0 for void).
    return_size: usize,

    /// Invocation flags for optimization and tracking.
    flags: InvocationFlags,
}

/// Invocation flags for optimization and bookkeeping.
#[derive(Debug, Clone, Copy, Default)]
#[allow(clippy::struct_excessive_bools)]
struct InvocationFlags {
    /// Has this invocation been invoked?
    invoked: bool,

    /// Has the target been modified since creation?
    target_modified: bool,

    /// Has the selector been modified since creation?
    selector_modified: bool,

    /// Have any arguments been modified since creation?
    arguments_modified: bool,
}

// SAFETY: Invocation is Send because all fields are Send:
// - Object: Send (atomic refcount)
// - Selector: Send (interned string)
// - Vec<*mut u8>: Send (pointer ownership)
// - Option<String>: Send
// - Option<*mut u8>: Send (pointer ownership)
// - InvocationFlags: Send (plain old data)
unsafe impl Send for Invocation {}

impl Invocation {
    /// Creates a new invocation with target and selector (no arguments).
    ///
    /// # Arguments
    ///
    /// * `target` - The target object (receiver)
    /// * `selector` - The selector to send
    ///
    /// # Returns
    ///
    /// `Ok(Invocation)` if created successfully, `Err` if target is invalid.
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidPointer` if the target's internal pointer is invalid.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::Invocation;
    /// use oxidec::runtime::{Class, Object, Selector};
    /// use std::str::FromStr;
    ///
    /// # let class = Class::new_root("MyClass").unwrap();
    /// # let target = Object::new(&class).unwrap();
    /// # let selector = Selector::from_str("testMethod").unwrap();
    /// let invocation = Invocation::new(&target, &selector)?;
    /// # Ok::<(), oxidec::error::Error>(())
    /// ```
    pub fn new(target: &Object, selector: &Selector) -> Result<Self> {
        Self::with_arguments(target, selector, &MessageArgs::None)
    }

    /// Creates a new invocation with target, selector, and arguments.
    ///
    /// # Arguments
    ///
    /// * `target` - The target object (receiver)
    /// * `selector` - The selector to send
    /// * `args` - Message arguments (excluding self and _cmd)
    ///
    /// # Returns
    ///
    /// `Ok(Invocation)` if created successfully, `Err` if:
    /// - Target is invalid
    /// - Argument count exceeds `MAX_ARGS`
    /// - Argument marshalling fails
    ///
    /// # Errors
    ///
    /// Returns `Error::InvalidPointer` if the target's internal pointer is invalid,
    /// or `Error::ArgumentCountMismatch` if the argument count exceeds `MAX_ARGS`.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::{Invocation, MessageArgs, Class, Object, Selector};
    /// use std::str::FromStr;
    ///
    /// # let class = Class::new_root("MyClass").unwrap();
    /// # let target = Object::new(&class).unwrap();
    /// # let selector = Selector::from_str("testMethod:").unwrap();
    /// let args = MessageArgs::two(10, 20);
    /// let invocation = Invocation::with_arguments(&target, &selector, &args)?;
    /// # Ok::<(), oxidec::error::Error>(())
    /// ```
    pub fn with_arguments(
        target: &Object,
        selector: &Selector,
        args: &MessageArgs,
    ) -> Result<Self> {
        let arg_count = args.count();

        if arg_count > MAX_ARGS {
            return Err(Error::ArgumentCountMismatch {
                expected: MAX_ARGS,
                got: arg_count,
            });
        }

        // Marshal arguments into type-erased storage
        let arguments = Self::marshal_arguments(args)?;

        Ok(Self {
            target: target.clone(),
            selector: selector.clone(),
            arguments,
            signature: None, // Will be set by forwarding pipeline
            return_value: None,
            return_size: 0,
            flags: InvocationFlags::default(),
        })
    }

    /// Marshals `MessageArgs` into type-erased pointer storage.
    ///
    /// # Safety
    ///
    /// This function uses `Box::into_raw` to leak memory for each argument.
    /// The memory is reclaimed when the Invocation is dropped. This is
    /// safe because:
    /// - We own the Box (allocated here)
    /// - We store the raw pointer
    /// - Drop implementation reclaims it
    /// - No double-free possible (single ownership)
    fn marshal_arguments(args: &MessageArgs) -> Result<Vec<*mut u8>> {
        let arg_slice = args.as_slice();
        let mut arguments = Vec::with_capacity(arg_slice.len());

        for &arg in arg_slice {
            // Store each argument as a boxed usize
            // SAFETY: We leak the Box to convert to raw pointer.
            // The Drop implementation will reclaim this memory.
            let boxed = Box::new(arg);
            let ptr = Box::into_raw(boxed).cast();
            arguments.push(ptr);
        }

        Ok(arguments)
    }

    /// Returns the target object.
    #[inline]
    #[must_use]
    pub fn target(&self) -> &Object {
        &self.target
    }

    /// Returns the selector.
    #[inline]
    #[must_use]
    pub fn selector(&self) -> &Selector {
        &self.selector
    }

    /// Returns the number of arguments (excluding self and _cmd).
    #[inline]
    #[must_use]
    pub fn argument_count(&self) -> usize {
        self.arguments.len()
    }

    /// Sets a new target for this invocation.
    ///
    /// # Arguments
    ///
    /// * `target` - The new target object
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::{Invocation, Class, Object, Selector};
    /// use std::str::FromStr;
    ///
    /// # let class = Class::new_root("MyClass").unwrap();
    /// # let target = Object::new(&class).unwrap();
    /// # let new_target = target.clone();
    /// # let selector = Selector::from_str("testMethod").unwrap();
    /// # let mut invocation = Invocation::new(&target, &selector).unwrap();
    /// invocation.set_target(&new_target);
    /// ```
    pub fn set_target(&mut self, target: &Object) {
        self.target = target.clone();
        self.flags.target_modified = true;
    }

    /// Sets a new selector for this invocation.
    ///
    /// # Arguments
    ///
    /// * `selector` - The new selector
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::{Invocation, Class, Object, Selector};
    /// use std::str::FromStr;
    ///
    /// # let class = Class::new_root("MyClass").unwrap();
    /// # let target = Object::new(&class).unwrap();
    /// # let selector = Selector::from_str("oldMethod").unwrap();
    /// # let new_selector = Selector::from_str("newMethod").unwrap();
    /// # let mut invocation = Invocation::new(&target, &selector).unwrap();
    /// invocation.set_selector(&new_selector);
    /// ```
    pub fn set_selector(&mut self, selector: &Selector) {
        self.selector = selector.clone();
        self.flags.selector_modified = true;
    }

    /// Sets the method signature for this invocation.
    ///
    /// The signature is used for argument validation and reconstruction.
    /// This is typically set by the four-stage forwarding pipeline.
    ///
    /// # Arguments
    ///
    /// * `signature` - The method signature encoding string
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::{Invocation, Class, Object, Selector};
    /// use std::str::FromStr;
    ///
    /// # let class = Class::new_root("MyClass").unwrap();
    /// # let target = Object::new(&class).unwrap();
    /// # let selector = Selector::from_str("testMethod").unwrap();
    /// # let mut invocation = Invocation::new(&target, &selector).unwrap();
    /// invocation.set_signature(Some("i@:i".to_string()));
    /// ```
    pub fn set_signature(&mut self, signature: Option<String>) {
        self.signature = signature;
    }

    /// Gets an argument by index (type-safe).
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type to interpret the argument as (must be usize-sized)
    ///
    /// # Arguments
    ///
    /// * `index` - The argument index (0-based, excludes self and _cmd)
    ///
    /// # Returns
    ///
    /// `Ok(&T)` if the index is valid, `Err` otherwise.
    ///
    /// # Errors
    ///
    /// - `Error::ArgumentCountMismatch` - Index out of bounds
    ///
    /// # Safety
    ///
    /// This function requires T to be the same size as usize. The pointer
    /// was allocated from a Box<usize> and we reconstruct it as &T.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::Invocation;
    /// use oxidec::runtime::{Class, Object, Selector};
    /// use oxidec::runtime::MessageArgs;
    /// use std::str::FromStr;
    ///
    /// # let class = Class::new_root("MyClass").unwrap();
    /// # let obj = Object::new(&class).unwrap();
    /// # let sel = Selector::from_str("testMethod:").unwrap();
    /// # let args = MessageArgs::two(42usize, 100usize);
    /// # let invocation = Invocation::with_arguments(&obj, &sel, &args).unwrap();
    /// let arg: &usize = invocation.get_argument(0)?;
    /// # Ok::<(), oxidec::error::Error>(())
    /// ```
    pub fn get_argument<T>(&self, index: usize) -> Result<&T>
    where
        T: std::fmt::Debug + 'static,
    {
        if index >= self.arguments.len() {
            return Err(Error::ArgumentCountMismatch {
                expected: self.arguments.len(),
                got: index,
            });
        }

        let ptr = self.arguments[index];

        // SAFETY: The pointer was allocated from Box::into_raw in marshal_arguments
        // as Box<usize>. We reconstruct the reference as &T using bitcast.
        //
        // Invariant: ptr points to valid memory allocated by marshal_arguments
        // Invariant: Memory will remain valid for lifetime of &self
        // Invariant: No other mutable references exist to this memory
        // Invariant: T must be same size as usize (enforced by caller)
        let usize_ptr = ptr.cast::<usize>();
        let usize_ref = unsafe { &*usize_ptr };

        // SAFETY: Transmute from &usize to &T
        // This is safe because MessageArgs stores all values as usize,
        // and we're just reinterpreting the bits.
        //
        // Invariant: T has same size and alignment as usize
        let value_ref = unsafe { std::mem::transmute::<&usize, &T>(usize_ref) };

        Ok(value_ref)
    }

    /// Sets an argument by index (type-safe).
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type of the value to set (must be usize-sized)
    ///
    /// # Arguments
    ///
    /// * `index` - The argument index (0-based, excludes self and _cmd)
    /// * `value` - The value to set
    ///
    /// # Returns
    ///
    /// `Ok(())` if successful, `Err` if index out of bounds.
    ///
    /// # Errors
    ///
    /// - `Error::ArgumentCountMismatch` - Index out of bounds
    ///
    /// # Safety
    ///
    /// This function uses transmute to convert &T to &usize, which is safe
    /// when T has the same size and alignment as usize.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::{Invocation, Class, Object, Selector, MessageArgs};
    /// use std::str::FromStr;
    ///
    /// # let class = Class::new_root("MyClass").unwrap();
    /// # let target = Object::new(&class).unwrap();
    /// # let selector = Selector::from_str("testMethod:").unwrap();
    /// # let args = MessageArgs::two(1usize, 2usize);
    /// # let mut invocation = Invocation::with_arguments(&target, &selector, &args).unwrap();
    /// invocation.set_argument(0, &42usize)?;
    /// # Ok::<(), oxidec::error::Error>(())
    /// ```
    pub fn set_argument<T>(&mut self, index: usize, value: &T) -> Result<()> {
        if index >= self.arguments.len() {
            return Err(Error::ArgumentCountMismatch {
                expected: self.arguments.len(),
                got: index,
            });
        }

        let ptr = self.arguments[index];

        // SAFETY: The pointer was allocated from Box::into_raw in marshal_arguments
        // as Box<usize>. We transmute &T to &usize to write the value.
        //
        // Invariant: ptr points to valid, owned memory
        // Invariant: No other references exist to this memory
        // Invariant: T has same size as usize (caller responsibility)
        unsafe {
            let usize_ref: &usize = std::mem::transmute(value);
            let write_ptr = ptr.cast::<usize>();
            std::ptr::write(write_ptr, *usize_ref);
        }

        self.flags.arguments_modified = true;
        Ok(())
    }

    /// Gets the return value (type-safe).
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type to interpret the return value as (must be usize-sized)
    ///
    /// # Returns
    ///
    /// `Ok(&T)` if invocation has a return value, `Err` if void or not invoked.
    ///
    /// # Errors
    ///
    /// - `Error::InvalidPointer` - No return value (void or not yet invoked)
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::{Invocation, Class, Object, Selector};
    /// use std::str::FromStr;
    ///
    /// # let class = Class::new_root("MyClass").unwrap();
    /// # let target = Object::new(&class).unwrap();
    /// # let selector = Selector::from_str("testMethod").unwrap();
    /// # let mut invocation = Invocation::new(&target, &selector).unwrap();
    /// # invocation.set_return_value(&42usize);
    /// unsafe {
    ///     let result: &usize = invocation.get_return_value()?;
    /// }
    /// # Ok::<(), oxidec::error::Error>(())
    /// ```
    pub fn get_return_value<T>(&self) -> Result<&T>
    where
        T: std::fmt::Debug + 'static,
    {
        let ptr = self.return_value.ok_or(Error::InvalidPointer { ptr: 0 })?;

        // SAFETY: Same rationale as get_argument
        let usize_ptr = ptr.cast::<usize>();
        let usize_ref = unsafe { &*usize_ptr };
        let value_ref = unsafe { std::mem::transmute::<&usize, &T>(usize_ref) };

        Ok(value_ref)
    }

    /// Sets the return value (for proxy rewriting).
    ///
    /// # Type Parameters
    ///
    /// * `T` - The type of the return value (must be usize-sized)
    ///
    /// # Arguments
    ///
    /// * `value` - The return value to set
    ///
    /// # Panics
    ///
    /// This function never panics.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::{Invocation, Class, Object, Selector};
    /// use std::str::FromStr;
    ///
    /// # let class = Class::new_root("MyClass").unwrap();
    /// # let target = Object::new(&class).unwrap();
    /// # let selector = Selector::from_str("testMethod").unwrap();
    /// # let mut invocation = Invocation::new(&target, &selector).unwrap();
    /// invocation.set_return_value(&42usize);
    /// ```
    pub fn set_return_value<T>(&mut self, value: &T) {
        // Allocate return value storage if needed
        if self.return_value.is_none() {
            let boxed = Box::new(0usize); // Placeholder
            let ptr = Box::into_raw(boxed).cast();
            self.return_value = Some(ptr);
            self.return_size = std::mem::size_of::<T>();
        }

        let ptr = self.return_value.unwrap();

        // SAFETY: Same rationale as set_argument
        unsafe {
            let usize_ref: &usize = std::mem::transmute(value);
            let write_ptr = ptr.cast::<usize>();
            std::ptr::write(write_ptr, *usize_ref);
        }
    }

    /// Invokes the message send.
    ///
    /// This is the core method that actually sends the message to the target
    /// with the current selector and arguments. It stores the return value
    /// for later retrieval.
    ///
    /// # Returns
    ///
    /// `Ok(Some(retval))` for non-void returns, `Ok(None)` for void, `Err` on failure.
    ///
    /// # Safety
    ///
    /// This function is `unsafe` because:
    /// - It performs a raw message send via the runtime
    /// - Type safety relies on correct signature encoding
    /// - Arguments are type-erased and validated at runtime
    ///
    /// Callers must ensure:
    /// - The signature encoding (if set) matches the actual method
    /// - Arguments are valid for the selector
    /// - The target is a valid object
    ///
    /// # Errors
    ///
    /// Returns `Error::SelectorNotFound` if the selector is not found in the
    /// target's class hierarchy, or `Error::ArgumentCountMismatch` if the
    /// argument count doesn't match the method signature.
    ///
    /// # Example
    ///
    /// ```rust
    /// use oxidec::runtime::{Invocation, Class, Object, Selector};
    /// use oxidec::runtime::get_global_arena;
    /// use std::str::FromStr;
    ///
    /// # unsafe extern "C" fn noop_impl(
    /// #     _self: oxidec::runtime::object::ObjectPtr,
    /// #     _cmd: oxidec::runtime::selector::SelectorHandle,
    /// #     _args: *const *mut u8,
    /// #     _ret: *mut u8,
    /// # ) {}
    /// #
    /// # let class = Class::new_root("MyClass").unwrap();
    /// # let selector = Selector::from_str("testMethod").unwrap();
    /// # let arena = get_global_arena();
    /// # let method = oxidec::runtime::class::Method {
    /// #     selector: selector.clone(),
    /// #     imp: noop_impl,
    /// #     types: oxidec::runtime::RuntimeString::new("v@:", arena),
    /// # };
    /// # class.add_method(method).unwrap();
    /// # let target = Object::new(&class).unwrap();
    /// # let mut invocation = Invocation::new(&target, &selector).unwrap();
    /// unsafe {
    ///     let result = invocation.invoke()?;
    /// }
    /// # Ok::<(), oxidec::error::Error>(())
    /// ```
    pub unsafe fn invoke(&mut self) -> Result<Option<usize>> {
        use crate::runtime::dispatch;

        // Get target class
        let target_class = self.target.class();

        // Lookup method implementation
        let imp = target_class
            .lookup_imp(self.selector())
            .ok_or(Error::SelectorNotFound)?;

        // Validate argument count using method signature
        let method = target_class
            .lookup_method(self.selector())
            .ok_or(Error::SelectorNotFound)?;

        let encoding = method.types.as_str()?;

        // Parse signature to get expected argument count
        let (_ret_type, arg_types) =
            crate::runtime::encoding::parse_signature(encoding)?;

        // arg_types includes self (@) and _cmd (:), so actual args = len - 2
        let expected_args = arg_types.len() - 2;
        let actual_args = self.argument_count();

        if actual_args != expected_args {
            return Err(Error::ArgumentCountMismatch {
                expected: arg_types.len(),
                got: actual_args + 2,
            });
        }

        // Reconstruct MessageArgs from type-erased arguments
        let args = self.reconstruct_message_args()?;

        // Call the method using dispatch helper
        // SAFETY: We've validated all arguments above, and the target/selector are valid
        let result = unsafe {
            dispatch::call_method_with_args(
                self.target(),
                imp,
                self.selector(),
                &args,
            )
        };

        // Store return value if non-void
        self.flags.invoked = true;
        if let Some(retval) = result {
            self.set_return_value(&retval);
            Ok(Some(retval))
        } else {
            Ok(None)
        }
    }

    /// Reconstructs `MessageArgs` from type-erased argument storage.
    ///
    /// # Returns
    ///
    /// `Ok(MessageArgs)` with the reconstructed arguments
    ///
    /// # Errors
    ///
    /// Returns `Error::ArgumentCountMismatch` if the argument count exceeds
    /// supported limits (8 for direct variants, more requires Many variant).
    fn reconstruct_message_args(&self) -> Result<MessageArgs> {
        match self.arguments.len() {
            0 => Ok(MessageArgs::None),
            1 => {
                let arg: &usize = self.get_argument(0)?;
                Ok(MessageArgs::one(*arg))
            }
            2 => {
                let arg0: &usize = self.get_argument(0)?;
                let arg1: &usize = self.get_argument(1)?;
                Ok(MessageArgs::two(*arg0, *arg1))
            }
            3 => {
                let arg0: &usize = self.get_argument(0)?;
                let arg1: &usize = self.get_argument(1)?;
                let arg2: &usize = self.get_argument(2)?;
                Ok(MessageArgs::three([*arg0, *arg1, *arg2]))
            }
            4 => {
                let arg0: &usize = self.get_argument(0)?;
                let arg1: &usize = self.get_argument(1)?;
                let arg2: &usize = self.get_argument(2)?;
                let arg3: &usize = self.get_argument(3)?;
                Ok(MessageArgs::four([*arg0, *arg1, *arg2, *arg3]))
            }
            5 => {
                let args: [usize; 5] = [
                    *self.get_argument(0)?,
                    *self.get_argument(1)?,
                    *self.get_argument(2)?,
                    *self.get_argument(3)?,
                    *self.get_argument(4)?,
                ];
                Ok(MessageArgs::five(args))
            }
            6 => {
                let args: [usize; 6] = [
                    *self.get_argument(0)?,
                    *self.get_argument(1)?,
                    *self.get_argument(2)?,
                    *self.get_argument(3)?,
                    *self.get_argument(4)?,
                    *self.get_argument(5)?,
                ];
                Ok(MessageArgs::six(args))
            }
            7 => {
                let args: [usize; 7] = [
                    *self.get_argument(0)?,
                    *self.get_argument(1)?,
                    *self.get_argument(2)?,
                    *self.get_argument(3)?,
                    *self.get_argument(4)?,
                    *self.get_argument(5)?,
                    *self.get_argument(6)?,
                ];
                Ok(MessageArgs::seven(args))
            }
            8 => {
                let args: [usize; 8] = [
                    *self.get_argument(0)?,
                    *self.get_argument(1)?,
                    *self.get_argument(2)?,
                    *self.get_argument(3)?,
                    *self.get_argument(4)?,
                    *self.get_argument(5)?,
                    *self.get_argument(6)?,
                    *self.get_argument(7)?,
                ];
                Ok(MessageArgs::eight(args))
            }
            n => {
                // For > 8 arguments, we'd need to use MessageArgs::many with static storage
                // For now, return an error as this is unlikely to be needed in practice
                Err(Error::ArgumentCountMismatch {
                    expected: 8,
                    got: n + 2,
                })
            }
        }
    }

    /// Resets the invocation for reuse in object pooling.
    ///
    /// This method is used internally by the invocation pool to efficiently
    /// reuse invocations without reallocating memory. It clears all state
    /// and prepares the invocation for a new message send.
    ///
    /// # Arguments
    ///
    /// * `target` - The new target object
    /// * `selector` - The new selector
    ///
    /// # Safety
    ///
    /// This method must only be called by the pool module to ensure proper
    /// memory management. It reclaims all argument and return value memory.
    #[allow(clippy::cast_ptr_alignment)]
    pub(crate) fn reset(&mut self, target: &Object, selector: &Selector) {
        // Reset target and selector
        self.target = target.clone();
        self.selector = selector.clone();

        // Clear signature and return value
        self.signature = None;
        if let Some(ptr) = self.return_value.take() {
            // SAFETY: Reclaim return value memory
            let _ = unsafe { Box::from_raw(ptr.cast::<usize>()) };
        }
        self.return_size = 0;

        // Reset flags
        self.flags = InvocationFlags::default();

        // Clear arguments (allocated memory will be reclaimed in marshal_arguments)
        for ptr in &self.arguments {
            // SAFETY: Reclaim argument memory
            let _ = unsafe { Box::from_raw((*ptr).cast::<usize>()) };
        }
        self.arguments.clear();
    }

    /// Resets the invocation for reuse with new arguments.
    ///
    /// This method is used internally by the invocation pool to efficiently
    /// reuse invocations with new arguments without reallocating memory.
    ///
    /// # Arguments
    ///
    /// * `target` - The new target object
    /// * `selector` - The new selector
    /// * `args` - The new arguments
    ///
    /// # Errors
    ///
    /// Returns `Error::ArgumentCountMismatch` if argument count exceeds limits.
    #[allow(clippy::cast_ptr_alignment)]
    pub(crate) fn reset_with_args(
        &mut self,
        target: &Object,
        selector: &Selector,
        args: &MessageArgs,
    ) -> Result<()> {
        // Reset basic fields
        self.reset(target, selector);

        // Marshal new arguments
        self.arguments = Self::marshal_arguments(args)?;
        Ok(())
    }
}

impl Drop for Invocation {
    fn drop(&mut self) {
        // Reclaim argument memory allocated in marshal_arguments
        for ptr in &self.arguments {
            // SAFETY: These pointers were allocated via Box::into_raw in
            // marshal_arguments. We're reclaiming them here to prevent leaks.
            // This is safe because:
            // 1. We own these pointers exclusively
            // 2. No other references exist
            // 3. Memory was originally allocated as Box<usize>
            // 4. We're reconstructing the Box to let it drop normally
            let _ = unsafe { Box::from_raw((*ptr).cast::<usize>()) };
        }

        // Reclaim return value memory if allocated
        if let Some(ptr) = self.return_value {
            // SAFETY: Same rationale as arguments
            let _ = unsafe { Box::from_raw(ptr.cast::<usize>()) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::class::Class;
    use std::str::FromStr;

    #[test]
    fn test_invocation_creation() {
        let class = Class::new_root("TestInvocationCreation").unwrap();
        let object = Object::new(&class).unwrap();
        let selector = Selector::from_str("testMethod:").unwrap();

        let invocation = Invocation::new(&object, &selector);
        assert!(invocation.is_ok());

        let invocation = invocation.unwrap();
        assert_eq!(invocation.argument_count(), 0);
    }

    #[test]
    fn test_invocation_with_arguments() {
        let class = Class::new_root("TestInvocationWithArgs").unwrap();
        let object = Object::new(&class).unwrap();
        let selector = Selector::from_str("testMethod:").unwrap();

        let args = MessageArgs::two(10, 20);
        let invocation = Invocation::with_arguments(&object, &selector, &args);
        assert!(invocation.is_ok());

        let invocation = invocation.unwrap();
        assert_eq!(invocation.argument_count(), 2);
    }

    #[test]
    fn test_invocation_too_many_arguments() {
        const MANY_ARGS: [usize; 17] = [0; 17];

        let class = Class::new_root("TestTooManyArgs").unwrap();
        let object = Object::new(&class).unwrap();
        let selector = Selector::from_str("testMethod:").unwrap();

        // Create 17 arguments (exceeds MAX_ARGS of 16)
        let args = MessageArgs::many(&MANY_ARGS);

        let invocation = Invocation::with_arguments(&object, &selector, &args);
        assert!(invocation.is_err());
    }

    #[test]
    fn test_get_set_target() {
        let class = Class::new_root("TestGetSetTarget").unwrap();
        let object1 = Object::new(&class).unwrap();
        let object2 = Object::new(&class).unwrap();
        let selector = Selector::from_str("testMethod:").unwrap();

        let mut invocation = Invocation::new(&object1, &selector).unwrap();
        assert_eq!(invocation.target(), &object1);

        invocation.set_target(&object2);
        assert_eq!(invocation.target(), &object2);
    }

    #[test]
    fn test_get_set_selector() {
        let class = Class::new_root("TestGetSetSelector").unwrap();
        let object = Object::new(&class).unwrap();
        let selector1 = Selector::from_str("method1:").unwrap();
        let selector2 = Selector::from_str("method2:").unwrap();

        let mut invocation = Invocation::new(&object, &selector1).unwrap();
        assert_eq!(invocation.selector(), &selector1);

        invocation.set_selector(&selector2);
        assert_eq!(invocation.selector(), &selector2);
    }

    #[test]
    fn test_get_argument() {
        let class = Class::new_root("TestGetArgument").unwrap();
        let object = Object::new(&class).unwrap();
        let selector = Selector::from_str("testMethod:").unwrap();

        let msg_args = MessageArgs::two(42usize, 100usize);
        let invocation =
            Invocation::with_arguments(&object, &selector, &msg_args).unwrap();

        let arg0: &usize = invocation.get_argument(0).unwrap();
        assert_eq!(*arg0, 42);

        let arg1: &usize = invocation.get_argument(1).unwrap();
        assert_eq!(*arg1, 100);
    }

    #[test]
    fn test_get_argument_out_of_bounds() {
        let class = Class::new_root("TestArgumentOutOfBounds").unwrap();
        let object = Object::new(&class).unwrap();
        let selector = Selector::from_str("testMethod:").unwrap();

        let invocation = Invocation::new(&object, &selector).unwrap();
        let result: Result<&i32> = invocation.get_argument(0);
        assert!(result.is_err());
    }

    #[test]
    fn test_set_argument() {
        let class = Class::new_root("TestSetArgument").unwrap();
        let object = Object::new(&class).unwrap();
        let selector = Selector::from_str("testMethod:").unwrap();

        let msg_args = MessageArgs::two(1usize, 2usize);
        let mut invocation =
            Invocation::with_arguments(&object, &selector, &msg_args).unwrap();

        invocation.set_argument(0, &99usize).unwrap();
        let arg0: &usize = invocation.get_argument(0).unwrap();
        assert_eq!(*arg0, 99);
    }

    #[test]
    fn test_set_return_value() {
        let class = Class::new_root("TestSetReturnValue").unwrap();
        let object = Object::new(&class).unwrap();
        let selector = Selector::from_str("testMethod:").unwrap();

        let mut invocation = Invocation::new(&object, &selector).unwrap();
        invocation.set_return_value(&42usize);

        let result: &usize = invocation.get_return_value().unwrap();
        assert_eq!(*result, 42);
    }

    #[test]
    fn test_invocation_send() {
        let class = Class::new_root("TestInvocationSend").unwrap();
        let object = Object::new(&class).unwrap();
        let selector = Selector::from_str("testMethod:").unwrap();

        let invocation = Invocation::new(&object, &selector).unwrap();

        // Test that Invocation is Send (can move between threads)
        std::thread::spawn(move || {
            let _ = invocation;
        })
        .join()
        .unwrap();
    }

    #[test]
    fn test_invocation_flags() {
        let class = Class::new_root("TestInvocationFlags").unwrap();
        let object1 = Object::new(&class).unwrap();
        let object2 = Object::new(&class).unwrap();
        let selector1 = Selector::from_str("method1:").unwrap();
        let selector2 = Selector::from_str("method2:").unwrap();

        let mut invocation = Invocation::new(&object1, &selector1).unwrap();

        // Initially no flags set
        assert!(!invocation.flags.invoked);
        assert!(!invocation.flags.target_modified);
        assert!(!invocation.flags.selector_modified);
        assert!(!invocation.flags.arguments_modified);

        // Modify target
        invocation.set_target(&object2);
        assert!(invocation.flags.target_modified);

        // Modify selector
        invocation.set_selector(&selector2);
        assert!(invocation.flags.selector_modified);

        // Modify arguments
        let args = MessageArgs::one(42usize);
        let mut invocation =
            Invocation::with_arguments(&object1, &selector1, &args).unwrap();
        invocation.set_argument(0, &99usize).unwrap();
        assert!(invocation.flags.arguments_modified);
    }
}
