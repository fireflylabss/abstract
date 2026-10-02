//! Right-click menus: `CtxMenuExt::ctx_menu` wraps a trigger element and
//! shows a [`PopupMenu`] anchored at the click. Same plumbing as
//! gpui-component's `ContextMenu` (element state, hitbox, deferred draw,
//! dismiss subscription) — forked because that element gives no hook for an
//! open animation or for replacing a menu opened elsewhere.
//!
//! On top of the crate behavior: a ~120ms fade + settle-in per open, and an
//! `OpenMenu` global so a second right-click dismisses the first menu even if
//! its outside-click handler did not run. Menus still snap inside the window
//! (`snap_to_window_with_margin`) and submenus flip side near edges
//! (`PopupMenu::update_submenu_menu_anchor`).

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::component::menu::PopupMenu;

use super::*;

/// The context menu currently open, so the next right-click replaces it.
struct OpenMenu(WeakEntity<PopupMenu>);
impl Global for OpenMenu {}

type MenuBuilder = dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu;

/// Attach a context menu to an element; same builder contract as
/// `ContextMenuExt::context_menu`.
pub(crate) trait CtxMenuExt: InteractiveElement + ParentElement + Styled {
    /// Add a context menu shown on right-click.
    ///
    /// The element becomes `relative`; the menu is positioned `absolute` and
    /// does not affect the trigger's layout.
    #[track_caller]
    fn ctx_menu(
        mut self,
        f: impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    ) -> CtxMenu<Self>
    where
        Self: Sized,
    {
        // The ID must be stable across renders, otherwise the element state
        // (open menu) is lost on every re-render.
        let caller = std::panic::Location::caller();
        let id = self
            .interactivity()
            .element_id
            .clone()
            .map(|id| ElementId::Name(format!("ctx-menu-{id:?}").into()))
            .unwrap_or_else(|| ElementId::CodeLocation(*caller));
        CtxMenu::new(id, self).menu(f)
    }
}

impl<E: InteractiveElement + ParentElement + Styled> CtxMenuExt for E {}

/// A context menu that can be shown on right-click.
pub(crate) struct CtxMenu<E: ParentElement + Styled + Sized> {
    id: ElementId,
    element: Option<E>,
    menu: Option<Rc<MenuBuilder>>,
    // This is not in use, just for style refinement forwarding.
    _ignore_style: StyleRefinement,
    anchor: Anchor,
}

impl<E: ParentElement + Styled> CtxMenu<E> {
    fn new(id: impl Into<ElementId>, element: E) -> Self {
        Self {
            id: id.into(),
            element: Some(element),
            menu: None,
            anchor: Anchor::TopLeft,
            _ignore_style: StyleRefinement::default(),
        }
    }

    /// Build the context menu using the given builder function.
    #[must_use]
    fn menu<F>(mut self, builder: F) -> Self
    where
        F: Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static,
    {
        self.menu = Some(Rc::new(builder));
        self
    }

    fn with_element_state<R>(
        &mut self,
        id: &GlobalElementId,
        window: &mut Window,
        cx: &mut App,
        f: impl FnOnce(&mut Self, &mut CtxMenuState, &mut Window, &mut App) -> R,
    ) -> R {
        window.with_optional_element_state::<CtxMenuState, _>(Some(id), |element_state, window| {
            let mut element_state = element_state.unwrap().unwrap_or_default();
            let result = f(self, &mut element_state, window, cx);
            (result, Some(element_state))
        })
    }
}

impl<E: ParentElement + Styled> ParentElement for CtxMenu<E> {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        if let Some(element) = &mut self.element {
            element.extend(elements);
        }
    }
}

impl<E: ParentElement + Styled> Styled for CtxMenu<E> {
    fn style(&mut self) -> &mut StyleRefinement {
        if let Some(element) = &mut self.element {
            element.style()
        } else {
            &mut self._ignore_style
        }
    }
}

impl<E: ParentElement + Styled + IntoElement + 'static> IntoElement for CtxMenu<E> {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

struct CtxMenuSharedState {
    menu_view: Option<Entity<PopupMenu>>,
    open: bool,
    position: Point<Pixels>,
    /// Bumped per open; keys the open animation so each menu replays it.
    generation: usize,
    _subscription: Option<Subscription>,
}

pub(crate) struct CtxMenuState {
    element: Option<AnyElement>,
    shared_state: Rc<RefCell<CtxMenuSharedState>>,
}

impl Default for CtxMenuState {
    fn default() -> Self {
        Self {
            element: None,
            shared_state: Rc::new(RefCell::new(CtxMenuSharedState {
                menu_view: None,
                open: false,
                position: Default::default(),
                generation: 0,
                _subscription: None,
            })),
        }
    }
}

impl<E: ParentElement + Styled + IntoElement + 'static> Element for CtxMenu<E> {
    type RequestLayoutState = CtxMenuState;
    type PrepaintState = Hitbox;

    fn id(&self) -> Option<ElementId> {
        Some(self.id.clone())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let anchor = self.anchor;

        self.with_element_state(
            id.unwrap(),
            window,
            cx,
            |this, state: &mut CtxMenuState, window, cx| {
                let (position, open, generation) = {
                    let shared_state = state.shared_state.borrow();
                    (
                        shared_state.position,
                        shared_state.open,
                        shared_state.generation,
                    )
                };
                let menu_view = state.shared_state.borrow().menu_view.clone();
                let mut menu_element = None;
                if open {
                    let has_menu_item = menu_view
                        .as_ref()
                        .map(|menu| !menu.read(cx).is_empty())
                        .unwrap_or(false);

                    if has_menu_item {
                        menu_element = Some(
                            deferred(
                                anchored().child(
                                    div()
                                        .w(window.bounds().size.width)
                                        .h(window.bounds().size.height)
                                        .on_scroll_wheel(|_, _, cx| {
                                            cx.stop_propagation();
                                        })
                                        .child(
                                            anchored()
                                                .position(position)
                                                .snap_to_window_with_margin(px(8.))
                                                .anchor(anchor)
                                                .when_some(menu_view, |this, menu| {
                                                    // Focus the menu, so that can be handle the action.
                                                    if !menu
                                                        .focus_handle(cx)
                                                        .contains_focused(window, cx)
                                                    {
                                                        menu.focus_handle(cx).focus(window, cx);
                                                    }

                                                    this.child(
                                                        div().child(menu.clone()).with_animation(
                                                            ("ctx-menu-in", generation),
                                                            Animation::new(Duration::from_millis(
                                                                120,
                                                            ))
                                                            .with_easing(ease_out_quint),
                                                            |el, d| {
                                                                el.opacity(d).mt(z(-4. * (1. - d)))
                                                            },
                                                        ),
                                                    )
                                                }),
                                        ),
                                ),
                            )
                            .with_priority(gpui_base::POPUP_PRIORITY)
                            .into_any(),
                        );
                    }
                }

                let mut element = this
                    .element
                    .take()
                    .expect("Element should exists.")
                    .children(menu_element)
                    .into_any_element();

                let layout_id = element.request_layout(window, cx);

                (
                    layout_id,
                    CtxMenuState {
                        element: Some(element),
                        ..Default::default()
                    },
                )
            },
        )
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        if let Some(element) = &mut request_layout.element {
            element.prepaint(window, cx);
        }
        window.insert_hitbox(bounds, HitboxBehavior::Normal)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        _: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        hitbox: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(element) = &mut request_layout.element {
            element.paint(window, cx);
        }

        // Take the builder before setting up element state to avoid borrow issues
        let builder = self.menu.clone();

        self.with_element_state(
            id.unwrap(),
            window,
            cx,
            |_view, state: &mut CtxMenuState, window, _| {
                let shared_state = state.shared_state.clone();

                let hitbox = hitbox.clone();
                // When right mouse click, to build content menu, and show it at the mouse position.
                window.on_mouse_event(move |event: &MouseDownEvent, phase, window, cx| {
                    if phase.bubble()
                        && event.button == MouseButton::Right
                        && hitbox.is_hovered(window)
                    {
                        {
                            let mut shared_state = shared_state.borrow_mut();
                            // Clear any existing menu view to allow immediate replacement
                            // Set the new position and open the menu
                            shared_state.menu_view = None;
                            shared_state._subscription = None;
                            shared_state.position = event.position;
                            shared_state.open = true;
                            shared_state.generation += 1;
                        }

                        // Use defer to build the menu in the next frame, avoiding race conditions
                        window.defer(cx, {
                            let shared_state = shared_state.clone();
                            let builder = builder.clone();
                            move |window, cx| {
                                let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
                                    let Some(build) = &builder else {
                                        return menu;
                                    };
                                    build(menu, window, cx)
                                });

                                // One menu at a time: dismiss the menu opened
                                // by another trigger, if any is still around.
                                if let Some(prev) = cx
                                    .try_global::<OpenMenu>()
                                    .and_then(|g| g.0.upgrade())
                                    .filter(|prev| prev.entity_id() != menu.entity_id())
                                {
                                    prev.update(cx, |_, cx| cx.emit(DismissEvent));
                                }
                                cx.set_global(OpenMenu(menu.downgrade()));

                                // Set up the subscription for dismiss handling
                                let _subscription = window.subscribe(&menu, cx, {
                                    let shared_state = shared_state.clone();
                                    move |_, _: &DismissEvent, window, _cx| {
                                        shared_state.borrow_mut().open = false;
                                        window.refresh();
                                    }
                                });

                                // Update the shared state with the built menu and subscription
                                {
                                    let mut state = shared_state.borrow_mut();
                                    state.menu_view = Some(menu.clone());
                                    state._subscription = Some(_subscription);
                                    window.refresh();
                                }
                            }
                        });
                    }
                });
            },
        );
    }
}
