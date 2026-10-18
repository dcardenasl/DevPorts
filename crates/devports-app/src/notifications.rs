//! System notifications through `UNUserNotificationCenter`.
//!
//! The notification plugin of Tauri wraps `NSUserNotification`, which Apple
//! deprecated in macOS 11 and may remove. `UNUserNotificationCenter` is the
//! supported API and the only one with real action buttons ("Extender 30 min"),
//! so it is used directly (ADR-025).
//!
//! It needs an application bundle with an identifier; run from a bare binary it
//! raises an Objective-C exception, so [`Notifier::new`] checks for the bundle
//! first and reports [`NotifyError::NoBundle`] instead.
//!
//! This is one of the files allowed to use `unsafe` (ADR-025): implementing the
//! delegate protocol and sharing the retained delegate across threads are
//! unsafe in `objc2` by nature. The unsafe surface is the delegate class and one
//! `Send`/`Sync` wrapper, both justified where they are.

#![allow(
    unsafe_code,
    reason = "Objective-C delegate class and its keep-alive wrapper; see the module docs"
)]

use std::sync::OnceLock;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::{Bool, ProtocolObject};
use objc2::{AnyThread, DefinedClass, define_class, msg_send};
use objc2_foundation::{NSArray, NSBundle, NSError, NSObject, NSObjectProtocol, NSSet, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNMutableNotificationContent, UNNotification, UNNotificationAction,
    UNNotificationActionOptions, UNNotificationCategory, UNNotificationCategoryOptions,
    UNNotificationPresentationOptions, UNNotificationRequest, UNNotificationResponse,
    UNUserNotificationCenter, UNUserNotificationCenterDelegate,
};

/// Category of the "a temporary instance is about to expire" notification.
pub const LEASE_EXPIRING_CATEGORY: &str = "lease-expiring";
/// Identifier of its "Extender 30 min" button.
pub const EXTEND_ACTION: &str = "extend";

const DEFAULT_ACTION_ID: &str = "com.apple.UNNotificationDefaultActionIdentifier";
const DISMISS_ACTION_ID: &str = "com.apple.UNNotificationDismissActionIdentifier";

/// What the user did with a notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Response {
    /// Clicked the notification itself.
    Opened,
    /// Pressed "Extender 30 min".
    Extend,
    /// Dismissed it.
    Dismissed,
    /// Pressed an action this build does not know.
    Other(String),
}

/// Maps the identifier of the action the system reports to a [`Response`].
#[must_use]
pub fn parse_response(action_id: &str) -> Response {
    match action_id {
        DEFAULT_ACTION_ID => Response::Opened,
        DISMISS_ACTION_ID => Response::Dismissed,
        EXTEND_ACTION => Response::Extend,
        other => Response::Other(other.to_owned()),
    }
}

/// Why notifications are not available.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NotifyError {
    /// The process is not an application bundle with an identifier, which
    /// `UNUserNotificationCenter` requires. Happens when running the bare binary.
    #[error("notifications need the application bundle (run DevPorts.app, not the bare binary)")]
    NoBundle,
}

type ResponseHandler = Box<dyn Fn(&str, Response) + Send + Sync>;

struct DelegateState {
    on_response: ResponseHandler,
}

define_class!(
    // The center calls the delegate on its own queue, so it is not main-thread only.
    #[unsafe(super(NSObject))]
    #[name = "DevPortsNotificationDelegate"]
    #[ivars = DelegateState]
    struct Delegate;

    unsafe impl NSObjectProtocol for Delegate {}

    unsafe impl UNUserNotificationCenterDelegate for Delegate {
        #[unsafe(method(userNotificationCenter:willPresentNotification:withCompletionHandler:))]
        fn will_present(
            &self,
            _center: &UNUserNotificationCenter,
            _notification: &UNNotification,
            completion_handler: &block2::DynBlock<dyn Fn(UNNotificationPresentationOptions)>,
        ) {
            // A menu-bar app is never "in the foreground", but say so explicitly: a
            // notification must always show as a banner.
            completion_handler.call((UNNotificationPresentationOptions::Banner
                | UNNotificationPresentationOptions::List
                | UNNotificationPresentationOptions::Sound,));
        }

        #[unsafe(method(userNotificationCenter:didReceiveNotificationResponse:withCompletionHandler:))]
        fn did_receive(
            &self,
            _center: &UNUserNotificationCenter,
            response: &UNNotificationResponse,
            completion_handler: &block2::DynBlock<dyn Fn()>,
        ) {
            let action = response.actionIdentifier().to_string();
            let notification = response.notification().request().identifier().to_string();
            (self.ivars().on_response)(&notification, parse_response(&action));
            completion_handler.call(());
        }
    }
);

impl Delegate {
    fn new(on_response: ResponseHandler) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateState { on_response });
        // SAFETY: `NSObject`'s `init` is always valid on a freshly allocated object.
        unsafe { msg_send![super(this), init] }
    }
}

/// Keeps the delegate alive for the life of the process. The center holds its
/// delegate weakly, so something must own it.
struct KeepAlive(
    #[allow(dead_code, reason = "held only to keep the object alive")] Retained<Delegate>,
);

// SAFETY: the wrapped delegate is never messaged from Rust after it is handed to
// the center; the center calls it from its own queues, and its only state is a
// `Send + Sync` closure. Moving the owning reference between threads is sound.
unsafe impl Send for KeepAlive {}
// SAFETY: as above; there is no shared access from Rust at all.
unsafe impl Sync for KeepAlive {}

static DELEGATE: OnceLock<KeepAlive> = OnceLock::new();

/// Registers the "lease expiring" category with its one button. Registering it again
/// replaces it, which is how the button changes language.
fn register_category(center: &UNUserNotificationCenter, extend_title: &str) {
    let extend = UNNotificationAction::actionWithIdentifier_title_options(
        &NSString::from_str(EXTEND_ACTION),
        &NSString::from_str(extend_title),
        UNNotificationActionOptions::empty(),
    );
    let category = UNNotificationCategory::categoryWithIdentifier_actions_intentIdentifiers_options(
        &NSString::from_str(LEASE_EXPIRING_CATEGORY),
        &NSArray::from_slice(&[&*extend]),
        &NSArray::new(),
        UNNotificationCategoryOptions::empty(),
    );
    center.setNotificationCategories(&NSSet::from_slice(&[&*category]));
}

/// Gives the "lease expiring" button a new title. Does nothing before notifications are
/// set up (a bare binary, or before [`Notifier::new`]), so it is safe to call whenever the
/// language changes.
pub fn relocalize(extend_title: &str) {
    if DELEGATE.get().is_some() {
        register_category(
            &UNUserNotificationCenter::currentNotificationCenter(),
            extend_title,
        );
    }
}

/// Posts notifications and reports what the user does with them.
#[derive(Debug, Clone, Copy)]
pub struct Notifier;

#[allow(
    clippy::unused_self,
    reason = "a `Notifier` is the proof that the delegate and category are registered; posting without one is impossible"
)]
impl Notifier {
    /// Registers the delegate and the "lease expiring" category, whose button reads
    /// `extend_title`.
    ///
    /// `on_response` receives the identifier of the notification the user acted on
    /// and what they did. It runs on a system queue: keep it short.
    ///
    /// # Errors
    ///
    /// [`NotifyError::NoBundle`] when the process is not an application bundle.
    pub fn new(
        extend_title: &str,
        on_response: impl Fn(&str, Response) + Send + Sync + 'static,
    ) -> Result<Self, NotifyError> {
        if NSBundle::mainBundle().bundleIdentifier().is_none() {
            return Err(NotifyError::NoBundle);
        }
        let center = UNUserNotificationCenter::currentNotificationCenter();

        register_category(&center, extend_title);

        let delegate = DELEGATE
            .get_or_init(|| KeepAlive(Delegate::new(Box::new(on_response))))
            .0
            .clone();
        center.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        Ok(Self)
    }

    /// Asks the user for permission to show notifications. The first time, macOS
    /// shows its own prompt; `done` receives the answer.
    pub fn request_authorization(self, done: impl Fn(bool) + 'static) {
        let center = UNUserNotificationCenter::currentNotificationCenter();
        let block = RcBlock::new(move |granted: Bool, _error: *mut NSError| {
            done(granted.as_bool());
        });
        center.requestAuthorizationWithOptions_completionHandler(
            UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound,
            &block,
        );
    }

    /// Posts a notification without buttons; clicking it opens the panel.
    pub fn post_plain(
        self,
        id: &str,
        title: &str,
        body: &str,
        done: impl Fn(Option<String>) + 'static,
    ) {
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(id),
            &content,
            None,
        );
        let block = RcBlock::new(move |error: *mut NSError| {
            // SAFETY: the system passes either null or a valid `NSError` that lives
            // for the duration of this callback.
            let message =
                unsafe { error.as_ref() }.map(|error| error.localizedDescription().to_string());
            done(message);
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .addNotificationRequest_withCompletionHandler(&request, Some(&block));
    }

    /// Posts a notification with the "Extender 30 min" button. `id` identifies it
    /// in [`Notifier::new`]'s callback; posting the same `id` again replaces it.
    pub fn post_lease_expiring(
        self,
        id: &str,
        title: &str,
        body: &str,
        done: impl Fn(Option<String>) + 'static,
    ) {
        let content = UNMutableNotificationContent::new();
        content.setTitle(&NSString::from_str(title));
        content.setBody(&NSString::from_str(body));
        content.setCategoryIdentifier(&NSString::from_str(LEASE_EXPIRING_CATEGORY));
        let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
            &NSString::from_str(id),
            &content,
            None,
        );
        let block = RcBlock::new(move |error: *mut NSError| {
            // SAFETY: the system passes either null or a valid `NSError` that lives
            // for the duration of this callback.
            let message =
                unsafe { error.as_ref() }.map(|error| error.localizedDescription().to_string());
            done(message);
        });
        UNUserNotificationCenter::currentNotificationCenter()
            .addNotificationRequest_withCompletionHandler(&request, Some(&block));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_systems_action_identifiers_map_to_responses() {
        assert_eq!(
            parse_response("com.apple.UNNotificationDefaultActionIdentifier"),
            Response::Opened
        );
        assert_eq!(
            parse_response("com.apple.UNNotificationDismissActionIdentifier"),
            Response::Dismissed
        );
        assert_eq!(parse_response("extend"), Response::Extend);
        assert_eq!(
            parse_response("snooze"),
            Response::Other("snooze".to_owned())
        );
    }

    #[test]
    fn a_bare_binary_is_refused_instead_of_crashing() {
        // `cargo test` runs a plain executable, not an application bundle.
        assert_eq!(
            Notifier::new("Extend", |_, _| {}).unwrap_err(),
            NotifyError::NoBundle
        );
    }
}
