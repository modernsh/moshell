/// Joins two slices that are adjacent in memory into one slice.
///
/// Returns None in the case the slices aren't adjacent.
pub fn try_join<'a, T>(a: &'a [T], b: &'a [T]) -> Option<&'a [T]> {
    let a_len = a.len();
    let a_tail = a[a_len..].as_ptr();
    if std::ptr::eq(a_tail, b.as_ptr()) {
        // SAFETY: The two slices are adjacent in memory.
        Some(unsafe { std::slice::from_raw_parts(a.as_ptr(), a.len() + b.len()) })
    } else {
        None
    }
}

/// Joins two string slices that are adjacent in memory into one string slice.
///
/// Returns None in the case the slices aren't adjacent.
pub fn try_join_str<'a>(a: &'a str, b: &'a str) -> Option<&'a str> {
    // SAFETY: The two slices are already valid UTF-8.
    try_join(a.as_bytes(), b.as_bytes()).map(|s| unsafe { std::str::from_utf8_unchecked(s) })
}