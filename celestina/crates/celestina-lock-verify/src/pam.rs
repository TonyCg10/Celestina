//! The only module of the verifier with `unsafe` (design §4.11, H-3): the
//! hand-written `libpam` declarations, the conversation callback and the
//! allocation of its responses. The project's second documented `unsafe`
//! exception, after `celestina-wayland/src/egl.rs`; `celestina/AGENTS.md`
//! "Local boundary" records it, and every block below states the invariant it
//! relies on.
//!
//! What lives here is only what cannot be written safely: the extern
//! declarations, the pointer handling of `struct pam_conv`/`pam_message`/
//! `pam_response`, and the `malloc`/`free` PAM's contract requires. Which
//! prompt gets the secret is decided by the pure `conversation` module; this
//! module only reads the message *styles* (never the text), asks it, and
//! builds the answer. Responses are `malloc`'d and NUL-terminated because PAM
//! frees them with `free`.
//!
//! Panics cannot unwind out of the `extern "C"` callback: since Rust 1.81 they
//! abort the process, so nothing crosses the boundary and no state is left
//! half-built for PAM to read.
//!
//! The calls made are `pam_start`, `pam_authenticate`, `pam_acct_mgmt` and
//! `pam_end`. There is no declaration for any other `libpam` function.
#![allow(unsafe_code)]
#![deny(unsafe_op_in_unsafe_fn)]

use std::ffi::{c_char, c_int, c_void, CString};
use std::mem::size_of;
use std::ptr::{self, NonNull};
use std::sync::{Arc, Mutex, PoisonError};

use zeroize::Zeroize;

use crate::consts::{
    PAM_BUF_ERR, PAM_CONV_ERR, PAM_MAX_NUM_MSG, PAM_MAX_RESP_SIZE, PAM_SUCCESS, PAM_SYSTEM_ERR,
};
use crate::conversation::{ConvState, Reply};
use crate::session::{PamApi, SharedConv};

/// `struct pam_message`. Only `msg_style` is ever read; the text is not.
#[repr(C)]
struct PamMessage {
    msg_style: c_int,
    msg: *const c_char,
}

/// `struct pam_response`. PAM frees `resp` with `free`.
#[repr(C)]
struct PamResponse {
    resp: *mut c_char,
    resp_retcode: c_int,
}

type ConvFn = unsafe extern "C" fn(
    num_msg: c_int,
    msg: *mut *const PamMessage,
    resp: *mut *mut PamResponse,
    appdata_ptr: *mut c_void,
) -> c_int;

/// `struct pam_conv`.
#[repr(C)]
struct PamConv {
    conv: ConvFn,
    appdata_ptr: *mut c_void,
}

/// `pam_handle_t`, opaque.
#[repr(C)]
struct RawHandle {
    _private: [u8; 0],
}

#[link(name = "pam")]
unsafe extern "C" {
    fn pam_start(
        service_name: *const c_char,
        user: *const c_char,
        pam_conversation: *const PamConv,
        pamh: *mut *mut RawHandle,
    ) -> c_int;
    fn pam_authenticate(pamh: *mut RawHandle, flags: c_int) -> c_int;
    fn pam_acct_mgmt(pamh: *mut RawHandle, flags: c_int) -> c_int;
    fn pam_end(pamh: *mut RawHandle, pam_status: c_int) -> c_int;
}

/// The allocator PAM's responses come from. `libc`'s in production; the tests
/// inject one that fails on demand and counts what is freed.
trait ResponseAlloc {
    /// # Safety
    ///
    /// As `calloc(3)`.
    unsafe fn calloc(&self, count: usize, size: usize) -> *mut c_void;
    /// # Safety
    ///
    /// As `malloc(3)`.
    unsafe fn malloc(&self, size: usize) -> *mut c_void;
    /// Free a block of `len` bytes obtained from this allocator.
    ///
    /// # Safety
    ///
    /// `ptr` came from this allocator's `calloc`/`malloc` with that `len`, and
    /// is not used afterwards.
    unsafe fn free(&self, ptr: *mut c_void, len: usize);
}

struct LibcAlloc;

impl ResponseAlloc for LibcAlloc {
    unsafe fn calloc(&self, count: usize, size: usize) -> *mut c_void {
        // SAFETY: plain `calloc`; the caller handles a NULL return.
        unsafe { libc::calloc(count, size) }
    }

    unsafe fn malloc(&self, size: usize) -> *mut c_void {
        // SAFETY: plain `malloc`; the caller handles a NULL return.
        unsafe { libc::malloc(size) }
    }

    unsafe fn free(&self, ptr: *mut c_void, _len: usize) {
        // SAFETY: the caller guarantees `ptr` is a live block from `malloc` or
        // `calloc` above and gives it up here.
        unsafe { libc::free(ptr) }
    }
}

/// Free the first `built` secret copies of `array` (each `secret_len + 1`
/// bytes, wiped first) and the array itself.
///
/// # Safety
///
/// `array` holds at least `built` initialised `PamResponse` entries obtained
/// from `alloc.calloc`, every non-NULL `resp` among them from `alloc.malloc`
/// with `secret_len + 1` bytes; none is used afterwards.
unsafe fn free_responses<A: ResponseAlloc>(
    array: NonNull<PamResponse>,
    built: usize,
    secret_len: usize,
    count: usize,
    alloc: &A,
) {
    for i in 0..built {
        // SAFETY: `i < built <= count`, inside the array (see the contract).
        let resp = unsafe { (*array.as_ptr().add(i)).resp };
        if resp.is_null() {
            continue;
        }
        // SAFETY: `resp` is a live block of `secret_len + 1` bytes (contract).
        // `zeroize` writes volatilely, so the wipe is not optimised away.
        unsafe { std::slice::from_raw_parts_mut(resp.cast::<u8>(), secret_len + 1) }.zeroize();
        // SAFETY: the block came from `alloc.malloc` with that length.
        unsafe { alloc.free(resp.cast(), secret_len + 1) };
    }
    // SAFETY: the array came from `alloc.calloc(count, size_of::<PamResponse>())`.
    unsafe { alloc.free(array.as_ptr().cast(), count * size_of::<PamResponse>()) };
}

/// Build the response array for `replies`: a zeroed array of `PamResponse`,
/// with a NUL-terminated copy of `secret` where the reply is [`Reply::Secret`]
/// and `resp = NULL` elsewhere. On any allocation failure everything already
/// built is wiped and freed, and `None` is returned.
///
/// # Safety
///
/// `alloc` must behave as `libc`'s allocator for the blocks it hands out.
unsafe fn build_responses<A: ResponseAlloc>(
    replies: &[Reply],
    secret: &[u8],
    alloc: &A,
) -> Option<NonNull<PamResponse>> {
    let count = replies.len();
    // SAFETY: `calloc` with a non-zero count and the struct size; NULL is
    // handled right below.
    let array = unsafe { alloc.calloc(count, size_of::<PamResponse>()) };
    let array = NonNull::new(array.cast::<PamResponse>())?;
    for (index, reply) in replies.iter().enumerate() {
        if *reply != Reply::Secret {
            continue;
        }
        // SAFETY: `secret.len() + 1` is at most 513; NULL is handled below.
        let copy = unsafe { alloc.malloc(secret.len() + 1) }.cast::<u8>();
        let Some(copy) = NonNull::new(copy) else {
            // SAFETY: entries `0..index` were filled by this loop from
            // `alloc.malloc(secret.len() + 1)`; the array is `count` long.
            unsafe { free_responses(array, index, secret.len(), count, alloc) };
            return None;
        };
        // SAFETY: `copy` is a fresh block of `secret.len() + 1` bytes that
        // does not overlap `secret`; the last byte is the terminator.
        unsafe {
            ptr::copy_nonoverlapping(secret.as_ptr(), copy.as_ptr(), secret.len());
            *copy.as_ptr().add(secret.len()) = 0;
        }
        // SAFETY: `index < count`, inside the zeroed array.
        unsafe { (*array.as_ptr().add(index)).resp = copy.as_ptr().cast() };
    }
    Some(array)
}

/// The body of the conversation function, over an injectable allocator.
///
/// # Safety
///
/// The arguments are those `pam_conv(3)` documents for the callback: `msg`
/// points to `num_msg` pointers to readable `PamMessage`s (or is NULL), `resp`
/// is a writable pointer (or NULL) and `appdata_ptr` is the pointer stored in
/// `PamConv` by [`RealPam::start`] (or NULL): a `*const Mutex<ConvState>` that
/// stays valid for the whole call.
unsafe fn converse<A: ResponseAlloc>(
    num_msg: c_int,
    msg: *mut *const PamMessage,
    resp: *mut *mut PamResponse,
    appdata_ptr: *mut c_void,
    alloc: &A,
) -> c_int {
    if resp.is_null() {
        return PAM_CONV_ERR;
    }
    // SAFETY: `resp` is non-NULL and writable (contract). Answering with NULL
    // first means every early return leaves PAM nothing to free.
    unsafe { *resp = ptr::null_mut() };
    if msg.is_null() || appdata_ptr.is_null() {
        return PAM_CONV_ERR;
    }
    let count = match usize::try_from(num_msg) {
        Ok(count) if (1..=PAM_MAX_NUM_MSG).contains(&count) => count,
        _ => return PAM_CONV_ERR,
    };
    let mut styles = [0_i32; PAM_MAX_NUM_MSG];
    for (index, style) in styles.iter_mut().enumerate().take(count) {
        // SAFETY: `msg` points to `num_msg` pointers and `index < count`.
        let message = unsafe { *msg.add(index) };
        if message.is_null() {
            return PAM_CONV_ERR;
        }
        // SAFETY: a non-NULL entry points to a readable `pam_message`. Only
        // the style is read; the text pointer is never followed.
        *style = unsafe { (*message).msg_style };
    }
    // SAFETY: `appdata_ptr` is the `Arc::as_ptr` of the `Mutex<ConvState>` the
    // session keeps alive until after `pam_end` (contract); a shared reference
    // is enough, the `Mutex` provides the mutation.
    let cell = unsafe { &*appdata_ptr.cast::<Mutex<ConvState>>().cast_const() };
    let mut state = cell.lock().unwrap_or_else(PoisonError::into_inner);
    let Ok(replies) = state.conversation.respond(&styles[..count]) else {
        return PAM_CONV_ERR;
    };
    let secret: &[u8] = match state.secret_bytes() {
        Some(bytes) => bytes,
        None if replies.contains(&Reply::Secret) => return PAM_CONV_ERR,
        None => &[],
    };
    // PAM's own bound on a response: the secret already fits (512 bytes), and
    // anything longer is not handed out.
    if secret.len() > PAM_MAX_RESP_SIZE {
        return PAM_CONV_ERR;
    }
    // SAFETY: `alloc` is the production allocator or a test double that
    // honours the allocator contract.
    match unsafe { build_responses(&replies, secret, alloc) } {
        Some(array) => {
            // SAFETY: `resp` is writable (checked above); PAM now owns `array`.
            unsafe { *resp = array.as_ptr() };
            PAM_SUCCESS
        }
        None => PAM_BUF_ERR,
    }
}

/// The function PAM calls. A thin shim over [`converse`].
unsafe extern "C" fn conversation(
    num_msg: c_int,
    msg: *mut *const PamMessage,
    resp: *mut *mut PamResponse,
    appdata_ptr: *mut c_void,
) -> c_int {
    // SAFETY: PAM calls the function it was given with the arguments of
    // `pam_conv(3)`, and `appdata_ptr` is the one `RealPam::start` stored.
    unsafe { converse(num_msg, msg, resp, appdata_ptr, &LibcAlloc) }
}

/// The real `libpam`.
pub struct RealPam;

/// A started PAM transaction and everything PAM may still read from it.
pub struct Session {
    raw: NonNull<RawHandle>,
    ended: bool,
    // PAM copies or keeps the `pam_conv` and calls back with `appdata_ptr`
    // until `pam_end`; both stay alive until the session is dropped, which
    // happens after `pam_end`.
    _conv: Box<PamConv>,
    _state: SharedConv,
}

impl Drop for Session {
    fn drop(&mut self) {
        if !self.ended {
            // SAFETY: `raw` came from a successful `pam_start` and was not
            // ended; it is not used again.
            unsafe { pam_end(self.raw.as_ptr(), crate::consts::PAM_ABORT) };
        }
    }
}

impl PamApi for RealPam {
    type Handle = Session;

    fn start(&mut self, service: &str, user: &str, conv: SharedConv) -> Result<Session, i32> {
        let (Ok(service), Ok(user)) = (CString::new(service), CString::new(user)) else {
            return Err(PAM_SYSTEM_ERR);
        };
        let appdata = Arc::as_ptr(&conv).cast_mut().cast::<c_void>();
        let pam_conv = Box::new(PamConv {
            conv: conversation,
            appdata_ptr: appdata,
        });
        let mut raw: *mut RawHandle = ptr::null_mut();
        // SAFETY: `service` and `user` are NUL-terminated and outlive the
        // call; `pam_conv` is a valid `struct pam_conv` that this session
        // keeps alive; `raw` is a valid out-pointer.
        let status = unsafe {
            pam_start(
                service.as_ptr(),
                user.as_ptr(),
                &raw const *pam_conv,
                &raw mut raw,
            )
        };
        if status != PAM_SUCCESS {
            return Err(status);
        }
        let raw = NonNull::new(raw).ok_or(PAM_SYSTEM_ERR)?;
        Ok(Session {
            raw,
            ended: false,
            _conv: pam_conv,
            _state: conv,
        })
    }

    fn authenticate(&mut self, session: &mut Session, flags: i32) -> i32 {
        // SAFETY: `raw` is a live handle from `pam_start`, not yet ended.
        unsafe { pam_authenticate(session.raw.as_ptr(), flags) }
    }

    fn acct_mgmt(&mut self, session: &mut Session, flags: i32) -> i32 {
        // SAFETY: `raw` is a live handle from `pam_start`, not yet ended.
        unsafe { pam_acct_mgmt(session.raw.as_ptr(), flags) }
    }

    fn end(&mut self, mut session: Session, status: i32) -> i32 {
        // SAFETY: `raw` is a live handle from `pam_start`; `ended` is set at
        // once so `Drop` does not end it a second time.
        let result = unsafe { pam_end(session.raw.as_ptr(), status) };
        session.ended = true;
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::consts::{PAM_ERROR_MSG, PAM_PROMPT_ECHO_OFF, PAM_PROMPT_ECHO_ON, PAM_TEXT_INFO};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use zeroize::Zeroizing;

    /// An allocator over `libc` that fails the `fail_malloc`-th `malloc`
    /// (1-based) and remembers every live block, so a test can assert that
    /// nothing leaked and that secret copies were wiped before they were freed.
    struct Counting {
        fail_calloc: bool,
        fail_malloc: Option<usize>,
        mallocs: RefCell<usize>,
        live: RefCell<HashMap<usize, (usize, bool)>>,
        freed_secret_copies_zeroed: RefCell<Vec<bool>>,
    }

    impl Counting {
        fn new() -> Self {
            Self {
                fail_calloc: false,
                fail_malloc: None,
                mallocs: RefCell::new(0),
                live: RefCell::new(HashMap::new()),
                freed_secret_copies_zeroed: RefCell::new(Vec::new()),
            }
        }
        fn leaked(&self) -> usize {
            self.live.borrow().len()
        }
    }

    impl ResponseAlloc for Counting {
        unsafe fn calloc(&self, count: usize, size: usize) -> *mut c_void {
            if self.fail_calloc {
                return ptr::null_mut();
            }
            // SAFETY: plain `calloc`.
            let block = unsafe { libc::calloc(count, size) };
            self.live
                .borrow_mut()
                .insert(block as usize, (count * size, false));
            block
        }

        unsafe fn malloc(&self, size: usize) -> *mut c_void {
            *self.mallocs.borrow_mut() += 1;
            if Some(*self.mallocs.borrow()) == self.fail_malloc {
                return ptr::null_mut();
            }
            // SAFETY: plain `malloc`; the block is filled with 0xAA so that a
            // missing wipe is visible.
            let block = unsafe { libc::malloc(size) };
            unsafe { ptr::write_bytes(block.cast::<u8>(), 0xAA, size) };
            self.live.borrow_mut().insert(block as usize, (size, true));
            block
        }

        unsafe fn free(&self, block: *mut c_void, len: usize) {
            let (size, secret_copy) = self
                .live
                .borrow_mut()
                .remove(&(block as usize))
                .expect("freed a block that is not live");
            assert_eq!(size, len, "free length");
            if secret_copy {
                // SAFETY: the block is live and `len` bytes long.
                let bytes = unsafe { std::slice::from_raw_parts(block.cast::<u8>(), len) };
                self.freed_secret_copies_zeroed
                    .borrow_mut()
                    .push(bytes.iter().all(|byte| *byte == 0));
            }
            // SAFETY: the block came from `libc` above.
            unsafe { libc::free(block) };
        }
    }

    fn state_with(secret: &[u8]) -> Mutex<ConvState> {
        Mutex::new(ConvState::new(Zeroizing::new(secret.to_vec())))
    }

    /// Call `converse` with messages of the given styles. Returns the status
    /// and what `*resp` holds afterwards.
    fn call<A: ResponseAlloc>(
        styles: &[i32],
        state: &Mutex<ConvState>,
        alloc: &A,
    ) -> (c_int, *mut PamResponse) {
        let messages: Vec<PamMessage> = styles
            .iter()
            .map(|style| PamMessage {
                msg_style: *style,
                msg: ptr::null(),
            })
            .collect();
        let mut pointers: Vec<*const PamMessage> = messages.iter().map(ptr::from_ref).collect();
        let mut resp: *mut PamResponse = ptr::dangling_mut(); // proves it is reset
        let num = c_int::try_from(styles.len()).expect("small");
        // SAFETY: `pointers` holds `num` pointers to live messages, `resp` is
        // writable and the state outlives the call.
        let status = unsafe {
            converse(
                num,
                pointers.as_mut_ptr(),
                &raw mut resp,
                ptr::from_ref(state).cast_mut().cast(),
                alloc,
            )
        };
        (status, resp)
    }

    fn free_array(array: *mut PamResponse, count: usize) {
        for index in 0..count {
            // SAFETY: the array has `count` entries built by `converse`.
            let resp = unsafe { (*array.add(index)).resp };
            // SAFETY: each non-NULL `resp` was `malloc`'d.
            unsafe { libc::free(resp.cast()) };
        }
        // SAFETY: the array was `calloc`'d.
        unsafe { libc::free(array.cast()) };
    }

    #[test]
    fn the_secret_answers_the_echo_off_prompt_as_a_malloced_nul_terminated_copy() {
        let state = state_with(b"hunter2");
        let (status, array) = call(&[PAM_PROMPT_ECHO_OFF], &state, &LibcAlloc);
        assert_eq!(status, PAM_SUCCESS);
        assert!(!array.is_null());
        // SAFETY: one entry was built.
        let text = unsafe { std::ffi::CStr::from_ptr((*array).resp) };
        assert_eq!(text.to_bytes(), b"hunter2");
        // SAFETY: as above.
        assert_eq!(unsafe { (*array).resp_retcode }, 0);
        free_array(array, 1);
    }

    #[test]
    fn messages_are_answered_with_a_null_response_and_the_secret_goes_to_the_prompt() {
        let state = state_with(b"pw");
        let styles = [PAM_TEXT_INFO, PAM_PROMPT_ECHO_OFF, PAM_ERROR_MSG];
        let (status, array) = call(&styles, &state, &LibcAlloc);
        assert_eq!(status, PAM_SUCCESS);
        // SAFETY: three entries were built.
        let resps: Vec<*mut c_char> = (0..3).map(|i| unsafe { (*array.add(i)).resp }).collect();
        assert!(resps[0].is_null());
        assert!(!resps[1].is_null());
        assert!(resps[2].is_null());
        free_array(array, 3);
        assert_eq!(state.lock().unwrap().conversation.messages_seen(), 2);
    }

    #[test]
    fn refused_calls_answer_conv_err_and_leave_resp_null() {
        for styles in [
            &[PAM_PROMPT_ECHO_ON][..],
            &[PAM_PROMPT_ECHO_OFF, PAM_PROMPT_ECHO_OFF],
            &[99],
            &[PAM_PROMPT_ECHO_OFF, PAM_PROMPT_ECHO_ON],
        ] {
            let state = state_with(b"pw");
            let counting = Counting::new();
            let (status, array) = call(styles, &state, &counting);
            assert_eq!(status, PAM_CONV_ERR, "{styles:?}");
            assert!(array.is_null(), "{styles:?}");
            assert_eq!(counting.leaked(), 0, "{styles:?}");
            assert_eq!(
                *counting.mallocs.borrow(),
                0,
                "no copy was made: {styles:?}"
            );
        }
    }

    #[test]
    fn num_msg_zero_negative_and_thirty_three_are_conv_err() {
        for num in [0, -1, 33, i32::MAX, i32::MIN] {
            let state = state_with(b"pw");
            let mut resp: *mut PamResponse = ptr::dangling_mut();
            let mut one = PamMessage {
                msg_style: PAM_TEXT_INFO,
                msg: ptr::null(),
            };
            let mut pointers: [*const PamMessage; 1] = [&raw mut one];
            // SAFETY: the count is wrong on purpose but is rejected before
            // any message is read; every pointer is valid.
            let status = unsafe {
                converse(
                    num,
                    pointers.as_mut_ptr(),
                    &raw mut resp,
                    ptr::from_ref(&state).cast_mut().cast(),
                    &LibcAlloc,
                )
            };
            assert_eq!(status, PAM_CONV_ERR, "{num}");
            assert!(resp.is_null(), "{num}");
        }
    }

    #[test]
    fn null_pointers_are_conv_err() {
        let state = state_with(b"pw");
        let appdata: *mut c_void = ptr::from_ref(&state).cast_mut().cast();
        let mut resp: *mut PamResponse = ptr::null_mut();
        let mut message = PamMessage {
            msg_style: PAM_TEXT_INFO,
            msg: ptr::null(),
        };
        let mut pointers: [*const PamMessage; 1] = [&raw mut message];
        let mut null_entry: [*const PamMessage; 1] = [ptr::null()];
        // SAFETY (all four): NULL arguments are part of the contract being
        // tested; the non-NULL ones are valid.
        unsafe {
            assert_eq!(
                converse(
                    1,
                    pointers.as_mut_ptr(),
                    ptr::null_mut(),
                    appdata,
                    &LibcAlloc
                ),
                PAM_CONV_ERR
            );
            assert_eq!(
                converse(1, ptr::null_mut(), &raw mut resp, appdata, &LibcAlloc),
                PAM_CONV_ERR
            );
            assert_eq!(
                converse(
                    1,
                    pointers.as_mut_ptr(),
                    &raw mut resp,
                    ptr::null_mut(),
                    &LibcAlloc
                ),
                PAM_CONV_ERR
            );
            assert_eq!(
                converse(
                    1,
                    null_entry.as_mut_ptr(),
                    &raw mut resp,
                    appdata,
                    &LibcAlloc
                ),
                PAM_CONV_ERR
            );
        }
        assert!(resp.is_null());
    }

    #[test]
    fn no_secret_left_means_conv_err() {
        let state = state_with(b"pw");
        state.lock().unwrap().drop_secret();
        let (status, array) = call(&[PAM_PROMPT_ECHO_OFF], &state, &LibcAlloc);
        assert_eq!(status, PAM_CONV_ERR);
        assert!(array.is_null());
    }

    #[test]
    fn allocation_failure_frees_every_response_built() {
        // A failing `calloc`: nothing was built, nothing leaks.
        let mut counting = Counting::new();
        counting.fail_calloc = true;
        // SAFETY: the double honours the allocator contract.
        let built = unsafe { build_responses(&[Reply::Secret], b"pw", &counting) };
        assert!(built.is_none());
        assert_eq!(counting.leaked(), 0);

        // The second secret copy fails: the first is wiped and freed, and so
        // is the array.
        let mut counting = Counting::new();
        counting.fail_malloc = Some(2);
        let replies = [Reply::Secret, Reply::Empty, Reply::Secret];
        // SAFETY: as above.
        let built = unsafe { build_responses(&replies, b"hunter2", &counting) };
        assert!(built.is_none());
        assert_eq!(counting.leaked(), 0, "a block leaked");
        assert_eq!(
            counting.freed_secret_copies_zeroed.borrow().as_slice(),
            [true],
            "the first copy was freed without being wiped"
        );

        // The first copy fails.
        let mut counting = Counting::new();
        counting.fail_malloc = Some(1);
        // SAFETY: as above.
        assert!(unsafe { build_responses(&replies, b"hunter2", &counting) }.is_none());
        assert_eq!(counting.leaked(), 0);

        // Through the conversation: BUF_ERR, resp stays NULL, nothing leaks.
        let state = state_with(b"hunter2");
        let mut counting = Counting::new();
        counting.fail_malloc = Some(1);
        let (status, array) = call(&[PAM_PROMPT_ECHO_OFF], &state, &counting);
        assert_eq!(status, PAM_BUF_ERR);
        assert!(array.is_null());
        assert_eq!(counting.leaked(), 0);
    }

    #[test]
    fn a_successful_build_hands_everything_to_pam_and_nothing_stays_here() {
        let state = state_with(b"hunter2");
        let counting = Counting::new();
        let (status, array) = call(&[PAM_PROMPT_ECHO_OFF], &state, &counting);
        assert_eq!(status, PAM_SUCCESS);
        // The array and one copy are live: PAM owns them now.
        assert_eq!(counting.leaked(), 2);
        for block in counting.live.borrow().keys() {
            // SAFETY: the blocks are live `libc` allocations that PAM would
            // free; the test frees them directly.
            unsafe { libc::free(*block as *mut c_void) };
        }
        assert!(!array.is_null());
    }

    #[test]
    fn t_v5_the_declared_layouts_match_the_c_structs() {
        // struct pam_message { int; const char * }, pam_response { char *; int },
        // pam_conv { fn *; void * } on LP64.
        assert_eq!(size_of::<PamMessage>(), 16);
        assert_eq!(size_of::<PamResponse>(), 16);
        assert_eq!(size_of::<PamConv>(), 16);
        assert_eq!(std::mem::offset_of!(PamMessage, msg), 8);
        assert_eq!(std::mem::offset_of!(PamResponse, resp_retcode), 8);
        assert_eq!(std::mem::offset_of!(PamConv, appdata_ptr), 8);
    }
}
