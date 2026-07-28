//! macOS SystemConfiguration and CFNetwork proxy/PAC resolution.

use super::ProxyRoute;
use std::ffi::c_void;
use std::ptr;
use std::time::{Duration, Instant};
use system_configuration::core_foundation::array::{CFArray, CFArrayRef};
use system_configuration::core_foundation::base::{
    kCFAllocatorDefault, CFEqual, CFGetTypeID, CFIndex, CFType, CFTypeRef, TCFType,
};
use system_configuration::core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
use system_configuration::core_foundation::error::CFErrorRef;
use system_configuration::core_foundation::number::CFNumber;
use system_configuration::core_foundation::runloop::{
    kCFRunLoopDefaultMode, CFRunLoop, CFRunLoopSource, CFRunLoopSourceInvalidate,
    CFRunLoopSourceRef,
};
use system_configuration::core_foundation::string::{CFString, CFStringRef};
use system_configuration::core_foundation::url::{
    CFURLCreateWithString, CFURLGetTypeID, CFURLRef, CFURL,
};
use system_configuration::dynamic_store::SCDynamicStoreBuilder;

const PAC_TIMEOUT: Duration = Duration::from_secs(5);
type ProxyDictionary = CFDictionary<CFString, CFType>;
type ProxyArray = CFArray<ProxyDictionary>;

#[repr(C)]
struct CFStreamClientContext {
    version: CFIndex,
    info: *mut c_void,
    retain: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
    release: Option<unsafe extern "C" fn(*mut c_void)>,
    copy_description: Option<unsafe extern "C" fn(*mut c_void) -> CFStringRef>,
}

type PacCallback = unsafe extern "C" fn(*mut c_void, CFArrayRef, CFErrorRef);

#[link(name = "CFNetwork", kind = "framework")]
unsafe extern "C" {
    static kCFProxyTypeKey: CFStringRef;
    static kCFProxyHostNameKey: CFStringRef;
    static kCFProxyPortNumberKey: CFStringRef;
    static kCFProxyAutoConfigurationURLKey: CFStringRef;
    static kCFProxyAutoConfigurationJavaScriptKey: CFStringRef;
    static kCFProxyTypeNone: CFStringRef;
    static kCFProxyTypeHTTP: CFStringRef;
    static kCFProxyTypeHTTPS: CFStringRef;
    static kCFProxyTypeSOCKS: CFStringRef;
    static kCFProxyTypeAutoConfigurationURL: CFStringRef;
    static kCFProxyTypeAutoConfigurationJavaScript: CFStringRef;

    fn CFNetworkCopyProxiesForURL(url: CFURLRef, settings: CFDictionaryRef) -> CFArrayRef;
    fn CFNetworkExecuteProxyAutoConfigurationURL(
        pac_url: CFURLRef,
        target_url: CFURLRef,
        callback: PacCallback,
        context: *mut CFStreamClientContext,
    ) -> CFRunLoopSourceRef;
    fn CFNetworkExecuteProxyAutoConfigurationScript(
        script: CFStringRef,
        target_url: CFURLRef,
        callback: PacCallback,
        context: *mut CFStreamClientContext,
    ) -> CFRunLoopSourceRef;
}

pub(super) fn resolve(request_url: &str) -> Option<ProxyRoute> {
    let target = cf_url(request_url)?;
    let settings = SCDynamicStoreBuilder::new("A3S Code")
        .build()?
        .get_proxies()?;
    let proxies = copy_proxies(&target, &settings)?;
    array_route(&proxies, &target)
}

fn copy_proxies(target: &CFURL, settings: &CFDictionary<CFString, CFType>) -> Option<ProxyArray> {
    let proxies = unsafe {
        CFNetworkCopyProxiesForURL(target.as_concrete_TypeRef(), settings.as_concrete_TypeRef())
    };
    (!proxies.is_null()).then(|| unsafe { ProxyArray::wrap_under_create_rule(proxies) })
}

fn array_route(proxies: &ProxyArray, target: &CFURL) -> Option<ProxyRoute> {
    for proxy in proxies {
        if let Some(route) = entry_route(&proxy, target) {
            return Some(route);
        }
    }
    Some(ProxyRoute::Direct)
}

fn entry_route(proxy: &ProxyDictionary, target: &CFURL) -> Option<ProxyRoute> {
    let proxy_type = cf_string(proxy, unsafe { kCFProxyTypeKey })?;
    if equals(&proxy_type, unsafe { kCFProxyTypeNone }) {
        return Some(ProxyRoute::Direct);
    }
    if equals(&proxy_type, unsafe { kCFProxyTypeHTTP })
        || equals(&proxy_type, unsafe { kCFProxyTypeHTTPS })
    {
        return concrete_proxy(proxy, "http", false);
    }
    if equals(&proxy_type, unsafe { kCFProxyTypeSOCKS }) {
        return concrete_proxy(proxy, "socks5", true);
    }
    if equals(&proxy_type, unsafe { kCFProxyTypeAutoConfigurationURL }) {
        let pac_url = cf_url_value(proxy, unsafe { kCFProxyAutoConfigurationURLKey })?;
        return execute_pac(|callback, context| unsafe {
            CFNetworkExecuteProxyAutoConfigurationURL(
                pac_url.as_concrete_TypeRef(),
                target.as_concrete_TypeRef(),
                callback,
                context,
            )
        })
        .and_then(|proxies| array_route(&proxies, target));
    }
    if equals(&proxy_type, unsafe {
        kCFProxyTypeAutoConfigurationJavaScript
    }) {
        let script = cf_string(proxy, unsafe { kCFProxyAutoConfigurationJavaScriptKey })?;
        return execute_pac(|callback, context| unsafe {
            CFNetworkExecuteProxyAutoConfigurationScript(
                script.as_concrete_TypeRef(),
                target.as_concrete_TypeRef(),
                callback,
                context,
            )
        })
        .and_then(|proxies| array_route(&proxies, target));
    }
    None
}

fn concrete_proxy(proxy: &ProxyDictionary, scheme: &str, socks: bool) -> Option<ProxyRoute> {
    let host = cf_string(proxy, unsafe { kCFProxyHostNameKey })?.to_string();
    if host.is_empty() {
        return None;
    }
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host
    };
    let port = cf_i32(proxy, unsafe { kCFProxyPortNumberKey });
    let value = port.filter(|port| *port > 0).map_or_else(
        || format!("{scheme}://{host}"),
        |port| format!("{scheme}://{host}:{port}"),
    );
    reqwest::Url::parse(&value).ok().map(|url| {
        if socks {
            ProxyRoute::Socks(url)
        } else {
            ProxyRoute::Http(url)
        }
    })
}

fn execute_pac(
    create: impl FnOnce(PacCallback, *mut CFStreamClientContext) -> CFRunLoopSourceRef,
) -> Option<ProxyArray> {
    let mut state = PacState { result: None };
    let mut context = CFStreamClientContext {
        version: 0,
        info: (&mut state as *mut PacState).cast(),
        retain: None,
        release: None,
        copy_description: None,
    };
    let source = create(pac_callback, &mut context);
    if source.is_null() {
        return None;
    }
    let source = unsafe { CFRunLoopSource::wrap_under_create_rule(source) };
    let run_loop = CFRunLoop::get_current();
    let mode = unsafe { kCFRunLoopDefaultMode };
    run_loop.add_source(&source, mode);
    let started = Instant::now();
    while state.result.is_none() && started.elapsed() < PAC_TIMEOUT {
        CFRunLoop::run_in_mode(mode, Duration::from_millis(50), true);
    }
    if state.result.is_none() {
        unsafe { CFRunLoopSourceInvalidate(source.as_concrete_TypeRef()) };
    }
    run_loop.remove_source(&source, mode);
    state.result.flatten()
}

unsafe extern "C" fn pac_callback(client: *mut c_void, proxies: CFArrayRef, error: CFErrorRef) {
    let state = unsafe { &mut *client.cast::<PacState>() };
    state.result = if error.is_null() && !proxies.is_null() {
        Some(Some(unsafe { ProxyArray::wrap_under_get_rule(proxies) }))
    } else {
        Some(None)
    };
    CFRunLoop::get_current().stop();
}

struct PacState {
    result: Option<Option<ProxyArray>>,
}

fn cf_string(proxy: &ProxyDictionary, key: CFStringRef) -> Option<CFString> {
    proxy
        .find(key)
        .and_then(|value| value.downcast::<CFString>())
}

fn cf_i32(proxy: &ProxyDictionary, key: CFStringRef) -> Option<i32> {
    proxy
        .find(key)
        .and_then(|value| value.downcast::<CFNumber>())
        .and_then(|value| value.to_i32())
}

fn cf_url_value(proxy: &ProxyDictionary, key: CFStringRef) -> Option<CFURL> {
    proxy.find(key).and_then(|value| {
        if unsafe { CFGetTypeID(value.as_CFTypeRef()) == CFURLGetTypeID() } {
            Some(unsafe { CFURL::wrap_under_get_rule(value.as_CFTypeRef() as CFURLRef) })
        } else {
            value
                .downcast::<CFString>()
                .and_then(|value| cf_url(&value.to_string()))
        }
    })
}

fn equals(value: &CFString, expected: CFStringRef) -> bool {
    unsafe { CFEqual(value.as_CFTypeRef(), expected as CFTypeRef) != 0 }
}

fn cf_url(value: &str) -> Option<CFURL> {
    let value = CFString::new(value);
    let url = unsafe {
        CFURLCreateWithString(
            kCFAllocatorDefault,
            value.as_concrete_TypeRef(),
            ptr::null(),
        )
    };
    (!url.is_null()).then(|| unsafe { CFURL::wrap_under_create_rule(url) })
}
