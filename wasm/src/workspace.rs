use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element, EventTarget, HtmlDivElement, PointerEvent};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SplitDirection {
    Vertical,
    Horizontal,
}

impl SplitDirection {
    fn parse(direction: &str) -> Option<Self> {
        match direction {
            "vertical" => Some(Self::Vertical),
            "horizontal" => Some(Self::Horizontal),
            _ => None,
        }
    }
}

#[derive(Debug, Clone)]
enum WorkspaceNodeKind {
    Leaf {
        pane_id: u32,
    },
    Split {
        direction: SplitDirection,
        ratio: f64,
        first: u32,
        second: u32,
    },
}

#[derive(Debug, Clone)]
struct WorkspaceNode {
    parent: Option<u32>,
    kind: WorkspaceNodeKind,
}

struct PaneMeta {
    host_id: String,
    host: HtmlDivElement,
}

struct SplitView {
    direction: SplitDirection,
    wrapper: HtmlDivElement,
    first_slot: HtmlDivElement,
    second_slot: HtmlDivElement,
    divider: HtmlDivElement,
    hit_target: HtmlDivElement,
}

#[derive(Debug, Clone)]
struct WorkspaceDividerStyle {
    thickness_css: f64,
    hit_area_css: f64,
    color: [f32; 4],
    active_color: [f32; 4],
}

impl Default for WorkspaceDividerStyle {
    fn default() -> Self {
        let theme = axiuscharts::ThemeConfig::default();
        Self {
            thickness_css: 1.0,
            hit_area_css: 9.0,
            color: theme.workspace.divider_color,
            active_color: theme.workspace.divider_active_color,
        }
    }
}

impl WorkspaceDividerStyle {
    fn normalize(&mut self) {
        self.thickness_css = self.thickness_css.clamp(1.0, 24.0);
        self.hit_area_css = self.hit_area_css.clamp(self.thickness_css, 48.0);
    }
}

#[derive(Debug, Clone)]
struct WorkspacePaneStyle {
    background_color: [f32; 4],
    active_border_color: [f32; 4],
    active_border_width_css: f64,
}

impl Default for WorkspacePaneStyle {
    fn default() -> Self {
        let theme = axiuscharts::ThemeConfig::default();
        Self {
            background_color: theme.workspace.pane_background,
            active_border_color: theme.workspace.pane_active_border,
            active_border_width_css: 1.0,
        }
    }
}

impl WorkspacePaneStyle {
    fn normalize(&mut self) {
        self.active_border_width_css = self.active_border_width_css.clamp(0.0, 8.0);
    }
}

#[derive(Debug, Clone, Default)]
struct WorkspaceStyleConfig {
    divider: WorkspaceDividerStyle,
    pane: WorkspacePaneStyle,
}

fn rgba_css(color: [f32; 4]) -> String {
    crate::utils::rgba_css(&color)
}

struct WorkspaceInner {
    container: HtmlDivElement,
    root_node_id: u32,
    next_node_id: u32,
    next_pane_id: u32,
    active_pane_id: u32,
    nodes: HashMap<u32, WorkspaceNode>,
    panes: HashMap<u32, PaneMeta>,
    pane_to_node: HashMap<u32, u32>,
    split_views: HashMap<u32, SplitView>,
    drag_split_id: Option<u32>,
    drag_pointer_id: Option<i32>,
    fullscreen_pane_id: Option<u32>,
    style: WorkspaceStyleConfig,
}

impl WorkspaceInner {
    fn document() -> Result<Document, JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
        window
            .document()
            .ok_or_else(|| JsValue::from_str("document unavailable"))
    }

    fn new(container: HtmlDivElement) -> Result<Self, JsValue> {
        let mut nodes = HashMap::new();
        let mut panes = HashMap::new();
        let mut pane_to_node = HashMap::new();

        let first_pane_id = 1u32;
        let root_node_id = 1u32;
        let host = Self::create_host(first_pane_id)?;
        let host_id = host.id();

        nodes.insert(
            root_node_id,
            WorkspaceNode {
                parent: None,
                kind: WorkspaceNodeKind::Leaf {
                    pane_id: first_pane_id,
                },
            },
        );
        panes.insert(first_pane_id, PaneMeta { host_id, host });
        pane_to_node.insert(first_pane_id, root_node_id);

        let mut this = Self {
            container,
            root_node_id,
            next_node_id: 2,
            next_pane_id: 2,
            active_pane_id: first_pane_id,
            nodes,
            panes,
            pane_to_node,
            split_views: HashMap::new(),
            drag_split_id: None,
            drag_pointer_id: None,
            fullscreen_pane_id: None,
            style: WorkspaceStyleConfig::default(),
        };
        this.style.divider.normalize();
        this.style.pane.normalize();
        this.rebuild_dom()?;
        Ok(this)
    }

    fn create_host(pane_id: u32) -> Result<HtmlDivElement, JsValue> {
        let document = Self::document()?;
        let host = document
            .create_element("div")?
            .dyn_into::<HtmlDivElement>()?;
        let host_id = format!("axiuscharts-workspace-pane-{pane_id}");
        host.set_id(&host_id);
        host.set_class_name("chart-host");
        host.set_attribute("data-pane-id", &pane_id.to_string())?;
        host.style().set_css_text(
            "width:100%;height:100%;min-width:0;min-height:0;position:relative;overflow:hidden;background:transparent;",
        );
        Ok(host)
    }

    fn next_node(&mut self) -> u32 {
        let id = self.next_node_id;
        self.next_node_id = self.next_node_id.saturating_add(1);
        id
    }

    fn next_pane(&mut self) -> u32 {
        let id = self.next_pane_id;
        self.next_pane_id = self.next_pane_id.saturating_add(1);
        id
    }

    fn active_pane_id(&self) -> u32 {
        self.active_pane_id
    }

    fn set_active_pane(&mut self, pane_id: u32) -> bool {
        if !self.panes.contains_key(&pane_id) {
            return false;
        }
        self.active_pane_id = pane_id;
        self.apply_active_styles();
        true
    }

    fn pane_host_id(&self, pane_id: u32) -> Option<String> {
        self.panes.get(&pane_id).map(|p| p.host_id.clone())
    }

    fn pane_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.panes.keys().copied().collect();
        ids.sort_unstable();
        ids
    }

    fn fullscreen_pane_id(&self) -> Option<u32> {
        self.fullscreen_pane_id
            .filter(|pane_id| self.panes.contains_key(pane_id))
    }

    fn is_fullscreen(&self) -> bool {
        self.fullscreen_pane_id().is_some()
    }

    fn toggle_fullscreen_pane(&mut self, pane_id: u32) -> bool {
        if !self.panes.contains_key(&pane_id) {
            return false;
        }

        self.active_pane_id = pane_id;
        self.fullscreen_pane_id = if self.fullscreen_pane_id == Some(pane_id) {
            None
        } else {
            Some(pane_id)
        };

        self.rebuild_dom().is_ok()
    }

    fn clear_fullscreen(&mut self) -> bool {
        if self.fullscreen_pane_id.is_none() {
            return false;
        }
        self.fullscreen_pane_id = None;
        self.rebuild_dom().is_ok()
    }

    fn split_active(&mut self, direction: SplitDirection) -> Result<u32, JsValue> {
        let active = self.active_pane_id;
        self.split_pane(active, direction)
    }

    fn split_pane(&mut self, pane_id: u32, direction: SplitDirection) -> Result<u32, JsValue> {
        let target_node_id = self
            .pane_to_node
            .get(&pane_id)
            .copied()
            .ok_or_else(|| JsValue::from_str("pane not found"))?;

        let target_parent = self
            .nodes
            .get(&target_node_id)
            .map(|n| n.parent)
            .ok_or_else(|| JsValue::from_str("target node missing"))?;

        let is_leaf = matches!(
            self.nodes.get(&target_node_id).map(|n| &n.kind),
            Some(WorkspaceNodeKind::Leaf { .. })
        );
        if !is_leaf {
            return Err(JsValue::from_str("pane is not a leaf node"));
        }

        let new_pane_id = self.next_pane();
        let new_host = Self::create_host(new_pane_id)?;
        let new_host_id = new_host.id();
        self.panes.insert(
            new_pane_id,
            PaneMeta {
                host_id: new_host_id,
                host: new_host,
            },
        );

        let first_leaf_id = self.next_node();
        let second_leaf_id = self.next_node();

        self.nodes.insert(
            first_leaf_id,
            WorkspaceNode {
                parent: Some(target_node_id),
                kind: WorkspaceNodeKind::Leaf { pane_id },
            },
        );
        self.nodes.insert(
            second_leaf_id,
            WorkspaceNode {
                parent: Some(target_node_id),
                kind: WorkspaceNodeKind::Leaf {
                    pane_id: new_pane_id,
                },
            },
        );

        self.nodes.insert(
            target_node_id,
            WorkspaceNode {
                parent: target_parent,
                kind: WorkspaceNodeKind::Split {
                    direction,
                    ratio: 0.5,
                    first: first_leaf_id,
                    second: second_leaf_id,
                },
            },
        );

        self.pane_to_node.insert(pane_id, first_leaf_id);
        self.pane_to_node.insert(new_pane_id, second_leaf_id);
        self.active_pane_id = new_pane_id;
        self.fullscreen_pane_id = None;

        self.rebuild_dom()?;
        Ok(new_pane_id)
    }

    fn rebuild_dom(&mut self) -> Result<(), JsValue> {
        while let Some(child) = self.container.first_child() {
            self.container.remove_child(&child)?;
        }
        self.split_views.clear();

        if let Some(fullscreen_pane_id) = self.fullscreen_pane_id() {
            if let Some(pane) = self.panes.get(&fullscreen_pane_id) {
                self.container.append_child(&pane.host)?;
            } else {
                self.fullscreen_pane_id = None;
                let root = self.build_node_dom(self.root_node_id)?;
                self.container.append_child(&root)?;
            }
        } else {
            let root = self.build_node_dom(self.root_node_id)?;
            self.container.append_child(&root)?;
        }
        self.apply_active_styles();
        self.apply_divider_styles();
        Ok(())
    }

    fn build_node_dom(&mut self, node_id: u32) -> Result<HtmlDivElement, JsValue> {
        let node_kind = self
            .nodes
            .get(&node_id)
            .map(|n| n.kind.clone())
            .ok_or_else(|| JsValue::from_str("node not found"))?;

        match node_kind {
            WorkspaceNodeKind::Leaf { pane_id } => {
                let pane = self
                    .panes
                    .get(&pane_id)
                    .ok_or_else(|| JsValue::from_str("pane metadata missing"))?;
                Ok(pane.host.clone())
            }
            WorkspaceNodeKind::Split {
                direction,
                ratio,
                first,
                second,
            } => {
                let document = Self::document()?;
                let wrapper = document
                    .create_element("div")?
                    .dyn_into::<HtmlDivElement>()?;
                wrapper.set_class_name("chart-split-wrapper");
                wrapper.style().set_css_text(&format!(
                    "display:flex;flex-direction:{};width:100%;height:100%;min-width:0;min-height:0;overflow:hidden;background:transparent;",
                    if direction == SplitDirection::Vertical {
                        "row"
                    } else {
                        "column"
                    }
                ));

                let first_slot = document
                    .create_element("div")?
                    .dyn_into::<HtmlDivElement>()?;
                first_slot
                    .style()
                    .set_css_text("min-width:0;min-height:0;overflow:hidden;position:relative;");

                let second_slot = document
                    .create_element("div")?
                    .dyn_into::<HtmlDivElement>()?;
                second_slot
                    .style()
                    .set_css_text("min-width:0;min-height:0;overflow:hidden;position:relative;");

                let divider = document
                    .create_element("div")?
                    .dyn_into::<HtmlDivElement>()?;
                divider.set_class_name(if direction == SplitDirection::Vertical {
                    "chart-divider vertical"
                } else {
                    "chart-divider horizontal"
                });
                divider.set_attribute("data-split-id", &node_id.to_string())?;

                let hit_target = document
                    .create_element("div")?
                    .dyn_into::<HtmlDivElement>()?;
                hit_target.set_class_name("chart-divider-hit");
                hit_target.set_attribute("data-split-id", &node_id.to_string())?;
                hit_target.style().set_css_text(
                    "position:absolute;top:0;left:0;right:0;bottom:0;background:transparent;touch-action:none;user-select:none;",
                );
                divider.append_child(&hit_target)?;

                let first_dom = self.build_node_dom(first)?;
                let second_dom = self.build_node_dom(second)?;
                first_slot.append_child(&first_dom)?;
                second_slot.append_child(&second_dom)?;

                wrapper.append_child(&first_slot)?;
                wrapper.append_child(&divider)?;
                wrapper.append_child(&second_slot)?;

                self.split_views.insert(
                    node_id,
                    SplitView {
                        direction,
                        wrapper: wrapper.clone(),
                        first_slot: first_slot.clone(),
                        second_slot: second_slot.clone(),
                        divider: divider.clone(),
                        hit_target: hit_target.clone(),
                    },
                );

                self.apply_split_ratio(node_id, ratio);
                self.apply_divider_style(node_id);
                Ok(wrapper)
            }
        }
    }

    fn apply_active_styles(&self) {
        let bg = rgba_css(self.style.pane.background_color);
        let border = rgba_css(self.style.pane.active_border_color);
        let border_width = self.style.pane.active_border_width_css.max(0.0);
        for (pane_id, pane) in &self.panes {
            let _ = pane.host.style().set_property("background", &bg);
            if *pane_id == self.active_pane_id && border_width > 0.0 {
                let _ = pane.host.style().set_property(
                    "box-shadow",
                    &format!("inset 0 0 0 {border_width:.3}px {border}"),
                );
            } else {
                let _ = pane.host.style().set_property("box-shadow", "none");
            }
        }
    }

    fn apply_split_ratio(&self, split_id: u32, ratio: f64) {
        if let Some(view) = self.split_views.get(&split_id) {
            let pct = (ratio.clamp(0.15, 0.85) * 100.0).clamp(15.0, 85.0);
            let first_flex = format!("0 0 {pct}%");
            let second_flex = format!("0 0 {}%", 100.0 - pct);
            let _ = view.first_slot.style().set_property("flex", &first_flex);
            let _ = view.second_slot.style().set_property("flex", &second_flex);
            if view.direction == SplitDirection::Vertical {
                let _ = view.wrapper.style().set_property("flex-direction", "row");
            } else {
                let _ = view
                    .wrapper
                    .style()
                    .set_property("flex-direction", "column");
            }
        }
    }

    fn apply_divider_style(&self, split_id: u32) {
        let Some(view) = self.split_views.get(&split_id) else {
            return;
        };

        let mut style = self.style.divider.clone();
        style.normalize();
        let line_color = if self.drag_split_id == Some(split_id) {
            rgba_css(style.active_color)
        } else {
            rgba_css(style.color)
        };
        let thickness = style.thickness_css;
        let hit_area = style.hit_area_css;

        if view.direction == SplitDirection::Vertical {
            let _ = view.divider.style().set_css_text(&format!(
                "position:relative;overflow:visible;touch-action:none;user-select:none;\
                 flex:0 0 {thickness:.3}px;width:{thickness:.3}px;height:100%;\
                 background:{line_color};z-index:5;"
            ));
            let left = -((hit_area - thickness) * 0.5);
            let _ = view.hit_target.style().set_css_text(&format!(
                "position:absolute;top:0;bottom:0;left:{left:.3}px;width:{hit_area:.3}px;\
                 cursor:col-resize;touch-action:none;user-select:none;background:transparent;z-index:6;"
            ));
        } else {
            let _ = view.divider.style().set_css_text(&format!(
                "position:relative;overflow:visible;touch-action:none;user-select:none;\
                 flex:0 0 {thickness:.3}px;height:{thickness:.3}px;width:100%;\
                 background:{line_color};z-index:5;"
            ));
            let top = -((hit_area - thickness) * 0.5);
            let _ = view.hit_target.style().set_css_text(&format!(
                "position:absolute;left:0;right:0;top:{top:.3}px;height:{hit_area:.3}px;\
                 cursor:row-resize;touch-action:none;user-select:none;background:transparent;z-index:6;"
            ));
        }
    }

    fn apply_divider_styles(&self) {
        let split_ids: Vec<u32> = self.split_views.keys().copied().collect();
        for split_id in split_ids {
            self.apply_divider_style(split_id);
        }
    }

    fn set_split_divider_thickness(&mut self, thickness_css: f64) {
        self.style.divider.thickness_css = thickness_css;
        self.style.divider.normalize();
        self.apply_divider_styles();
    }

    fn set_split_divider_hit_area(&mut self, hit_area_css: f64) {
        self.style.divider.hit_area_css = hit_area_css;
        self.style.divider.normalize();
        self.apply_divider_styles();
    }

    fn set_split_divider_color(&mut self, color: [f32; 4]) {
        self.style.divider.color = color;
        self.apply_divider_styles();
    }

    fn set_split_divider_active_color(&mut self, color: [f32; 4]) {
        self.style.divider.active_color = color;
        self.apply_divider_styles();
    }

    fn set_pane_background_color(&mut self, color: [f32; 4]) {
        self.style.pane.background_color = color;
        self.apply_active_styles();
    }

    fn set_active_pane_border_color(&mut self, color: [f32; 4]) {
        self.style.pane.active_border_color = color;
        self.apply_active_styles();
    }

    fn set_active_pane_border_width(&mut self, width_css: f64) {
        self.style.pane.active_border_width_css = width_css;
        self.style.pane.normalize();
        self.apply_active_styles();
    }

    fn find_attr_u32(target: Option<EventTarget>, attr: &str) -> Option<u32> {
        let mut current = target.and_then(|t| t.dyn_into::<Element>().ok());
        while let Some(el) = current {
            if let Some(value) = el.get_attribute(attr) {
                if let Ok(parsed) = value.parse::<u32>() {
                    return Some(parsed);
                }
            }
            current = el.parent_element();
        }
        None
    }

    fn on_pointer_down(&mut self, event: PointerEvent) {
        if let Some(split_id) = Self::find_attr_u32(event.target(), "data-split-id") {
            self.drag_split_id = Some(split_id);
            self.drag_pointer_id = Some(event.pointer_id());
            self.apply_divider_styles();
            event.prevent_default();
            return;
        }

        if let Some(pane_id) = Self::find_attr_u32(event.target(), "data-pane-id") {
            self.set_active_pane(pane_id);
        }
    }

    fn on_pointer_move(&mut self, event: PointerEvent) {
        let split_id = match self.drag_split_id {
            Some(id) => id,
            None => return,
        };
        if self.drag_pointer_id != Some(event.pointer_id()) {
            return;
        }

        let ratio = if let Some(view) = self.split_views.get(&split_id) {
            let rect = view.wrapper.get_bounding_client_rect();
            if view.direction == SplitDirection::Vertical && rect.width() > 0.0 {
                (event.client_x() as f64 - rect.left()) / rect.width()
            } else if view.direction == SplitDirection::Horizontal && rect.height() > 0.0 {
                (event.client_y() as f64 - rect.top()) / rect.height()
            } else {
                return;
            }
        } else {
            return;
        };

        let mut applied_ratio: Option<f64> = None;
        if let Some(node) = self.nodes.get_mut(&split_id) {
            if let WorkspaceNodeKind::Split { ratio: r, .. } = &mut node.kind {
                *r = ratio.clamp(0.15, 0.85);
                applied_ratio = Some(*r);
            }
        }
        if let Some(r) = applied_ratio {
            self.apply_split_ratio(split_id, r);
        }
    }

    fn on_pointer_up(&mut self, event: PointerEvent) {
        if self.drag_pointer_id == Some(event.pointer_id()) {
            self.drag_pointer_id = None;
            self.drag_split_id = None;
            self.apply_divider_styles();
        }
    }
}

#[wasm_bindgen]
pub struct ChartWorkspace {
    inner: Rc<RefCell<WorkspaceInner>>,
    container_target: EventTarget,
    window_target: EventTarget,
    _pointer_down: Closure<dyn FnMut(PointerEvent)>,
    _pointer_move: Closure<dyn FnMut(PointerEvent)>,
    _pointer_up: Closure<dyn FnMut(PointerEvent)>,
    _pointer_cancel: Closure<dyn FnMut(PointerEvent)>,
}

#[wasm_bindgen]
impl ChartWorkspace {
    #[wasm_bindgen(constructor)]
    pub fn new(container_id: &str) -> Result<ChartWorkspace, JsValue> {
        let document = WorkspaceInner::document()?;
        let container = document
            .get_element_by_id(container_id)
            .ok_or_else(|| JsValue::from_str("workspace container not found"))?
            .dyn_into::<HtmlDivElement>()?;
        let inner = Rc::new(RefCell::new(WorkspaceInner::new(container.clone())?));

        let container_target: EventTarget = container.clone().unchecked_into();
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("window unavailable"))?;
        let window_target: EventTarget = window.unchecked_into();

        let inner_down = inner.clone();
        let pointer_down = Closure::wrap(Box::new(move |event: PointerEvent| {
            inner_down.borrow_mut().on_pointer_down(event);
        }) as Box<dyn FnMut(PointerEvent)>);
        container_target.add_event_listener_with_callback(
            "pointerdown",
            pointer_down.as_ref().unchecked_ref(),
        )?;

        let inner_move = inner.clone();
        let pointer_move = Closure::wrap(Box::new(move |event: PointerEvent| {
            inner_move.borrow_mut().on_pointer_move(event);
        }) as Box<dyn FnMut(PointerEvent)>);
        window_target.add_event_listener_with_callback(
            "pointermove",
            pointer_move.as_ref().unchecked_ref(),
        )?;

        let inner_up = inner.clone();
        let pointer_up = Closure::wrap(Box::new(move |event: PointerEvent| {
            inner_up.borrow_mut().on_pointer_up(event);
        }) as Box<dyn FnMut(PointerEvent)>);
        window_target
            .add_event_listener_with_callback("pointerup", pointer_up.as_ref().unchecked_ref())?;

        let inner_cancel = inner.clone();
        let pointer_cancel = Closure::wrap(Box::new(move |event: PointerEvent| {
            inner_cancel.borrow_mut().on_pointer_up(event);
        }) as Box<dyn FnMut(PointerEvent)>);
        window_target.add_event_listener_with_callback(
            "pointercancel",
            pointer_cancel.as_ref().unchecked_ref(),
        )?;

        Ok(Self {
            inner,
            container_target,
            window_target,
            _pointer_down: pointer_down,
            _pointer_move: pointer_move,
            _pointer_up: pointer_up,
            _pointer_cancel: pointer_cancel,
        })
    }

    pub fn root_pane_id(&self) -> u32 {
        self.inner.borrow().pane_ids().first().copied().unwrap_or(0)
    }

    pub fn active_pane_id(&self) -> u32 {
        self.inner.borrow().active_pane_id()
    }

    pub fn set_active_pane(&mut self, pane_id: u32) -> bool {
        self.inner.borrow_mut().set_active_pane(pane_id)
    }

    pub fn pane_host_id(&self, pane_id: u32) -> String {
        self.inner
            .borrow()
            .pane_host_id(pane_id)
            .unwrap_or_default()
    }

    pub fn pane_ids(&self) -> js_sys::Array {
        let ids = self.inner.borrow().pane_ids();
        let out = js_sys::Array::new_with_length(ids.len() as u32);
        for (i, id) in ids.into_iter().enumerate() {
            out.set(i as u32, JsValue::from_f64(id as f64));
        }
        out
    }

    pub fn fullscreen_pane_id(&self) -> u32 {
        self.inner.borrow().fullscreen_pane_id().unwrap_or(0)
    }

    pub fn is_pane_fullscreen(&self) -> bool {
        self.inner.borrow().is_fullscreen()
    }

    pub fn toggle_pane_fullscreen(&mut self, pane_id: u32) -> bool {
        self.inner.borrow_mut().toggle_fullscreen_pane(pane_id)
    }

    pub fn clear_pane_fullscreen(&mut self) -> bool {
        self.inner.borrow_mut().clear_fullscreen()
    }

    pub fn split_active(&mut self, direction: &str) -> Result<u32, JsValue> {
        let direction = SplitDirection::parse(direction)
            .ok_or_else(|| JsValue::from_str("direction must be 'vertical' or 'horizontal'"))?;
        self.inner.borrow_mut().split_active(direction)
    }

    pub fn split_pane(&mut self, pane_id: u32, direction: &str) -> Result<u32, JsValue> {
        let direction = SplitDirection::parse(direction)
            .ok_or_else(|| JsValue::from_str("direction must be 'vertical' or 'horizontal'"))?;
        self.inner.borrow_mut().split_pane(pane_id, direction)
    }

    pub fn set_split_divider_thickness(&mut self, thickness_css: f64) {
        self.inner
            .borrow_mut()
            .set_split_divider_thickness(thickness_css);
    }

    pub fn set_split_divider_hit_area(&mut self, hit_area_css: f64) {
        self.inner
            .borrow_mut()
            .set_split_divider_hit_area(hit_area_css);
    }

    pub fn set_split_divider_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner
            .borrow_mut()
            .set_split_divider_color([r, g, b, a]);
    }

    pub fn set_split_divider_active_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner
            .borrow_mut()
            .set_split_divider_active_color([r, g, b, a]);
    }

    pub fn set_workspace_pane_background_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner
            .borrow_mut()
            .set_pane_background_color([r, g, b, a]);
    }

    pub fn set_workspace_active_pane_border_color(&mut self, r: f32, g: f32, b: f32, a: f32) {
        self.inner
            .borrow_mut()
            .set_active_pane_border_color([r, g, b, a]);
    }

    pub fn set_workspace_active_pane_border_width(&mut self, width_css: f64) {
        self.inner
            .borrow_mut()
            .set_active_pane_border_width(width_css);
    }

    pub fn dispose(&mut self) {
        let _ = self.container_target.remove_event_listener_with_callback(
            "pointerdown",
            self._pointer_down.as_ref().unchecked_ref(),
        );
        let _ = self.window_target.remove_event_listener_with_callback(
            "pointermove",
            self._pointer_move.as_ref().unchecked_ref(),
        );
        let _ = self.window_target.remove_event_listener_with_callback(
            "pointerup",
            self._pointer_up.as_ref().unchecked_ref(),
        );
        let _ = self.window_target.remove_event_listener_with_callback(
            "pointercancel",
            self._pointer_cancel.as_ref().unchecked_ref(),
        );
    }
}
