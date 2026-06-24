use iced::advanced::Widget;
use iced::advanced::widget::Tree;
use iced::widget::text;
use iced::{Length, Size};
use iced_widgets::List;

#[derive(Clone, Debug, PartialEq)]
enum Message {}

fn list<'a>(items: &'a [usize]) -> List<'a, usize, Message> {
    List::new(items, |item, selected| {
        text(format!("{item}:{selected}")).into()
    })
}

#[test]
fn widget_trait_metadata_uses_fill_size_and_empty_children() {
    let items = [1, 2, 3];
    let widget = list(&items);

    let _tag = widget.tag();
    let _state = widget.state();
    let size = widget.size();
    let children = widget.children();

    assert_eq!(size, Size::new(Length::Fill, Length::Fill));
    assert!(children.is_empty());
}

#[test]
fn widget_trait_diff_preserves_scrollable_child_only() {
    let items = [1, 2, 3];
    let widget = list(&items);
    let mut tree = Tree::empty();
    tree.children.push(Tree::empty());
    tree.children.push(Tree::empty());

    widget.diff(&mut tree);

    assert_eq!(tree.children.len(), 1);
}
