use crate::LOGGING_TAG_MAX_LEN;
use std::ffi::CStr;
use std::mem::MaybeUninit;

// FIXME: When `maybe_uninit_uninit_array` is stabilized, use it instead of this helper
pub fn uninit_array<const N: usize, T>() -> [MaybeUninit<T>; N] {
    // SAFETY: Array contains MaybeUninit, which is fine to be uninit
    unsafe { MaybeUninit::uninit().assume_init() }
}

// FIXME: Remove when maybe_uninit_slice is stabilized to provide MaybeUninit::slice_assume_init_ref()
pub unsafe fn slice_assume_init_ref<T>(slice: &[MaybeUninit<T>]) -> &[T] {
    &*(slice as *const [MaybeUninit<T>] as *const [T])
}

/// Fills up `storage` with `tag` and a necessary NUL terminator, optionally ellipsizing the input
/// `tag` if it's too large.
///
/// Returns a [`CStr`] containing the initialized portion of `storage`, including its NUL
/// terminator.
pub fn fill_tag_bytes<'a>(
    storage: &'a mut [MaybeUninit<u8>; LOGGING_TAG_MAX_LEN + 1],
    tag: &[u8],
) -> &'a CStr {
    // FIXME: Simplify when maybe_uninit_fill with MaybeUninit::fill_from() is stabilized
    let initialized = if tag.len() > LOGGING_TAG_MAX_LEN {
        for (input, output) in tag
            .iter()
            // Elipsize the last two characters (TODO: use special … character)?
            .take(LOGGING_TAG_MAX_LEN - 2)
            .chain(b"..\0")
            .zip(storage.iter_mut())
        {
            output.write(*input);
        }
        storage.as_slice()
    } else {
        for (input, output) in tag.iter().chain(b"\0").zip(storage.iter_mut()) {
            output.write(*input);
        }
        &storage[..tag.len() + 1]
    };

    // SAFETY: The above code ensures that `initialized` only refers to a portion of the `array`
    // slice that was initialized, thus it is safe to cast those `MaybeUninit<u8>`s to `u8`:
    let initialized = unsafe { slice_assume_init_ref(initialized) };
    CStr::from_bytes_with_nul(initialized).expect("Unreachable: we wrote a nul terminator")
}
