//! Test the output of the `class!` macro.
use objc2::class;
use objc2::runtime::AnyClass;

#[no_mangle]
fn get_class() -> &'static AnyClass {
    class!(NSObject)
}

#[no_mangle]
fn get_same_class() -> &'static AnyClass {
    class!(NSObject)
}

#[no_mangle]
fn get_different_class() -> &'static AnyClass {
    class!(NSString)
}

#[no_mangle]
fn unused_class() {
    let _ = class!(NSData);
}

#[no_mangle]
fn use_fns() -> [&'static AnyClass; 4] {
    let s1 = get_class();
    let s2 = get_same_class();
    let s3 = get_different_class();
    let s4 = class!(NSException);
    [s1, s2, s3, s4]
}

#[no_mangle]
fn use_same_twice() -> [&'static AnyClass; 2] {
    // Should not need to load twice
    [get_class(), get_class()]
}

#[no_mangle]
fn use_in_loop(n: usize) {
    for _i in 0..n {
        // Should be a noop
        let _ = class!(NSLock);
    }
}
