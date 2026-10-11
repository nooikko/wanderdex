//! macOS: pets drawn natively. Each pet is a web-view-free window holding one CALayer that shows the current
//! frame of a sprite sheet. The window server hit-tests transparent windows by alpha, so clicks on a pet's
//! fully transparent pixels reach whatever is underneath (a web view catches every click in its rectangle).
//!
//! Never call `set_ignore_cursor_events` on these windows: setting it at all, even to false, turns that
//! per-pixel hit testing off for the whole window.

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSEvent, NSImage, NSScreen, NSView};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSRect, NSString};
use objc2_quartz_core::{kCAFilterNearest, CALayer, CATransaction};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager, Window};

const DRAG_THRESHOLD_PT: f64 = 3.0;

/// Pointer input from a pet window, applied by the pet loop in arrival order.
pub enum Input {
    Click,
    DragStart((f64, f64)),
    DragMove((f64, f64)),
    Drop,
}

/// A frame to show: `contents` in the sheet's unit coordinates, `dest` in window points (top-left origin).
pub struct Frame {
    pub source: String,
    pub contents: (f64, f64, f64, f64),
    pub dest: (f64, f64, f64, f64),
}

static APP: OnceLock<AppHandle> = OnceLock::new();
static INPUTS: Mutex<Vec<(String, Input)>> = Mutex::new(Vec::new());

pub fn init(app: &AppHandle) {
    let _ = APP.set(app.clone());
}

/// Input received since the last call, oldest first.
pub fn take_inputs() -> Vec<(String, Input)> {
    std::mem::take(&mut *INPUTS.lock().unwrap())
}

fn push(label: &str, input: Input) {
    INPUTS.lock().unwrap().push((label.to_string(), input));
    if let Some(app) = APP.get() {
        crate::pets::wake_loop(app);
    }
}

/// The cursor in desktop space (logical points, top-left of the primary screen).
fn cursor(mtm: MainThreadMarker) -> (f64, f64) {
    let p = NSEvent::mouseLocation();
    let primary_h = NSScreen::screens(mtm).firstObject().map(|s| s.frame().size.height).unwrap_or(0.0);
    (p.x, primary_h - p.y)
}

struct Ivars {
    label: String,
    down: Cell<Option<(f64, f64)>>,
    dragging: Cell<bool>,
}

define_class!(
    // SAFETY: NSView has no subclassing requirements we break; PetView doesn't implement Drop.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[ivars = Ivars]
    struct PetView;

    impl PetView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            self.ivars().down.set(Some(cursor(self.mtm())));
            self.ivars().dragging.set(false);
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, _event: &NSEvent) {
            let Some(origin) = self.ivars().down.get() else { return };
            let c = cursor(self.mtm());
            let label = &self.ivars().label;
            if !self.ivars().dragging.get() {
                if (c.0 - origin.0).hypot(c.1 - origin.1) <= DRAG_THRESHOLD_PT {
                    return;
                }
                self.ivars().dragging.set(true);
                push(label, Input::DragStart(c));
            }
            push(label, Input::DragMove(c));
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, _event: &NSEvent) {
            if self.ivars().down.take().is_none() {
                return;
            }
            let dragged = self.ivars().dragging.replace(false);
            push(&self.ivars().label, if dragged { Input::Drop } else { Input::Click });
        }

        #[unsafe(method(rightMouseDown:))]
        fn right_mouse_down(&self, _event: &NSEvent) {
            let Some(app) = APP.get() else { return };
            let label = &self.ivars().label;
            if let (Some(window), Ok(menu)) = (app.get_window(label), crate::commands::pet_menu(app, label)) {
                let _ = window.popup_menu(&menu);
            }
        }
    }
);

impl PetView {
    fn new(mtm: MainThreadMarker, label: String, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Ivars { label, down: Cell::new(None), dragging: Cell::new(false) });
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

struct Sprite {
    view: Retained<PetView>,
    layer: Retained<CALayer>,
    sheets: HashMap<String, Retained<NSImage>>,
    source: RefCell<String>,
}

thread_local! {
    /// Main thread only: every live pet's view, layer and sheets.
    static SPRITES: RefCell<HashMap<String, Sprite>> = RefCell::new(HashMap::new());
}

/// Gives a pet window its view and loads its sheets. Call on the main thread.
pub fn attach(window: &Window, dir: &Path, sources: &[String]) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let Ok(ptr) = window.ns_view() else { return };
    // SAFETY: tauri returns the window's live content view.
    let content: &NSView = unsafe { &*(ptr as *const NSView) };
    let view = PetView::new(mtm, window.label().to_string(), content.bounds());
    view.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable);
    view.setWantsLayer(true);
    let layer = CALayer::new();
    // SAFETY: a constant string exported by QuartzCore.
    let nearest = unsafe { kCAFilterNearest };
    layer.setMagnificationFilter(nearest);
    layer.setMinificationFilter(nearest);
    if let Some(root) = view.layer() {
        root.addSublayer(&layer);
    }
    content.addSubview(&view);
    let sheets = sources
        .iter()
        .filter_map(|s| {
            let path = dir.join(format!("{s}-Anim.png"));
            let image = NSImage::initWithContentsOfFile(NSImage::alloc(), &NSString::from_str(&path.to_string_lossy()))?;
            Some((s.clone(), image))
        })
        .collect();
    let sprite = Sprite { view, layer, sheets, source: RefCell::new(String::new()) };
    SPRITES.with(|s| s.borrow_mut().insert(window.label().to_string(), sprite));
}

/// Shows a frame in a pet's layer (on the main thread, without implicit animation).
pub fn draw(app: &AppHandle, label: &str, f: Frame) {
    let label = label.to_string();
    let _ = app.run_on_main_thread(move || {
        SPRITES.with(|s| {
            let sprites = s.borrow();
            let Some(sprite) = sprites.get(&label) else { return };
            CATransaction::begin();
            CATransaction::setDisableActions(true);
            if *sprite.source.borrow() != f.source {
                if let Some(image) = sprite.sheets.get(&f.source) {
                    let image: &AnyObject = image;
                    // SAFETY: CALayer accepts an NSImage as contents on macOS.
                    unsafe { sprite.layer.setContents(Some(image)) };
                    *sprite.source.borrow_mut() = f.source.clone();
                }
            }
            let (x, y, w, h) = f.contents;
            sprite.layer.setContentsRect(CGRect::new(CGPoint::new(x, y), CGSize::new(w, h)));
            let (x, y, w, h) = f.dest;
            sprite.layer.setFrame(CGRect::new(CGPoint::new(x, y), CGSize::new(w, h)));
            CATransaction::commit();
        });
    });
}

/// Forgets a pet's view and sheets (before its window is destroyed).
pub fn detach(app: &AppHandle, label: &str) {
    let label = label.to_string();
    let _ = app.run_on_main_thread(move || {
        if let Some(sprite) = SPRITES.with(|s| s.borrow_mut().remove(&label)) {
            sprite.view.removeFromSuperview();
        }
    });
}
