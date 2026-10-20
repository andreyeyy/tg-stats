mod statistics;
use statistics::*;
use std::sync::mpsc;
use std::thread;
use std::sync::mpsc::Receiver;

use egui_plot::Bar;
use egui_plot::BarChart;
use egui_plot::Plot;
use egui_plot::PlotConfig;
use egui_plot::GridMark;

use eframe::egui;

const WINDOW_TITLE: &str = "Telegram Statistics";

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

struct MainApp {
    screen: Screen,
    stats: Option<ChatStats>,
    receiver: Option<Receiver<Result<ChatStats, StatsError>>>,
    selected_tab: usize,
    selected_user: Option<String>, // ID of the selected user
    error: Option<StatsError>,
}

impl MainApp {

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
            for file in &i.raw.dropped_files {
                let path = file.path().to_path_buf();
                self.screen = Screen::Loading;

                let (tx, rx) = mpsc::channel::<Result<ChatStats, StatsError>>();
                self.receiver = Some(rx);

                thread::spawn(move || {
                    let result = statistics::generate(&path);
                    let _ = tx.send(result);
                });
            }
        });
    }
    fn statistics(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let stats: &ChatStats = self.stats.as_ref().expect("ChatStats is ready when in statistics menu");

        ui.horizontal(|ui| {
            for (index, title) in ["General", "Users"].iter().enumerate() {
                if ui.selectable_label(self.selected_tab == index, *title).clicked() {
                    self.selected_tab = index;
                }
            }
        });
        ui.separator();

        match self.selected_tab {
            0 => { // General tab
                //
                ui.heading(format!("Total messages: {}", stats.total_messages));

                let mut users_messages: Vec<(u64, String)> = stats.users
                    .values()
                    .map(|user| (user.total_messages, user.name.clone()))
                    .collect();
                users_messages.sort_by(|a, b| a.0.cmp(&b.0));

                let bars: Vec<Bar> = users_messages
                    .iter()
                    .enumerate()
                    .map(|(i, (count, _name))| Bar::new(i as f64, *count as f64))
                    .collect();

                Plot::new("messages_by_user")
                    .show_grid(false)
                    .show_crosshair(false)
                    .y_axis_formatter(move |mark: GridMark, _range| {
                        let i = mark.value.round() as usize;
                            users_messages.get(i).unwrap_or(&(0u64, String::new())).1.clone()
                    })
                    .show(ui, |plot_ui| {
                        plot_ui.bar_chart(BarChart::new("Messages", bars).horizontal());
                    });
            }
            1 => { // Users tab

                let selected_name = 
                if let Some(id) = &self.selected_user {
                    stats.users.get(id).unwrap().name.clone()
                }
                else {"Select User".to_string()};

                egui::ComboBox::from_label("")
                .selected_text(selected_name)
                .show_ui(ui, |ui| {
                    for (id, user) in &stats.users {
                        ui.selectable_value(
                            &mut self.selected_user,
                            Some(id.clone()),
                            &user.name,
                        );
                    }
                });
                if let Some(selected_user_id) = &self.selected_user {
                    let user: &User = &stats.users[selected_user_id];


                    ui.label(format!("Total messages: {}", user.total_messages));

                    let bars = user.day_time_distribution
                            .iter()
                            .enumerate()
                            .map(|(i, value)| Bar::new(i as f64, *value as f64))
                            .collect();

                    Plot::new("daily_distribution")
                        .show_crosshair(false)
                        .show_grid(false)
                        .show(ui, |plot_ui| {
                            plot_ui.bar_chart(BarChart::new("Daily message distribution", bars));
                        });
                }

            }
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
                            egui::vec2(1000.0, 700.0),
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

