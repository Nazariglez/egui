use crate::*;

#[derive(Clone, Copy, Debug, Default, serde_derive::Deserialize, serde_derive::Serialize)]
#[serde(default)]
pub(crate) struct State {
    /// Positive offset means scrolling down/right
    offset: Vec2,

    show_scroll: bool, // TODO: default value?
}

// TODO: rename VScroll
#[derive(Clone, Debug)]
pub struct ScrollArea {
    max_height: f32,
    always_show_scroll: bool,
    auto_hide_scroll: bool,
}

impl Default for ScrollArea {
    fn default() -> Self {
        Self {
            max_height: 200.0,
            always_show_scroll: false,
            auto_hide_scroll: true,
        }
    }
}

impl ScrollArea {
    pub fn max_height(mut self, max_height: f32) -> Self {
        self.max_height = max_height;
        self
    }

    pub fn always_show_scroll(mut self, always_show_scroll: bool) -> Self {
        self.always_show_scroll = always_show_scroll;
        self
    }

    pub fn auto_hide_scroll(mut self, auto_hide_scroll: bool) -> Self {
        self.auto_hide_scroll = auto_hide_scroll;
        self
    }
}

struct Prepared {
    id: Id,
    state: State,
    current_scroll_bar_width: f32,
    always_show_scroll: bool,
    inner_rect: Rect,
    content_ui: Ui,
}

impl ScrollArea {
    fn prepare(self, ui: &mut Ui) -> Prepared {
        let Self {
            max_height,
            always_show_scroll,
            auto_hide_scroll,
        } = self;

        let ctx = ui.ctx().clone();

        let id = ui.make_child_id("scroll_area");
        let state = ctx
            .memory()
            .scroll_areas
            .get(&id)
            .cloned()
            .unwrap_or_default();

        // content: size of contents (generally large)
        // outer: size of scroll area including scroll bar(s)
        // inner: excluding scroll bar(s). The area we clip the contents to.

        let max_scroll_bar_width = 16.0;

        let current_scroll_bar_width = if state.show_scroll || !auto_hide_scroll {
            max_scroll_bar_width // TODO: animate?
        } else {
            0.0
        };

        let outer_size = vec2(
            ui.available().width(),
            ui.available().height().min(max_height),
        );

        let inner_size = outer_size - vec2(current_scroll_bar_width, 0.0);
        let inner_rect = Rect::from_min_size(ui.available().min, inner_size);

        let mut content_ui = ui.child_ui(Rect::from_min_size(
            inner_rect.min - state.offset,
            vec2(inner_size.x, f32::INFINITY),
        ));
        let mut content_clip_rect = ui.clip_rect().intersect(inner_rect);
        content_clip_rect.max.x = ui.clip_rect().max.x - current_scroll_bar_width; // Nice handling of forced resizing beyond the possible
        content_ui.set_clip_rect(content_clip_rect);

        Prepared {
            id,
            state,
            always_show_scroll,
            inner_rect,
            current_scroll_bar_width,
            content_ui,
        }
    }

    pub fn show<R>(self, ui: &mut Ui, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
        let mut prepared = self.prepare(ui);
        let ret = add_contents(&mut prepared.content_ui);
        Self::finish(ui, prepared);
        ret
    }

    fn finish(ui: &mut Ui, prepared: Prepared) {
        let Prepared {
            id,
            mut state,
            inner_rect,
            always_show_scroll,
            current_scroll_bar_width,
            content_ui,
        } = prepared;

        let content_size = content_ui.bounding_size();

        let inner_rect = Rect::from_min_size(
            inner_rect.min,
            vec2(
                inner_rect.width().max(content_size.x), // Expand width to fit content
                inner_rect.height(),
            ),
        );

        let outer_rect = Rect::from_min_size(
            inner_rect.min,
            inner_rect.size() + vec2(current_scroll_bar_width, 0.0),
        );

        let content_is_too_small = content_size.y > inner_rect.height();

        if content_is_too_small {
            // Dragg contents to scroll (for touch screens mostly):
            let content_interact = ui.interact_rect(inner_rect, id.with("area"));
            if content_interact.active {
                state.offset.y -= ui.input().mouse_move.y;
            }
        }

        // TODO: check that nothing else is being inteacted with
        if ui.contains_mouse(outer_rect) && ui.memory().active_id.is_none() {
            state.offset.y -= ui.input().scroll_delta.y;
        }

        let show_scroll_this_frame = content_is_too_small || always_show_scroll;
        if show_scroll_this_frame || state.show_scroll {
            let left = inner_rect.right() + 2.0;
            let right = outer_rect.right();
            let corner_radius = (right - left) / 2.0;
            let top = inner_rect.top();
            let bottom = inner_rect.bottom();

            let outer_scroll_rect = Rect::from_min_max(
                pos2(left, inner_rect.top()),
                pos2(right, inner_rect.bottom()),
            );

            let from_content =
                |content_y| remap_clamp(content_y, 0.0..=content_size.y, top..=bottom);

            let handle_rect = Rect::from_min_max(
                pos2(left, from_content(state.offset.y)),
                pos2(right, from_content(state.offset.y + inner_rect.height())),
            );

            // intentionally use same id for inside and outside of handle
            let interact_id = id.with("vertical");
            let handle_interact = ui.interact_rect(handle_rect, interact_id);

            if let Some(mouse_pos) = ui.input().mouse_pos {
                if handle_interact.active {
                    if inner_rect.top() <= mouse_pos.y && mouse_pos.y <= inner_rect.bottom() {
                        state.offset.y +=
                            ui.input().mouse_move.y * content_size.y / inner_rect.height();
                    }
                } else {
                    // Check for mouse down outside handle:
                    let scroll_bg_interact = ui.interact_rect(outer_scroll_rect, interact_id);

                    if scroll_bg_interact.active {
                        // Center scroll at mouse pos:
                        let mpos_top = mouse_pos.y - handle_rect.height() / 2.0;
                        state.offset.y = remap(mpos_top, top..=bottom, 0.0..=content_size.y);
                    }
                }
            }

            state.offset.y = state.offset.y.max(0.0);
            state.offset.y = state.offset.y.min(content_size.y - inner_rect.height());

            // Avoid frame-delay by calculating a new handle rect:
            let handle_rect = Rect::from_min_max(
                pos2(left, from_content(state.offset.y)),
                pos2(right, from_content(state.offset.y + inner_rect.height())),
            );

            let style = ui.style();
            let handle_fill_color = style.interact(&handle_interact).fill_color;
            let handle_outline = style.interact(&handle_interact).rect_outline;

            ui.add_paint_cmd(paint::PaintCmd::Rect {
                rect: outer_scroll_rect,
                corner_radius,
                fill_color: Some(ui.style().dark_bg_color),
                outline: None,
            });

            ui.add_paint_cmd(paint::PaintCmd::Rect {
                rect: handle_rect.expand(-2.0),
                corner_radius,
                fill_color: Some(handle_fill_color),
                outline: handle_outline,
            });
        }

        // let size = content_size.min(inner_rect.size());
        // let size = vec2(
        //     content_size.x, // ignore inner_rect, i.e. try to expand horizontally if necessary
        //     content_size.y.min(inner_rect.size().y), // respect vertical height.
        // );
        let size = outer_rect.size();
        ui.reserve_space(size, None);

        state.offset.y = state.offset.y.min(content_size.y - inner_rect.height());
        state.offset.y = state.offset.y.max(0.0);
        state.show_scroll = show_scroll_this_frame;

        ui.memory().scroll_areas.insert(id, state);
    }
}
