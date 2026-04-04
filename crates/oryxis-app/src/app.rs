use iced::border::Radius;
use iced::keyboard;
use iced::widget::{button, canvas, column, container, row, scrollable, text, Space};
use iced::{Background, Border, Color, Element, Length, Padding, Subscription, Task, Theme};

use oryxis_terminal::widget::{TerminalState, TerminalView};

use std::sync::{Arc, Mutex};
use tokio_stream::wrappers::UnboundedReceiverStream;

use crate::theme::OryxisColors;

/// Root application state.
pub struct Oryxis {
    active_view: View,
    connections: Vec<SidebarEntry>,
    selected: Option<usize>,
    // Terminal
    terminal: Option<Arc<Mutex<TerminalState>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Dashboard,
    Terminal,
    Keys,
    Snippets,
    Settings,
}

#[derive(Debug, Clone)]
struct SidebarEntry {
    label: String,
    group: String,
    connected: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    SelectConnection(usize),
    ChangeView(View),
    /// Raw bytes from PTY output.
    PtyOutput(Vec<u8>),
    /// Keyboard event for the terminal.
    KeyboardEvent(keyboard::Event),
}

impl Oryxis {
    pub fn boot() -> (Self, Task<Message>) {
        let connections = vec![
            SidebarEntry { label: "Local Shell".into(), group: "Local".into(), connected: false },
            SidebarEntry { label: "prod-web-01".into(), group: "Production".into(), connected: false },
            SidebarEntry { label: "prod-db-01".into(), group: "Production".into(), connected: false },
            SidebarEntry { label: "bastion".into(), group: "Production".into(), connected: false },
            SidebarEntry { label: "staging-web".into(), group: "Staging".into(), connected: false },
            SidebarEntry { label: "staging-db".into(), group: "Staging".into(), connected: false },
        ];

        (
            Self {
                active_view: View::Dashboard,
                connections,
                selected: None,
                terminal: None,
            },
            Task::none(),
        )
    }

    pub fn title(&self) -> String {
        "Oryxis".into()
    }

    pub fn theme(&self) -> Theme {
        Theme::custom(
            String::from("Oryxis Dark"),
            iced::theme::Palette {
                background: OryxisColors::BG_PRIMARY,
                text: OryxisColors::TEXT_PRIMARY,
                primary: OryxisColors::ACCENT,
                success: OryxisColors::SUCCESS,
                warning: OryxisColors::WARNING,
                danger: OryxisColors::ERROR,
            },
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectConnection(idx) => {
                self.selected = Some(idx);
                self.active_view = View::Terminal;

                // Spawn terminal if not already running
                if self.terminal.is_none() {
                    match TerminalState::new(120, 40) {
                        Ok((state, rx)) => {
                            self.terminal = Some(Arc::new(Mutex::new(state)));
                            if let Some(conn) = self.connections.get_mut(idx) {
                                conn.connected = true;
                            }
                            tracing::info!("Terminal spawned");

                            // Return a Task that streams PTY output as messages
                            let stream = UnboundedReceiverStream::new(rx);
                            return Task::stream(stream).map(Message::PtyOutput);
                        }
                        Err(e) => {
                            tracing::error!("Failed to spawn terminal: {}", e);
                        }
                    }
                }
            }
            Message::ChangeView(view) => {
                self.active_view = view;
            }
            Message::PtyOutput(bytes) => {
                if let Some(term) = &self.terminal {
                    if let Ok(mut state) = term.lock() {
                        state.process(&bytes);
                    }
                }
            }
            Message::KeyboardEvent(event) => {
                if self.active_view == View::Terminal {
                    if let keyboard::Event::KeyPressed {
                        key,
                        modifiers,
                        text: text_opt,
                        ..
                    } = event
                    {
                        // Try named key first (Enter, Backspace, arrows, etc.)
                        let bytes = key_to_named_bytes(&key, &modifiers)
                            .or_else(|| {
                                // Then try the text produced by the keypress
                                if modifiers.control() {
                                    // Ctrl+key: compute control character
                                    ctrl_key_bytes(&key)
                                } else {
                                    text_opt.map(|t| t.as_bytes().to_vec())
                                }
                            });

                        if let Some(bytes) = bytes {
                            if let Some(term) = &self.terminal {
                                if let Ok(mut state) = term.lock() {
                                    state.write(&bytes);
                                }
                            }
                        }
                    }
                }
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        keyboard::listen().map(Message::KeyboardEvent)
    }

    pub fn view(&self) -> Element<'_, Message> {
        let sidebar = self.view_sidebar();
        let content = self.view_content();
        let status_bar = self.view_status_bar();

        let main_row = row![sidebar, content].height(Length::Fill);
        let layout = column![main_row, status_bar];

        container(layout)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(Background::Color(OryxisColors::BG_PRIMARY)),
                ..Default::default()
            })
            .into()
    }

    fn view_sidebar(&self) -> Element<'_, Message> {
        let header = container(
            text("ORYXIS").size(16).color(OryxisColors::ACCENT),
        )
        .padding(16)
        .width(Length::Fill);

        let search = container(
            text("Search... (Ctrl+K)")
                .size(13)
                .color(OryxisColors::TEXT_MUTED),
        )
        .padding(Padding {
            top: 4.0,
            right: 16.0,
            bottom: 4.0,
            left: 16.0,
        })
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(OryxisColors::BG_SURFACE)),
            border: Border {
                radius: Radius::from(6.0),
                ..Default::default()
            },
            ..Default::default()
        });

        let search_container = container(search).padding(Padding {
            top: 0.0,
            right: 12.0,
            bottom: 12.0,
            left: 12.0,
        });

        let mut sidebar_items: Vec<Element<'_, Message>> = vec![header.into(), search_container.into()];

        let mut current_group = String::new();
        for (idx, entry) in self.connections.iter().enumerate() {
            if entry.group != current_group {
                current_group.clone_from(&entry.group);
                let group_label = container(
                    text(format!("▸ {}", current_group))
                        .size(12)
                        .color(OryxisColors::TEXT_SECONDARY),
                )
                .padding(Padding {
                    top: 8.0,
                    right: 16.0,
                    bottom: 4.0,
                    left: 16.0,
                });
                sidebar_items.push(group_label.into());
            }

            let is_selected = self.selected == Some(idx);
            let bg = if is_selected {
                OryxisColors::BG_SELECTED
            } else {
                Color::TRANSPARENT
            };
            let fg = if is_selected {
                OryxisColors::TEXT_PRIMARY
            } else {
                OryxisColors::TEXT_SECONDARY
            };

            let status_dot = if entry.connected { "●" } else { "○" };
            let status_color = if entry.connected {
                OryxisColors::SUCCESS
            } else {
                OryxisColors::TEXT_MUTED
            };

            let entry_row = row![
                text(status_dot).size(10).color(status_color),
                Space::new().width(8),
                text(&entry.label).size(13).color(fg),
            ]
            .align_y(iced::Alignment::Center);

            let entry_btn = button(
                container(entry_row).padding(Padding {
                    top: 6.0,
                    right: 16.0,
                    bottom: 6.0,
                    left: 16.0,
                }),
            )
            .on_press(Message::SelectConnection(idx))
            .width(Length::Fill)
            .style(move |_theme, _status| button::Style {
                background: Some(Background::Color(bg)),
                text_color: fg,
                border: Border::default(),
                ..Default::default()
            });

            sidebar_items.push(entry_btn.into());
        }

        sidebar_items.push(Space::new().height(Length::Fill).into());

        let divider = container(Space::new().height(1))
            .width(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(Background::Color(OryxisColors::BORDER)),
                ..Default::default()
            });
        sidebar_items.push(divider.into());

        for (label, view) in [
            ("Keys", View::Keys),
            ("Snippets", View::Snippets),
            ("Settings", View::Settings),
        ] {
            let is_active = self.active_view == view;
            let fg = if is_active {
                OryxisColors::ACCENT
            } else {
                OryxisColors::TEXT_SECONDARY
            };

            let nav_btn = button(
                container(text(label).size(13).color(fg)).padding(Padding {
                    top: 6.0,
                    right: 16.0,
                    bottom: 6.0,
                    left: 16.0,
                }),
            )
            .on_press(Message::ChangeView(view))
            .width(Length::Fill)
            .style(|_theme, _status| button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                border: Border::default(),
                ..Default::default()
            });

            sidebar_items.push(nav_btn.into());
        }

        let sidebar_content =
            scrollable(column(sidebar_items).width(Length::Fill)).height(Length::Fill);

        container(sidebar_content)
            .width(220)
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(Background::Color(OryxisColors::BG_SIDEBAR)),
                border: Border {
                    color: OryxisColors::BORDER,
                    width: 0.0,
                    radius: Radius::from(0.0),
                },
                ..Default::default()
            })
            .into()
    }

    fn view_content(&self) -> Element<'_, Message> {
        let content: Element<'_, Message> = match self.active_view {
            View::Dashboard => self.view_dashboard(),
            View::Terminal => self.view_terminal(),
            View::Keys => self.view_placeholder("Keys"),
            View::Snippets => self.view_placeholder("Snippets"),
            View::Settings => self.view_placeholder("Settings"),
        };

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(Background::Color(OryxisColors::BG_PRIMARY)),
                ..Default::default()
            })
            .into()
    }

    fn view_dashboard(&self) -> Element<'_, Message> {
        let title = text("Welcome to Oryxis")
            .size(24)
            .color(OryxisColors::TEXT_PRIMARY);

        let subtitle = text("Select a connection from the sidebar to get started.")
            .size(14)
            .color(OryxisColors::TEXT_SECONDARY);

        let stats = text(format!("{} connections configured", self.connections.len()))
            .size(13)
            .color(OryxisColors::TEXT_MUTED);

        container(column![
            title,
            Space::new().height(8),
            subtitle,
            Space::new().height(16),
            stats
        ])
        .center(Length::Fill)
        .into()
    }

    fn view_terminal(&self) -> Element<'_, Message> {
        let label = self
            .selected
            .and_then(|i| self.connections.get(i))
            .map(|c| c.label.as_str())
            .unwrap_or("No selection");

        // Terminal canvas
        let terminal_area: Element<'_, Message> = if let Some(term_state) = &self.terminal {
            let view = TerminalView::new(Arc::clone(term_state));
            canvas(view)
                .width(Length::Fill)
                .height(Length::Fill)
                .into()
        } else {
            container(
                text("Connecting...")
                    .size(14)
                    .color(OryxisColors::TEXT_MUTED),
            )
            .center(Length::Fill)
            .into()
        };

        let terminal_container = container(terminal_area)
            .width(Length::Fill)
            .height(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(Background::Color(OryxisColors::TERMINAL_BG)),
                ..Default::default()
            });

        // Tab bar
        let tab = container(text(label).size(13).color(OryxisColors::TEXT_PRIMARY))
            .padding(Padding {
                top: 8.0,
                right: 16.0,
                bottom: 8.0,
                left: 16.0,
            })
            .style(|_theme| container::Style {
                background: Some(Background::Color(OryxisColors::BG_SURFACE)),
                border: Border {
                    radius: Radius {
                        top_left: 6.0,
                        top_right: 6.0,
                        bottom_right: 0.0,
                        bottom_left: 0.0,
                    },
                    ..Default::default()
                },
                ..Default::default()
            });

        let tab_bar = container(row![tab, Space::new().width(Length::Fill)])
            .width(Length::Fill)
            .style(|_theme| container::Style {
                background: Some(Background::Color(OryxisColors::BG_SIDEBAR)),
                ..Default::default()
            });

        column![tab_bar, terminal_container].into()
    }

    fn view_placeholder(&self, name: &str) -> Element<'_, Message> {
        container(
            text(format!("{} — coming soon", name))
                .size(16)
                .color(OryxisColors::TEXT_MUTED),
        )
        .center(Length::Fill)
        .into()
    }

    fn view_status_bar(&self) -> Element<'_, Message> {
        let status_text = if let Some(idx) = self.selected {
            let conn = &self.connections[idx];
            if conn.connected {
                format!("● {} — connected", conn.label)
            } else {
                format!("○ {} — disconnected", conn.label)
            }
        } else {
            "No active connection".into()
        };

        let status_color = if self.selected.map_or(false, |i| self.connections[i].connected) {
            OryxisColors::SUCCESS
        } else {
            OryxisColors::TEXT_MUTED
        };

        container(
            row![
                text(status_text).size(12).color(status_color),
                Space::new().width(Length::Fill),
                text("Oryxis v0.1.0").size(12).color(OryxisColors::TEXT_MUTED),
            ]
            .padding(Padding {
                top: 4.0,
                right: 12.0,
                bottom: 4.0,
                left: 12.0,
            }),
        )
        .width(Length::Fill)
        .style(|_theme| container::Style {
            background: Some(Background::Color(OryxisColors::BG_SIDEBAR)),
            border: Border {
                color: OryxisColors::BORDER,
                width: 1.0,
                radius: Radius::from(0.0),
            },
            ..Default::default()
        })
        .into()
    }
}

/// Convert named keys (Enter, Backspace, arrows, etc.) to terminal escape sequences.
fn key_to_named_bytes(key: &keyboard::Key, _modifiers: &keyboard::Modifiers) -> Option<Vec<u8>> {
    if let keyboard::Key::Named(named) = key {
        let bytes: &[u8] = match named {
            keyboard::key::Named::Enter => b"\r",
            keyboard::key::Named::Backspace => b"\x7f",
            keyboard::key::Named::Tab => b"\t",
            keyboard::key::Named::Escape => b"\x1b",
            keyboard::key::Named::ArrowUp => b"\x1b[A",
            keyboard::key::Named::ArrowDown => b"\x1b[B",
            keyboard::key::Named::ArrowRight => b"\x1b[C",
            keyboard::key::Named::ArrowLeft => b"\x1b[D",
            keyboard::key::Named::Home => b"\x1b[H",
            keyboard::key::Named::End => b"\x1b[F",
            keyboard::key::Named::PageUp => b"\x1b[5~",
            keyboard::key::Named::PageDown => b"\x1b[6~",
            keyboard::key::Named::Insert => b"\x1b[2~",
            keyboard::key::Named::Delete => b"\x1b[3~",
            keyboard::key::Named::F1 => b"\x1bOP",
            keyboard::key::Named::F2 => b"\x1bOQ",
            keyboard::key::Named::F3 => b"\x1bOR",
            keyboard::key::Named::F4 => b"\x1bOS",
            keyboard::key::Named::F5 => b"\x1b[15~",
            keyboard::key::Named::F6 => b"\x1b[17~",
            keyboard::key::Named::F7 => b"\x1b[18~",
            keyboard::key::Named::F8 => b"\x1b[19~",
            keyboard::key::Named::F9 => b"\x1b[20~",
            keyboard::key::Named::F10 => b"\x1b[21~",
            keyboard::key::Named::F11 => b"\x1b[23~",
            keyboard::key::Named::F12 => b"\x1b[24~",
            keyboard::key::Named::Space => b" ",
            _ => return None,
        };
        Some(bytes.to_vec())
    } else {
        None
    }
}

/// Convert Ctrl+key to control characters.
fn ctrl_key_bytes(key: &keyboard::Key) -> Option<Vec<u8>> {
    if let keyboard::Key::Character(c) = key {
        let ch = c.as_str().bytes().next()?;
        let ctrl = match ch {
            b'a'..=b'z' => ch - b'a' + 1,
            b'A'..=b'Z' => ch - b'A' + 1,
            b'[' => 27,
            b'\\' => 28,
            b']' => 29,
            b'^' => 30,
            b'_' => 31,
            _ => return None,
        };
        Some(vec![ctrl])
    } else {
        None
    }
}

