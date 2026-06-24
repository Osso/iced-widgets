use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Operation, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::keyboard;
use iced::mouse::{self, Cursor};
use iced::widget::scrollable::Scrollable;
use iced::widget::{Column, container};
use iced::{Element, Event, Length, Rectangle, Size, Theme};
use std::cell::Cell;
use std::rc::Rc;

/// A list widget with keyboard navigation and selection tracking.
///
/// Can operate in two modes:
/// - **Uncontrolled**: Selection tracked internally (default)
/// - **Controlled**: Selection provided via `selected()` method
pub struct List<'a, T, Message> {
    items: &'a [T],
    view: Box<dyn Fn(&T, bool) -> Element<'a, Message> + 'a>,
    item_height: f32,
    on_select: Option<Box<dyn Fn(usize) -> Message + 'a>>,
    on_activate: Option<Box<dyn Fn(usize) -> Message + 'a>>,
    on_scroll_to: Option<Rc<dyn Fn(f32) -> Message + 'a>>,
    id: iced::widget::Id,
    external_selected: Option<usize>,
}

impl<'a, T, Message> List<'a, T, Message>
where
    Message: Clone + 'a,
    T: 'a,
{
    /// Creates a new List widget.
    ///
    /// # Arguments
    /// * `items` - Slice of items to display
    /// * `view` - Function that renders each item. Receives (item, is_selected).
    pub fn new(items: &'a [T], view: impl Fn(&T, bool) -> Element<'a, Message> + 'a) -> Self {
        Self {
            items,
            view: Box::new(view),
            item_height: 32.0,
            on_select: None,
            on_activate: None,
            on_scroll_to: None,
            id: iced::widget::Id::unique(),
            external_selected: None,
        }
    }

    /// Sets the height of each item in pixels. Required for scroll calculations.
    pub fn item_height(mut self, height: f32) -> Self {
        self.item_height = height;
        self
    }

    /// Sets the callback when selection changes.
    pub fn on_select(mut self, f: impl Fn(usize) -> Message + 'a) -> Self {
        self.on_select = Some(Box::new(f));
        self
    }

    /// Sets the callback when Enter is pressed on selected item.
    pub fn on_activate(mut self, f: impl Fn(usize) -> Message + 'a) -> Self {
        self.on_activate = Some(Box::new(f));
        self
    }

    /// Sets the callback when scrolling is needed to keep selection visible.
    ///
    /// The callback receives the target Y scroll offset.
    pub fn on_scroll_to(mut self, f: impl Fn(f32) -> Message + 'a) -> Self {
        self.on_scroll_to = Some(Rc::new(f));
        self
    }

    /// Sets a specific scrollable ID for this list.
    pub fn id(mut self, id: iced::widget::Id) -> Self {
        self.id = id;
        self
    }

    /// Returns the scrollable ID for this list.
    /// Use this with `scrollable::scroll_to` to scroll programmatically.
    pub fn get_id(&self) -> iced::widget::Id {
        self.id.clone()
    }

    /// Returns the Y scroll offset to make the given item index the first visible.
    pub fn scroll_offset_for(&self, index: usize) -> f32 {
        index as f32 * self.item_height
    }

    /// Scrolls to make the n-th item the first visible.
    /// Returns the Message from on_scroll_to callback, or None if no callback set.
    pub fn scroll_to_item(&self, index: usize) -> Option<Message> {
        let y = self.scroll_offset_for(index);
        self.on_scroll_to.as_ref().map(|f| f(y))
    }

    /// Sets the selected item index (controlled mode).
    ///
    /// When set, the widget uses this value instead of tracking selection internally.
    /// The `is_selected` parameter in the view function will reflect this value.
    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.external_selected = index;
        self
    }

    fn current_selection(&self, state: &State) -> Option<usize> {
        self.external_selected.or(state.selected)
    }

    fn sync_layout_state(&self, state: &mut State) {
        state.item_count = self.items.len();

        // Controlled mode overrides internal selection.
        if self.external_selected.is_some() {
            state.selected = self.external_selected;
        }
    }

    fn build_item_elements(&self, selected: Option<usize>) -> Vec<Element<'a, Message>> {
        self.items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = selected == Some(i);
                container((self.view)(item, is_selected))
                    .width(Length::Fill)
                    .height(Length::Fixed(self.item_height))
                    .clip(true)
                    .into()
            })
            .collect()
    }

    fn build_scrollable(&self, selected: Option<usize>) -> Element<'a, Message> {
        let content: Element<'a, Message> =
            Column::with_children(self.build_item_elements(selected))
                .width(Length::Fill)
                .into();

        Scrollable::new(content)
            .id(self.id.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn ensure_scrollable_child(tree: &mut Tree, scrollable: &Element<'_, Message>) {
        if tree.children.is_empty() {
            tree.children.push(Tree::new(scrollable));
            return;
        }

        tree.children[0].diff(scrollable);
    }

    fn sync_external_selection_if_needed(&self, state: &mut State, shell: &mut Shell<'_, Message>) {
        let Some(ext_sel) = self.external_selected else {
            return;
        };

        sync_external_selection(state, ext_sel, self.item_height, &self.on_scroll_to, shell);
    }

    fn apply_wheel_scroll_if_over(
        &self,
        event: &Event,
        cursor: Cursor,
        bounds: Rectangle,
        state: &mut State,
    ) {
        let Event::Mouse(mouse::Event::WheelScrolled { delta }) = event else {
            return;
        };
        if !cursor.is_over(bounds) {
            return;
        }

        handle_wheel_scroll(
            state,
            delta,
            self.items.len(),
            self.item_height,
            bounds.height,
        );
    }

    fn handle_mouse_press(
        &self,
        event: &Event,
        cursor: Cursor,
        bounds: Rectangle,
        state: &mut State,
        shell: &mut Shell<'_, Message>,
    ) -> bool {
        let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event else {
            return false;
        };
        let Some(pos) = cursor.position_in(bounds) else {
            return false;
        };

        handle_mouse_click(
            state,
            pos.y,
            self.items.len(),
            self.item_height,
            &self.on_select,
            shell,
        )
    }

    fn handle_key_press(
        &self,
        event: &Event,
        cursor: Cursor,
        bounds: Rectangle,
        state: &mut State,
        shell: &mut Shell<'_, Message>,
    ) -> bool {
        if !cursor.is_over(bounds) {
            return false;
        }
        let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event else {
            return false;
        };

        handle_keyboard_nav(
            state,
            key,
            self.item_height,
            &self.on_activate,
            &self.on_scroll_to,
            &self.on_select,
            shell,
        )
    }

    fn consume_event(shell: &mut Shell<'_, Message>) {
        shell.capture_event();
        shell.request_redraw();
    }
}

#[derive(Debug, Clone, Default)]
struct State {
    selected: Option<usize>,
    viewport_height: f32,
    item_count: usize,
    scroll_offset: Rc<Cell<f32>>,
    /// Last external selection we scrolled to (to detect changes)
    last_scrolled_to: Option<usize>,
}

impl State {
    fn scroll_offset_y(&self) -> f32 {
        self.scroll_offset.get()
    }

    fn set_scroll_offset_y(&self, offset: f32) {
        self.scroll_offset.set(offset);
    }
}

impl State {
    /// Absolute Y position of selected element from the beginning of the list
    fn selected_absolute_y(&self, item_height: f32) -> Option<f32> {
        self.selected.map(|i| i as f32 * item_height)
    }

    /// Index of the first visible element (based on scroll offset)
    fn first_visible_index(&self, item_height: f32) -> usize {
        if item_height > 0.0 {
            (self.scroll_offset_y() / item_height).floor() as usize
        } else {
            0
        }
    }

    /// Relative position of selected element to first visible (in items, not pixels)
    fn selected_relative_to_visible(&self, item_height: f32) -> Option<isize> {
        self.selected
            .map(|sel| sel as isize - self.first_visible_index(item_height) as isize)
    }

    /// Number of fully visible elements
    fn visible_count(&self, item_height: f32) -> usize {
        if item_height > 0.0 {
            (self.viewport_height / item_height).floor() as usize
        } else {
            0
        }
    }

    /// Dump current state for debugging
    fn dump(&self, item_height: f32) -> String {
        let first_vis_idx = self.first_visible_index(item_height);
        let vis_count = self.visible_count(item_height);
        let last_vis_idx = first_vis_idx + vis_count.saturating_sub(1);

        format!(
            "selected={:?} scroll_y={:.0} viewport_h={:.0} items={} | visible: {}..{} ({} items) | selected_abs_y={:?} selected_rel={:?}",
            self.selected,
            self.scroll_offset_y(),
            self.viewport_height,
            self.item_count,
            first_vis_idx,
            last_vis_idx,
            vis_count,
            self.selected_absolute_y(item_height),
            self.selected_relative_to_visible(item_height),
        )
    }
}

impl State {
    fn visible_items(&self, item_height: f32) -> usize {
        if item_height > 0.0 {
            (self.viewport_height / item_height).floor() as usize
        } else {
            10
        }
    }

    /// Returns scroll offset to make `index` visible, or None if already visible.
    fn scroll_into_view(&self, index: usize, item_height: f32) -> Option<f32> {
        let item_top = index as f32 * item_height;
        let item_bottom = item_top + item_height;
        let scroll_y = self.scroll_offset_y();
        let viewport_bottom = scroll_y + self.viewport_height;

        let result = if item_top < scroll_y {
            // Item is above viewport - scroll up to show at top
            Some(item_top)
        } else if item_bottom > viewport_bottom {
            // Item is below viewport - scroll down to show at bottom
            Some(item_bottom - self.viewport_height)
        } else {
            // Already visible
            None
        };

        if result.is_some() {
            eprintln!(
                "[scroll] index={} item_top={:.0} item_bottom={:.0} scroll_y={:.0} viewport_h={:.0} viewport_bottom={:.0} -> scroll_to={:?}",
                index,
                item_top,
                item_bottom,
                scroll_y,
                self.viewport_height,
                viewport_bottom,
                result
            );
        }

        result
    }

    fn select_previous(&mut self) {
        self.selected = Some(match self.selected {
            Some(0) => 0, // stay at first
            Some(i) => i - 1,
            None => 0,
        });
    }

    fn select_next(&mut self) {
        let last = self.item_count.saturating_sub(1);
        self.selected = Some(match self.selected {
            Some(i) if i >= last => last, // stay at last
            Some(i) => i + 1,
            None => 0,
        });
    }

    fn page_up(&mut self, item_height: f32) {
        let page_size = self.visible_items(item_height).max(1);
        self.selected = Some(match self.selected {
            Some(i) => i.saturating_sub(page_size),
            None => 0,
        });
    }

    fn page_down(&mut self, item_height: f32) {
        let page_size = self.visible_items(item_height).max(1);
        self.selected = Some(match self.selected {
            Some(i) => (i + page_size).min(self.item_count.saturating_sub(1)),
            None => page_size.min(self.item_count.saturating_sub(1)),
        });
    }

    fn select_first(&mut self) {
        if self.item_count > 0 {
            self.selected = Some(0);
        }
    }

    fn select_last(&mut self) {
        if self.item_count > 0 {
            self.selected = Some(self.item_count - 1);
        }
    }
}

impl<'a, T, Message> Widget<Message, Theme, iced::Renderer> for List<'a, T, Message>
where
    Message: Clone + 'a,
    T: 'a,
{
    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size::new(Length::Fill, Length::Fill)
    }

    fn children(&self) -> Vec<Tree> {
        Vec::new()
    }

    fn diff(&self, tree: &mut Tree) {
        tree.children.truncate(1);
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        {
            let state = tree.state.downcast_mut::<State>();
            self.sync_layout_state(state);
        }

        let selected = {
            let state = tree.state.downcast_ref::<State>();
            self.current_selection(state)
        };
        let mut scrollable = self.build_scrollable(selected);
        Self::ensure_scrollable_child(tree, &scrollable);
        let node = scrollable
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits);

        tree.state.downcast_mut::<State>().viewport_height = node.bounds().height;
        node
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let selected = self.current_selection(state);
        let scrollable = self.build_scrollable(selected);

        scrollable.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_mut::<State>();
        let bounds = layout.bounds();

        self.sync_external_selection_if_needed(state, shell);
        self.apply_wheel_scroll_if_over(event, cursor, bounds, state);

        if self.handle_mouse_press(event, cursor, bounds, state, shell) {
            Self::consume_event(shell);
            return;
        }
        if self.handle_key_press(event, cursor, bounds, state, shell) {
            Self::consume_event(shell);
            return;
        }

        let mut scrollable = self.build_scrollable(state.selected);

        scrollable.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        let selected = self.current_selection(state);
        let scrollable = self.build_scrollable(selected);

        scrollable.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let selected = self.current_selection(state);
        let mut scrollable = self.build_scrollable(selected);

        scrollable
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
}

fn sync_external_selection<'a, Message: Clone + 'a>(
    state: &mut State,
    ext_sel: usize,
    item_height: f32,
    on_scroll_to: &Option<Rc<dyn Fn(f32) -> Message + 'a>>,
    shell: &mut Shell<'_, Message>,
) {
    if state.last_scrolled_to == Some(ext_sel) {
        return;
    }
    state.last_scrolled_to = Some(ext_sel);
    state.selected = Some(ext_sel);
    if let Some(new_scroll_y) = state.scroll_into_view(ext_sel, item_height) {
        state.set_scroll_offset_y(new_scroll_y);
        if let Some(on_scroll_to) = on_scroll_to {
            shell.publish(on_scroll_to(new_scroll_y));
        }
    }
}

fn handle_wheel_scroll(
    state: &mut State,
    delta: &mouse::ScrollDelta,
    item_count: usize,
    item_height: f32,
    viewport_height: f32,
) {
    let content_height = item_count as f32 * item_height;
    let max_scroll = (content_height - viewport_height).max(0.0);
    let delta_y = match delta {
        mouse::ScrollDelta::Lines { y, .. } => -y * 20.0,
        mouse::ScrollDelta::Pixels { y, .. } => -y,
    };
    let new_offset = (state.scroll_offset_y() + delta_y).clamp(0.0, max_scroll);
    state.set_scroll_offset_y(new_offset);
}

fn handle_mouse_click<'a, Message: Clone + 'a>(
    state: &mut State,
    click_y_in_bounds: f32,
    item_count: usize,
    item_height: f32,
    on_select: &Option<Box<dyn Fn(usize) -> Message + 'a>>,
    shell: &mut Shell<'_, Message>,
) -> bool {
    let click_y = click_y_in_bounds + state.scroll_offset_y();
    let clicked_index = (click_y / item_height) as usize;
    if clicked_index >= item_count {
        return false;
    }
    let old_selected = state.selected;
    state.selected = Some(clicked_index);
    if state.selected != old_selected {
        if let Some(on_select) = on_select {
            shell.publish(on_select(clicked_index));
        }
    }
    true
}

fn handle_keyboard_nav<'a, Message: Clone + 'a>(
    state: &mut State,
    key: &iced::keyboard::Key,
    item_height: f32,
    on_activate: &Option<Box<dyn Fn(usize) -> Message + 'a>>,
    on_scroll_to: &Option<Rc<dyn Fn(f32) -> Message + 'a>>,
    on_select: &Option<Box<dyn Fn(usize) -> Message + 'a>>,
    shell: &mut Shell<'_, Message>,
) -> bool {
    let old_selected = state.selected;
    let Some(action) = keyboard_action(key) else {
        return false;
    };

    match action {
        KeyboardAction::Move(movement) => apply_keyboard_movement(state, movement, item_height),
        KeyboardAction::Activate => publish_activation(state.selected, on_activate, shell),
    }

    if state.selected != old_selected {
        publish_selection_change(state, item_height, on_scroll_to, on_select, shell);
    }

    true
}

#[derive(Clone, Copy)]
enum KeyboardAction {
    Move(KeyboardMovement),
    Activate,
}

#[derive(Clone, Copy)]
enum KeyboardMovement {
    Previous,
    Next,
    PageUp,
    PageDown,
    First,
    Last,
}

fn keyboard_action(key: &iced::keyboard::Key) -> Option<KeyboardAction> {
    use iced::keyboard::key::Named;

    match key {
        keyboard::Key::Named(Named::ArrowUp) => {
            Some(KeyboardAction::Move(KeyboardMovement::Previous))
        }
        keyboard::Key::Named(Named::ArrowDown) => {
            Some(KeyboardAction::Move(KeyboardMovement::Next))
        }
        keyboard::Key::Named(Named::PageUp) => Some(KeyboardAction::Move(KeyboardMovement::PageUp)),
        keyboard::Key::Named(Named::PageDown) => {
            Some(KeyboardAction::Move(KeyboardMovement::PageDown))
        }
        keyboard::Key::Named(Named::Home) => Some(KeyboardAction::Move(KeyboardMovement::First)),
        keyboard::Key::Named(Named::End) => Some(KeyboardAction::Move(KeyboardMovement::Last)),
        keyboard::Key::Named(Named::Enter) => Some(KeyboardAction::Activate),
        _ => None,
    }
}

fn apply_keyboard_movement(state: &mut State, movement: KeyboardMovement, item_height: f32) {
    match movement {
        KeyboardMovement::Previous => state.select_previous(),
        KeyboardMovement::Next => state.select_next(),
        KeyboardMovement::PageUp => state.page_up(item_height),
        KeyboardMovement::PageDown => state.page_down(item_height),
        KeyboardMovement::First => state.select_first(),
        KeyboardMovement::Last => state.select_last(),
    }
}

fn publish_activation<'a, Message: Clone + 'a>(
    selected: Option<usize>,
    on_activate: &Option<Box<dyn Fn(usize) -> Message + 'a>>,
    shell: &mut Shell<'_, Message>,
) {
    if let (Some(selected), Some(on_activate)) = (selected, on_activate) {
        shell.publish(on_activate(selected));
    }
}

fn publish_selection_change<'a, Message: Clone + 'a>(
    state: &mut State,
    item_height: f32,
    on_scroll_to: &Option<Rc<dyn Fn(f32) -> Message + 'a>>,
    on_select: &Option<Box<dyn Fn(usize) -> Message + 'a>>,
    shell: &mut Shell<'_, Message>,
) {
    let Some(new_index) = state.selected else {
        return;
    };

    if let Some(new_scroll_y) = state.scroll_into_view(new_index, item_height) {
        state.set_scroll_offset_y(new_scroll_y);
        if let Some(on_scroll_to) = on_scroll_to {
            shell.publish(on_scroll_to(new_scroll_y));
        }
    }

    eprintln!("[list] {}", state.dump(item_height));
    if let Some(on_select) = on_select {
        shell.publish(on_select(new_index));
    }
}

impl<'a, T, Message> From<List<'a, T, Message>> for Element<'a, Message>
where
    Message: Clone + 'a,
    T: 'a,
{
    fn from(list: List<'a, T, Message>) -> Self {
        Element::new(list)
    }
}

#[cfg(test)]
mod tests;
