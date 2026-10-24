mod statistics;
use statistics::*;
use std::sync::mpsc;
use std::thread;
use std::sync::mpsc::Receiver;
use std::path::Path;

use egui_plot::Bar;
use egui_plot::BarChart;
use egui_plot::Plot;
use egui_plot::GridMark;

use eframe::egui;

use rfd::FileDialog;

const WINDOW_TITLE: &str = "Telegram Statistics";
const BASE_FONT_SIZE: f32 = 14.0;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_drag_and_drop(true),
        ..Default::default()
    };
    eframe::run_native(
        WINDOW_TITLE,
        options,
        Box::new(|_cc| Ok(Box::new(MainApp {
            screen: Screen::DropScreen,
            stats: None,
            receiver: None,
            selected_tab: 0,
            selected_user: None,
            error: None,
        }
        ))),
    )
}

enum Screen {
    DropScreen,
    Loading,
    Statistics,
    Error,
}

fn centered_text(ui: &mut egui::Ui, text: &str) {
    ui.vertical_centered(|ui| {
        ui.label(
            egui::RichText::new(text)
                .size(BASE_FONT_SIZE)
                .color(egui::Color32::WHITE)
        );
    });
}

fn text(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(BASE_FONT_SIZE)
            .color(egui::Color32::WHITE)
    );
}

/// Formats seconds to either hours or days
fn seconds_to_string(seconds: u64) -> String {
    if seconds < 3600 {
        format!("{} seconds", seconds)
    }
    else if seconds < 24 * 3600 {
        format!("{:.1} hours", seconds as f64 / 3600.0)
    }
    else {
        format!("{:.1} days", seconds as f64 / (24.0 * 3600.0))
    }
}

fn activity_over_time_graph(ui: &mut egui::Ui,
                            activity: &[u64; statistics::ALL_TIME_DISTRIBUTION_RESOLUTION],
                            seconds_per_bar: u64) {
    centered_text(ui, &format!("Activity Over Time (each bar represents {})", seconds_to_string(seconds_per_bar)));

    let all_time_distribution_bars = activity
            .iter()
            .enumerate()
            .map(|(i, value)| Bar::new(i as f64, *value as f64).width(1.0))
            .collect();

    Plot::new("alltime")
        .height(300f32)
        .allow_scroll(false)
        .show_crosshair(false)
        .show_grid(false)
        .show(ui, |plot_ui| {
            plot_ui.bar_chart(BarChart::new("alltime_barchart", all_time_distribution_bars));
        });

}

struct MainApp {
    screen: Screen,
    stats: Option<ChatStats>,
    receiver: Option<Receiver<Result<ChatStats, StatsError>>>,
    selected_tab: usize,
    selected_user: Option<String>, // ID of the selected user
    error: Option<StatsError>,
}

impl MainApp {

    fn load_result_json(&mut self, path: &Path) {
        let path = path.to_path_buf();
        self.screen = Screen::Loading;

        let (tx, rx) = mpsc::channel::<Result<ChatStats, StatsError>>();
        self.receiver = Some(rx);

        thread::spawn(move || {
            let result = statistics::generate(&path);
            let _ = tx.send(result);
        });

    }

    fn loading(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
       ui.vertical_centered(|ui| {
            let available_height = ui.available_height();
            let estimated_content_height = 60.0;
            
            if available_height > estimated_content_height {
                ui.add_space((available_height - estimated_content_height) / 2.0);
            }

            ui.spinner();
            ui.label("Generating…");
        });
    }

    fn drop_screen(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 2.0 - 40.0);
            ui.label(
                egui::RichText::new("Drop result.json here")
                    .size(28.0)
                    .color(egui::Color32::LIGHT_BLUE),
            );
            ui.label(
                egui::RichText::new("or click to browse")
                    .size(14.0)
                    .color(egui::Color32::LIGHT_BLUE),
            );
        });

        ui.input(|i| {
            if i.pointer.button_clicked(egui::PointerButton::Primary) {
                if let Some(path) = FileDialog::new().add_filter("json", &["json"]).pick_file(){
                    self.load_result_json(&path);
                }
            }
            if let Some(file) = &i.raw.dropped_files.iter().next() {
                let path = file.path().to_path_buf();
                self.load_result_json(&path);
            }
        });
    }

    fn general_tab(&mut self, ui: &mut egui::Ui) {
        let stats: &ChatStats = self.stats.as_ref().expect("ChatStats is available when in general tab");

        egui::ScrollArea::vertical().show(ui, |ui| {

            text(ui, &format!("Total messages: {}", stats.total_messages));

            let mut users_messages: Vec<(u64, String)> = stats.users
                .values()
                .map(|user| (user.total_messages, user.name.clone()))
                .collect();
            users_messages.sort_by(|a, b| a.0.cmp(&b.0));

            let users_messages_bars: Vec<Bar> = users_messages
                .iter()
                .enumerate()
                .map(|(i, (count, _name))| Bar::new(i as f64, *count as f64))
                .collect();

            let users_messages_plot_height: f32 = (650f32).min(40.0 * stats.users.len() as f32);

            centered_text(ui, "Message Count Distribution");
            Plot::new("messages_by_user")
                .show_grid(false)
                .height(users_messages_plot_height)
                .show_crosshair(false)
                .allow_scroll(false)
                .y_axis_formatter(move |mark: GridMark, _range| {
                    let i = mark.value.round() as usize;
                        users_messages.get(i).unwrap_or(&(0u64, String::new())).1.clone()
                })
                .show(ui, |plot_ui| {
                    plot_ui.bar_chart(BarChart::new("Messages", users_messages_bars).horizontal());
                });

            let seconds_per_bar = (stats.last_message_date - stats.first_message_date) / statistics::ALL_TIME_DISTRIBUTION_RESOLUTION as u64;
            activity_over_time_graph(ui, &stats.all_time_distribution, seconds_per_bar);
        });
    }

    fn users_tab(&mut self, ui: &mut egui::Ui) {
        let stats: &ChatStats = self.stats.as_ref().expect("ChatStats is available when in users tab");

        let selected_name = 
        if let Some(id) = &self.selected_user {
            stats.users.get(id).unwrap().name.clone()
        }
        else {"Select User".to_string()};

        // User selection box
        egui::ComboBox::from_label("")
        .selected_text(selected_name)
        .show_ui(ui, |ui| {
            let mut sorted_users: Vec<(&String, &User)> = stats.users.iter().collect();
            sorted_users.sort_unstable_by_key(|(_id, user)| user.total_messages);

            for (id, user) in sorted_users.iter().rev() {
                ui.selectable_value(
                    &mut self.selected_user,
                    Some(id.to_string()),
                    &user.name,
                );
            }
        });

        egui::ScrollArea::vertical().show(ui, |ui| {
        if let Some(selected_user_id) = &self.selected_user {
            let user: &User = &stats.users[selected_user_id];


            text(ui, &format!("Total messages: {} ({:.1}% of all messages)",
                    user.total_messages,
                    (user.total_messages as f64 / stats.total_messages as f64) * 100.0));

            centered_text(ui, "Hourly Message Distribution");

            let day_time_distribution_bars = user.day_time_distribution
                    .iter()
                    .enumerate()
                    .map(|(i, value)| Bar::new(i as f64, *value as f64))
                    .collect();

            Plot::new("daytime")
                .height(300f32)
                .allow_scroll(false)
                .show_crosshair(false)
                .show_grid(false)
                .show(ui, |plot_ui| {
                    plot_ui.bar_chart(BarChart::new("daytime_barchart", day_time_distribution_bars));
                });

            ui.add_space(20.0);

            let seconds_per_bar = (stats.last_message_date - stats.first_message_date) / statistics::ALL_TIME_DISTRIBUTION_RESOLUTION as u64;
            activity_over_time_graph(ui, &user.all_time_distribution, seconds_per_bar);
        }
        });
    }

    fn statistics(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.horizontal(|ui| {
            for (index, title) in ["General", "Users"].iter().enumerate() {
                if ui.selectable_label(self.selected_tab == index, *title).clicked() {
                    self.selected_tab = index;
                }
            }
        });
        ui.separator();

        match self.selected_tab {
            0 => self.general_tab(ui),
            1 => self.users_tab(ui),
            _ => {}
        }
    }

}

impl eframe::App for MainApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if let Some(rx) = &self.receiver {
            if let Ok(result) = rx.try_recv() {
                match result {
                    Ok(stats) => {
                        // println!("{:?}", stats);
                        self.stats = Some(stats);
                        self.screen = Screen::Statistics;
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::InnerSize(
                            egui::vec2(1000.0, 750.0),
                        )); 
                    },
                    Err(e) => {
                        self.error = Some(e);
                        self.screen = Screen::Error;
                    },
                }
                self.receiver = None;
            }
        }
        match self.screen {

            Screen::DropScreen  => self.drop_screen(ui, _frame),
            Screen::Loading     => self.loading(ui, _frame),
            Screen::Statistics  => self.statistics(ui, _frame),

            Screen::Error=> {
                ui.heading("Error");
            }
        }
    }
}

