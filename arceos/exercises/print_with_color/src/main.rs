#![cfg_attr(feature = "axstd", no_std)]
#![cfg_attr(feature = "axstd", no_main)]

#[cfg(feature = "axstd")]
use axstd::println;

/// ANSI SGR escape sequences understood by any VT100-compatible terminal, and
/// by the QEMU serial console used to run this exercise.
///
/// `ESC[1;32m` selects bold + green foreground, `ESC[0m` resets every
/// attribute so the color does not leak into the following output.
const COLOR_START: &str = "\x1b[1;32m";
const COLOR_END: &str = "\x1b[0m";

#[cfg_attr(feature = "axstd", no_mangle)]
fn main() {
    // The color is applied at the application level on purpose: `axstd::println!`
    // forwards the bytes to the console verbatim, so escape sequences pass
    // straight through. The same green could be produced inside `axstd::io` or
    // `axhal`'s console writer, but those components are shared by every ArceOS
    // application (and by the boot log), so the color would spread far beyond
    // this greeting.
    println!("{}[WithColor]: Hello, Arceos!{}", COLOR_START, COLOR_END);
}
