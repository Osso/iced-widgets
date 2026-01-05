use iced::widget::{column, container, text};
use iced::{Color, Element, Length, Task};
use iced_widgets::List;

fn main() -> iced::Result {
    iced::application(App::default, App::update, App::view)
        .title("List Demo")
        .run()
}

struct App {
    items: Vec<String>,
}

#[derive(Debug, Clone)]
enum Message {
    Selected(usize),
    Activated(usize),
}

impl App {
    fn default() -> Self {
        Self {
            items: (1..=50).map(|i| format!("Item {}", i)).collect(),
        }
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Selected(index) => {
                println!("Selected: {} (index {})", self.items[index], index);
            }
            Message::Activated(index) => {
                println!("Activated: {} (index {})", self.items[index], index);
            }
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let list = List::new(&self.items, |item, is_selected| {
            let bg = if is_selected {
                Color::from_rgb(0.2, 0.4, 0.8)
            } else {
                Color::TRANSPARENT
            };

            container(text(item.clone()))
                .padding(8)
                .style(move |_theme| container::Style {
                    background: Some(iced::Background::Color(bg)),
                    ..Default::default()
                })
                .into()
        })
        .item_height(40.0)
        .on_select(Message::Selected)
        .on_activate(Message::Activated);

        column![
            text("Keyboard List Demo").size(24),
            text("Hover over list and use arrow keys, Page Up/Down, Home/End").size(14),
            container(list)
                .width(Length::Fill)
                .height(Length::Fixed(400.0))
                .padding(10),
        ]
        .spacing(10)
        .padding(20)
        .into()
    }
}
