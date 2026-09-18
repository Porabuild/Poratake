use std::path::Path;

use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::{NSArray, NSString, NSURL};

pub fn write_file(path: &Path) -> bool {
    let Some(path) = path.to_str() else {
        return false;
    };
    unsafe {
        let pasteboard: *mut AnyObject = msg_send![class!(NSPasteboard), generalPasteboard];
        if pasteboard.is_null() {
            return false;
        }
        let _: isize = msg_send![pasteboard, clearContents];
        let url = NSURL::fileURLWithPath(&NSString::from_str(path));
        let objects = NSArray::from_retained_slice(&[url]);
        msg_send![pasteboard, writeObjects: &*objects]
    }
}
