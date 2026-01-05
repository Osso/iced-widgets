use iced::advanced::layout::{self, Layout};
use iced::advanced::renderer;
use iced::advanced::widget::{self, Operation, Tree};
use iced::advanced::{Clipboard, Shell, Widget};
use iced::keyboard::{self, key::Named};
use iced::mouse::{self, Cursor};
use iced::widget::scrollable::Scrollable;
use iced::widget::{container, Column};
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
    /// The callback receives the target Y scroll offset. Use with `scrollable::scroll_to`:
    /// ```ignore
    /// .on_scroll_to(|y| Message::ScrollList(y))
    ///
    /// // In update():
    /// Message::ScrollList(y) => scrollable::scroll_to(list_id, AbsoluteOffset { x: 0.0, y })
    /// ```
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
        self.selected.map(|sel| {
            sel as isize - self.first_visible_index(item_height) as isize
        })
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
                index, item_top, item_bottom, scroll_y, self.viewport_height, viewport_bottom, result
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
        // No initial children - layout() will create the scrollable tree
        Vec::new()
    }

    fn diff(&self, tree: &mut Tree) {
        // Preserve first child (scrollable tree), clear any extras
        // Don't add Tree::empty() here - layout() will create with correct tag
        tree.children.truncate(1);
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let state = tree.state.downcast_mut::<State>();
        state.item_count = self.items.len();

        // Sync external selection
        if self.external_selected.is_some() {
            state.selected = self.external_selected;
        }

        // Use external selection if provided, otherwise internal
        let selected = self.external_selected.or(state.selected);
        let items: Vec<Element<'_, Message>> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = selected == Some(i);
                container((self.view)(item, is_selected))
                    .width(Length::Fill)
                    .height(Length::Fixed(self.item_height))
                    .into()
            })
            .collect();

        let content: Element<'_, Message> = Column::with_children(items).width(Length::Fill).into();

        let mut scrollable: Element<'_, Message> = Scrollable::new(content)
            .id(self.id.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        // Create child tree for the scrollable
        if tree.children.is_empty() {
            tree.children.push(Tree::new(&scrollable));
        } else {
            tree.children[0].diff(&scrollable);
        }

        let node =
            scrollable
                .as_widget_mut()
                .layout(&mut tree.children[0], renderer, limits);

        // Update viewport height for page calculations
        state.viewport_height = node.bounds().height;

        node
    }

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
        // Use external selection if provided, otherwise internal
        let selected = self.external_selected.or(state.selected);

        // Rebuild items for drawing
        let items: Vec<Element<'_, Message>> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = selected == Some(i);
                container((self.view)(item, is_selected))
                    .width(Length::Fill)
                    .height(Length::Fixed(self.item_height))
                    .into()
            })
            .collect();

        let content: Element<'_, Message> = Column::with_children(items).width(Length::Fill).into();

        let scrollable: Element<'_, Message> = Scrollable::new(content)
            .id(self.id.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

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

        // Detect external selection changes and scroll into view
        if let Some(ext_sel) = self.external_selected {
            if state.last_scrolled_to != Some(ext_sel) {
                state.last_scrolled_to = Some(ext_sel);
                state.selected = Some(ext_sel);
                if let Some(new_scroll_y) = state.scroll_into_view(ext_sel, self.item_height) {
                    state.set_scroll_offset_y(new_scroll_y);
                    if let Some(on_scroll_to) = &self.on_scroll_to {
                        shell.publish(on_scroll_to(new_scroll_y));
                    }
                }
            }
        }

        // Handle mouse clicks to select items
        if let Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) = event {
            if let Some(pos) = cursor.position_in(bounds) {
                // Calculate which item was clicked based on scroll position and click Y
                let scroll_y = state.scroll_offset_y();
                let click_y = pos.y + scroll_y;
                let clicked_index = (click_y / self.item_height) as usize;

                if clicked_index < self.items.len() {
                    let old_selected = state.selected;
                    state.selected = Some(clicked_index);

                    if state.selected != old_selected {
                        // Emit selection change callback
                        if let Some(on_select) = &self.on_select {
                            shell.publish(on_select(clicked_index));
                        }
                    }

                    shell.capture_event();
                    shell.request_redraw();
                    return;
                }
            }
        }

        // Handle keyboard events when cursor is over the list
        if cursor.is_over(bounds) {
            if let Event::Keyboard(keyboard::Event::KeyPressed { key, .. }) = event {
                let old_selected = state.selected;
                let mut handled = true;

                match key {
                    keyboard::Key::Named(Named::ArrowUp) => state.select_previous(),
                    keyboard::Key::Named(Named::ArrowDown) => state.select_next(),
                    keyboard::Key::Named(Named::PageUp) => state.page_up(self.item_height),
                    keyboard::Key::Named(Named::PageDown) => state.page_down(self.item_height),
                    keyboard::Key::Named(Named::Home) => state.select_first(),
                    keyboard::Key::Named(Named::End) => state.select_last(),
                    keyboard::Key::Named(Named::Enter) => {
                        if let (Some(selected), Some(on_activate)) =
                            (state.selected, &self.on_activate)
                        {
                            shell.publish(on_activate(selected));
                        }
                    }
                    _ => handled = false,
                }

                if handled {
                    // Handle selection change
                    if state.selected != old_selected {
                        if let Some(new_index) = state.selected {
                            // Check if we need to scroll to keep selection visible
                            if let Some(new_scroll_y) =
                                state.scroll_into_view(new_index, self.item_height)
                            {
                                state.set_scroll_offset_y(new_scroll_y);
                                if let Some(on_scroll_to) = &self.on_scroll_to {
                                    shell.publish(on_scroll_to(new_scroll_y));
                                }
                            }
                        }

                        // Debug dump on every selection change
                        eprintln!("[list] {}", state.dump(self.item_height));

                        // Emit selection change callback
                        if let (Some(selected), Some(on_select)) = (state.selected, &self.on_select)
                        {
                            shell.publish(on_select(selected));
                        }
                    }

                    shell.capture_event();
                    shell.request_redraw();
                    return;
                }
            }
        }

        // Forward other events to the scrollable
        let items: Vec<Element<'_, Message>> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = state.selected == Some(i);
                container((self.view)(item, is_selected))
                    .width(Length::Fill)
                    .height(Length::Fixed(self.item_height))
                    .into()
            })
            .collect();

        let content: Element<'_, Message> = Column::with_children(items).width(Length::Fill).into();

        // Build scrollable without on_scroll to avoid widget inconsistency issues
        // The scroll offset will be synced via scroll_into_view calculations
        let mut scrollable: Element<'_, Message> = Scrollable::new(content)
            .id(self.id.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

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

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        let state = tree.state.downcast_ref::<State>();
        let selected = self.external_selected.or(state.selected);

        let items: Vec<Element<'_, Message>> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = selected == Some(i);
                container((self.view)(item, is_selected))
                    .width(Length::Fill)
                    .height(Length::Fixed(self.item_height))
                    .into()
            })
            .collect();

        let content: Element<'_, Message> = Column::with_children(items).width(Length::Fill).into();

        let scrollable: Element<'_, Message> = Scrollable::new(content)
            .id(self.id.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        scrollable
            .as_widget()
            .mouse_interaction(&tree.children[0], layout, cursor, viewport, renderer)
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn Operation,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let selected = self.external_selected.or(state.selected);

        let items: Vec<Element<'_, Message>> = self
            .items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let is_selected = selected == Some(i);
                container((self.view)(item, is_selected))
                    .width(Length::Fill)
                    .height(Length::Fixed(self.item_height))
                    .into()
            })
            .collect();

        let content: Element<'_, Message> = Column::with_children(items).width(Length::Fill).into();

        let mut scrollable: Element<'_, Message> = Scrollable::new(content)
            .id(self.id.clone())
            .width(Length::Fill)
            .height(Length::Fill)
            .into();

        scrollable
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
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
