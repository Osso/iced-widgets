use super::*;
use iced::Point;
use iced::advanced::Shell;
use iced::keyboard::key::{Code, Key, Named, Physical};
use iced::keyboard::{Location, Modifiers};
use iced::widget::text;

#[derive(Clone, Debug, PartialEq)]
enum TestMessage {
    Selected(usize),
    Activated(usize),
    Scrolled(f32),
}

fn list<'a>(items: &'a [usize]) -> List<'a, usize, TestMessage> {
    List::new(items, |item, selected| {
        text(format!("{item}:{selected}")).into()
    })
}

fn state(selected: Option<usize>, item_count: usize, viewport_height: f32) -> State {
    State {
        selected,
        item_count,
        viewport_height,
        ..State::default()
    }
}

fn messages_from(run: impl FnOnce(&mut Shell<'_, TestMessage>)) -> Vec<TestMessage> {
    let mut messages = Vec::new();
    let mut shell = Shell::new(&mut messages);
    run(&mut shell);
    messages
}

fn cursor_over_list() -> Cursor {
    Cursor::Available(Point::new(5.0, 5.0))
}

fn cursor_outside_list() -> Cursor {
    Cursor::Available(Point::new(500.0, 500.0))
}

fn list_bounds() -> Rectangle {
    Rectangle {
        x: 0.0,
        y: 0.0,
        width: 100.0,
        height: 30.0,
    }
}

fn key_pressed(key: Key) -> Event {
    Event::Keyboard(keyboard::Event::KeyPressed {
        key: key.clone(),
        modified_key: key,
        physical_key: Physical::Code(Code::ArrowDown),
        location: Location::Standard,
        modifiers: Modifiers::NONE,
        text: None,
        repeat: false,
    })
}

#[test]
fn builder_configures_scroll_and_controlled_selection() {
    let items = [1, 2, 3];
    let widget = list(&items)
        .item_height(12.5)
        .on_select(TestMessage::Selected)
        .on_activate(TestMessage::Activated)
        .on_scroll_to(TestMessage::Scrolled)
        .id(iced::widget::Id::new("items"))
        .selected(Some(2));
    let internal_state = state(Some(1), items.len(), 24.0);

    assert_eq!(
        format!("{:?}", widget.get_id()),
        format!("{:?}", iced::widget::Id::new("items"))
    );
    assert_eq!(widget.scroll_offset_for(3), 37.5);
    assert_eq!(widget.scroll_to_item(2), Some(TestMessage::Scrolled(25.0)));
    assert_eq!(widget.current_selection(&internal_state), Some(2));
}

#[test]
fn current_selection_falls_back_to_internal_state() {
    let items = [1, 2, 3];
    let widget = list(&items);
    let internal_state = state(Some(1), items.len(), 24.0);

    assert_eq!(widget.scroll_to_item(2), None);
    assert_eq!(widget.current_selection(&internal_state), Some(1));
}

#[test]
fn sync_layout_state_updates_count_and_external_selection() {
    let items = [1, 2, 3, 4];
    let widget = list(&items).selected(Some(3));
    let mut internal_state = state(Some(1), 0, 24.0);

    widget.sync_layout_state(&mut internal_state);

    assert_eq!(internal_state.item_count, 4);
    assert_eq!(internal_state.selected, Some(3));
}

#[test]
fn sync_layout_state_preserves_internal_selection_without_external_selection() {
    let items = [1, 2, 3, 4];
    let widget = list(&items);
    let mut internal_state = state(Some(1), 0, 24.0);

    widget.sync_layout_state(&mut internal_state);

    assert_eq!(internal_state.item_count, 4);
    assert_eq!(internal_state.selected, Some(1));
}

#[test]
fn build_item_elements_runs_view_for_each_selection_state() {
    let items = [1, 2, 3];
    let rendered = Rc::new(Cell::new(0));
    let selected_hits = Rc::new(Cell::new(0));
    let rendered_in_view = Rc::clone(&rendered);
    let selected_hits_in_view = Rc::clone(&selected_hits);
    let widget: List<'_, usize, TestMessage> = List::new(&items, move |item, selected| {
        rendered_in_view.set(rendered_in_view.get() + 1);
        if selected {
            selected_hits_in_view.set(selected_hits_in_view.get() + 1);
        }
        text(format!("{item}:{selected}")).into()
    });

    let elements = widget.build_item_elements(Some(1));

    assert_eq!(elements.len(), 3);
    assert_eq!(rendered.get(), 3);
    assert_eq!(selected_hits.get(), 1);
}

#[test]
fn ensure_scrollable_child_creates_and_diffs_existing_child() {
    let items = [1, 2, 3];
    let widget = list(&items);
    let mut tree = Tree::empty();
    let scrollable = widget.build_scrollable(Some(1));

    List::<usize, TestMessage>::ensure_scrollable_child(&mut tree, &scrollable);
    assert_eq!(tree.children.len(), 1);

    let updated_scrollable = widget.build_scrollable(Some(2));
    List::<usize, TestMessage>::ensure_scrollable_child(&mut tree, &updated_scrollable);
    assert_eq!(tree.children.len(), 1);
}

#[test]
fn visible_state_accounts_for_scroll_and_zero_height_items() {
    let internal_state = state(Some(4), 10, 35.0);
    internal_state.set_scroll_offset_y(20.0);

    assert_eq!(internal_state.scroll_offset_y(), 20.0);
    assert_eq!(internal_state.first_visible_index(10.0), 2);
    assert_eq!(internal_state.selected_absolute_y(10.0), Some(40.0));
    assert_eq!(internal_state.selected_relative_to_visible(10.0), Some(2));
    assert_eq!(internal_state.visible_count(10.0), 3);
    assert_eq!(internal_state.visible_items(10.0), 3);
    assert_eq!(internal_state.first_visible_index(0.0), 0);
    assert_eq!(internal_state.visible_count(0.0), 0);
    assert_eq!(internal_state.visible_items(0.0), 10);
}

#[test]
fn dump_includes_selection_scroll_and_visible_range() {
    let internal_state = state(Some(2), 8, 30.0);
    internal_state.set_scroll_offset_y(10.0);

    let dump = internal_state.dump(10.0);

    assert!(dump.contains("selected=Some(2)"));
    assert!(dump.contains("scroll_y=10"));
    assert!(dump.contains("visible: 1..3 (3 items)"));
    assert!(dump.contains("selected_abs_y=Some(20.0)"));
    assert!(dump.contains("selected_rel=Some(1)"));
}

#[test]
fn scroll_into_view_reports_only_needed_offsets() {
    let internal_state = state(Some(0), 20, 30.0);
    internal_state.set_scroll_offset_y(20.0);

    assert_eq!(internal_state.scroll_into_view(1, 10.0), Some(10.0));
    assert_eq!(internal_state.scroll_into_view(2, 10.0), None);
    assert_eq!(internal_state.scroll_into_view(5, 10.0), Some(30.0));
}

#[test]
fn selection_movement_clamps_at_edges() {
    let mut internal_state = state(None, 5, 20.0);

    internal_state.select_previous();
    assert_eq!(internal_state.selected, Some(0));
    internal_state.select_previous();
    assert_eq!(internal_state.selected, Some(0));
    internal_state.select_next();
    assert_eq!(internal_state.selected, Some(1));
    internal_state.select_last();
    assert_eq!(internal_state.selected, Some(4));
    internal_state.select_next();
    assert_eq!(internal_state.selected, Some(4));
    internal_state.select_first();
    assert_eq!(internal_state.selected, Some(0));
}

#[test]
fn select_next_starts_empty_selection_at_zero() {
    let mut internal_state = state(None, 5, 20.0);

    internal_state.select_next();

    assert_eq!(internal_state.selected, Some(0));
}

#[test]
fn page_movement_uses_visible_item_count() {
    let mut internal_state = state(Some(5), 10, 30.0);

    internal_state.page_up(10.0);
    assert_eq!(internal_state.selected, Some(2));
    internal_state.page_down(10.0);
    assert_eq!(internal_state.selected, Some(5));
    internal_state.page_down(10.0);
    assert_eq!(internal_state.selected, Some(8));
    internal_state.page_down(10.0);
    assert_eq!(internal_state.selected, Some(9));

    let mut empty_selection = state(None, 10, 30.0);
    empty_selection.page_down(10.0);
    assert_eq!(empty_selection.selected, Some(3));
    let mut page_up_without_selection = state(None, 10, 30.0);
    page_up_without_selection.page_up(10.0);
    assert_eq!(page_up_without_selection.selected, Some(0));
}

#[test]
fn first_and_last_ignore_empty_lists() {
    let mut internal_state = state(None, 0, 30.0);

    internal_state.select_first();
    internal_state.select_last();

    assert_eq!(internal_state.selected, None);
}

#[test]
fn wheel_scroll_supports_lines_pixels_and_bounds() {
    let mut internal_state = state(None, 10, 30.0);

    handle_wheel_scroll(
        &mut internal_state,
        &mouse::ScrollDelta::Lines { x: 0.0, y: -2.0 },
        10,
        10.0,
        30.0,
    );
    assert_eq!(internal_state.scroll_offset_y(), 40.0);

    handle_wheel_scroll(
        &mut internal_state,
        &mouse::ScrollDelta::Pixels { x: 0.0, y: 100.0 },
        10,
        10.0,
        30.0,
    );
    assert_eq!(internal_state.scroll_offset_y(), 0.0);

    handle_wheel_scroll(
        &mut internal_state,
        &mouse::ScrollDelta::Pixels { x: 0.0, y: -200.0 },
        10,
        10.0,
        30.0,
    );
    assert_eq!(internal_state.scroll_offset_y(), 70.0);
}

#[test]
fn wheel_wrapper_ignores_unrelated_events_and_outside_cursor() {
    let items = [1, 2, 3, 4, 5];
    let widget = list(&items);
    let mut internal_state = state(None, 5, 30.0);
    let wheel_event = Event::Mouse(mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -20.0 },
    });

    widget.apply_wheel_scroll_if_over(
        &Event::Keyboard(keyboard::Event::ModifiersChanged(Modifiers::SHIFT)),
        cursor_over_list(),
        list_bounds(),
        &mut internal_state,
    );
    widget.apply_wheel_scroll_if_over(
        &wheel_event,
        cursor_outside_list(),
        list_bounds(),
        &mut internal_state,
    );

    assert_eq!(internal_state.scroll_offset_y(), 0.0);
}

#[test]
fn wheel_wrapper_scrolls_when_cursor_is_over_bounds() {
    let items = [1, 2, 3, 4, 5];
    let widget = list(&items);
    let mut internal_state = state(None, 5, 30.0);
    let wheel_event = Event::Mouse(mouse::Event::WheelScrolled {
        delta: mouse::ScrollDelta::Pixels { x: 0.0, y: -20.0 },
    });

    widget.apply_wheel_scroll_if_over(
        &wheel_event,
        cursor_over_list(),
        list_bounds(),
        &mut internal_state,
    );

    assert_eq!(internal_state.scroll_offset_y(), 20.0);
}

#[test]
fn mouse_click_selects_visible_item_and_publishes_changes_once() {
    let on_select: Option<Box<dyn Fn(usize) -> TestMessage>> =
        Some(Box::new(TestMessage::Selected));
    let mut internal_state = state(Some(1), 5, 30.0);
    internal_state.set_scroll_offset_y(10.0);

    let messages = messages_from(|shell| {
        assert!(handle_mouse_click(
            &mut internal_state,
            15.0,
            5,
            10.0,
            &on_select,
            shell,
        ));
        assert!(handle_mouse_click(
            &mut internal_state,
            15.0,
            5,
            10.0,
            &on_select,
            shell,
        ));
        assert!(!handle_mouse_click(
            &mut internal_state,
            100.0,
            5,
            10.0,
            &on_select,
            shell,
        ));
    });

    assert_eq!(internal_state.selected, Some(2));
    assert_eq!(messages, vec![TestMessage::Selected(2)]);
}

#[test]
fn mouse_press_wrapper_filters_event_and_cursor_before_selecting() {
    let items = [1, 2, 3, 4, 5];
    let widget = list(&items).on_select(TestMessage::Selected);
    let mut internal_state = state(None, 5, 30.0);
    let left_press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left));
    let right_press = Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Right));

    let messages = messages_from(|shell| {
        assert!(!widget.handle_mouse_press(
            &right_press,
            cursor_over_list(),
            list_bounds(),
            &mut internal_state,
            shell,
        ));
        assert!(!widget.handle_mouse_press(
            &left_press,
            cursor_outside_list(),
            list_bounds(),
            &mut internal_state,
            shell,
        ));
        assert!(widget.handle_mouse_press(
            &left_press,
            cursor_over_list(),
            list_bounds(),
            &mut internal_state,
            shell,
        ));
    });

    assert_eq!(internal_state.selected, Some(0));
    assert_eq!(messages, vec![TestMessage::Selected(0)]);
}

#[test]
fn keyboard_action_maps_navigation_activation_and_unknown_keys() {
    assert!(matches!(
        keyboard_action(&Key::Named(Named::ArrowUp)),
        Some(KeyboardAction::Move(KeyboardMovement::Previous))
    ));
    assert!(matches!(
        keyboard_action(&Key::Named(Named::ArrowDown)),
        Some(KeyboardAction::Move(KeyboardMovement::Next))
    ));
    assert!(matches!(
        keyboard_action(&Key::Named(Named::PageUp)),
        Some(KeyboardAction::Move(KeyboardMovement::PageUp))
    ));
    assert!(matches!(
        keyboard_action(&Key::Named(Named::PageDown)),
        Some(KeyboardAction::Move(KeyboardMovement::PageDown))
    ));
    assert!(matches!(
        keyboard_action(&Key::Named(Named::Home)),
        Some(KeyboardAction::Move(KeyboardMovement::First))
    ));
    assert!(matches!(
        keyboard_action(&Key::Named(Named::End)),
        Some(KeyboardAction::Move(KeyboardMovement::Last))
    ));
    assert!(matches!(
        keyboard_action(&Key::Named(Named::Enter)),
        Some(KeyboardAction::Activate)
    ));
    assert!(keyboard_action(&Key::Character("x".into())).is_none());
}

#[test]
fn keyboard_movement_delegates_to_state_transitions() {
    let mut internal_state = state(Some(2), 6, 20.0);

    apply_keyboard_movement(&mut internal_state, KeyboardMovement::Previous, 10.0);
    assert_eq!(internal_state.selected, Some(1));
    apply_keyboard_movement(&mut internal_state, KeyboardMovement::Next, 10.0);
    assert_eq!(internal_state.selected, Some(2));
    apply_keyboard_movement(&mut internal_state, KeyboardMovement::PageUp, 10.0);
    assert_eq!(internal_state.selected, Some(0));
    apply_keyboard_movement(&mut internal_state, KeyboardMovement::PageDown, 10.0);
    assert_eq!(internal_state.selected, Some(2));
    apply_keyboard_movement(&mut internal_state, KeyboardMovement::Last, 10.0);
    assert_eq!(internal_state.selected, Some(5));
    apply_keyboard_movement(&mut internal_state, KeyboardMovement::First, 10.0);
    assert_eq!(internal_state.selected, Some(0));
}

#[test]
fn keyboard_nav_publishes_selection_scroll_and_activation() {
    let on_select: Option<Box<dyn Fn(usize) -> TestMessage>> =
        Some(Box::new(TestMessage::Selected));
    let on_activate: Option<Box<dyn Fn(usize) -> TestMessage>> =
        Some(Box::new(TestMessage::Activated));
    let on_scroll_to: Option<Rc<dyn Fn(f32) -> TestMessage>> = Some(Rc::new(TestMessage::Scrolled));
    let mut internal_state = state(Some(1), 10, 20.0);

    let messages = messages_from(|shell| {
        assert!(handle_keyboard_nav(
            &mut internal_state,
            &Key::Named(Named::PageDown),
            10.0,
            &on_activate,
            &on_scroll_to,
            &on_select,
            shell,
        ));
        assert!(handle_keyboard_nav(
            &mut internal_state,
            &Key::Named(Named::Enter),
            10.0,
            &on_activate,
            &on_scroll_to,
            &on_select,
            shell,
        ));
        assert!(!handle_keyboard_nav(
            &mut internal_state,
            &Key::Character("x".into()),
            10.0,
            &on_activate,
            &on_scroll_to,
            &on_select,
            shell,
        ));
    });

    assert_eq!(internal_state.selected, Some(3));
    assert_eq!(
        messages,
        vec![
            TestMessage::Scrolled(20.0),
            TestMessage::Selected(3),
            TestMessage::Activated(3),
        ]
    );
}

#[test]
fn keyboard_nav_without_callbacks_still_handles_known_keys() {
    let mut internal_state = state(Some(0), 3, 20.0);

    let messages = messages_from(|shell| {
        assert!(handle_keyboard_nav(
            &mut internal_state,
            &Key::Named(Named::ArrowDown),
            10.0,
            &None,
            &None,
            &None,
            shell,
        ));
        assert!(handle_keyboard_nav(
            &mut internal_state,
            &Key::Named(Named::Enter),
            10.0,
            &None,
            &None,
            &None,
            shell,
        ));
    });

    assert_eq!(internal_state.selected, Some(1));
    assert!(messages.is_empty());
}

#[test]
fn key_press_wrapper_filters_event_and_cursor_before_navigating() {
    let items = [1, 2, 3, 4, 5];
    let widget = list(&items)
        .on_select(TestMessage::Selected)
        .on_scroll_to(TestMessage::Scrolled);
    let mut internal_state = state(Some(0), 5, 20.0);
    let arrow_down = key_pressed(Key::Named(Named::ArrowDown));

    let messages = messages_from(|shell| {
        assert!(!widget.handle_key_press(
            &arrow_down,
            cursor_outside_list(),
            list_bounds(),
            &mut internal_state,
            shell,
        ));
        assert!(!widget.handle_key_press(
            &Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)),
            cursor_over_list(),
            list_bounds(),
            &mut internal_state,
            shell,
        ));
        assert!(widget.handle_key_press(
            &arrow_down,
            cursor_over_list(),
            list_bounds(),
            &mut internal_state,
            shell,
        ));
    });

    assert_eq!(internal_state.selected, Some(1));
    assert_eq!(
        messages,
        vec![TestMessage::Scrolled(44.0), TestMessage::Selected(1)]
    );
}

#[test]
fn external_selection_syncs_once_and_scrolls_when_needed() {
    let on_scroll_to: Option<Rc<dyn Fn(f32) -> TestMessage>> = Some(Rc::new(TestMessage::Scrolled));
    let mut internal_state = state(Some(0), 10, 20.0);

    let messages = messages_from(|shell| {
        sync_external_selection(&mut internal_state, 3, 10.0, &on_scroll_to, shell);
        sync_external_selection(&mut internal_state, 3, 10.0, &on_scroll_to, shell);
        sync_external_selection(&mut internal_state, 1, 10.0, &on_scroll_to, shell);
    });

    assert_eq!(internal_state.selected, Some(1));
    assert_eq!(internal_state.last_scrolled_to, Some(1));
    assert_eq!(internal_state.scroll_offset_y(), 10.0);
    assert_eq!(
        messages,
        vec![TestMessage::Scrolled(20.0), TestMessage::Scrolled(10.0)]
    );
}

#[test]
fn external_selection_wrapper_noops_without_controlled_selection() {
    let items = [1, 2, 3];
    let widget = list(&items);
    let mut internal_state = state(Some(0), 3, 20.0);

    let messages = messages_from(|shell| {
        widget.sync_external_selection_if_needed(&mut internal_state, shell);
    });

    assert_eq!(internal_state.selected, Some(0));
    assert_eq!(internal_state.last_scrolled_to, None);
    assert!(messages.is_empty());
}

#[test]
fn external_selection_wrapper_syncs_controlled_selection() {
    let items = [1, 2, 3];
    let widget = list(&items)
        .selected(Some(2))
        .on_scroll_to(TestMessage::Scrolled);
    let mut internal_state = state(Some(0), 3, 10.0);

    let messages = messages_from(|shell| {
        widget.sync_external_selection_if_needed(&mut internal_state, shell);
    });

    assert_eq!(internal_state.selected, Some(2));
    assert_eq!(internal_state.last_scrolled_to, Some(2));
    assert_eq!(messages, vec![TestMessage::Scrolled(86.0)]);
}

#[test]
fn external_selection_without_scroll_callback_still_updates_state() {
    let mut internal_state = state(Some(0), 10, 20.0);

    let messages = messages_from(|shell| {
        sync_external_selection(&mut internal_state, 3, 10.0, &None, shell);
    });

    assert_eq!(internal_state.selected, Some(3));
    assert_eq!(internal_state.last_scrolled_to, Some(3));
    assert_eq!(internal_state.scroll_offset_y(), 20.0);
    assert!(messages.is_empty());
}

#[test]
fn consume_event_captures_and_requests_redraw() {
    let mut messages = Vec::new();
    let mut shell: Shell<'_, TestMessage> = Shell::new(&mut messages);

    List::<usize, TestMessage>::consume_event(&mut shell);

    assert!(shell.is_event_captured());
    assert_ne!(shell.redraw_request(), iced::window::RedrawRequest::Wait);
}

#[test]
fn list_converts_into_element() {
    let items = [1, 2, 3];
    let _: Element<'_, TestMessage> = list(&items).into();
}
