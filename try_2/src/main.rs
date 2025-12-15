// Cargo.toml dependencies needed:
// [dependencies]
// eframe = "0.24"
// egui = "0.24"
// rusqlite = { version = "0.30", features = ["bundled"] }
// rand = "0.8"
// image = "0.24"
// simple_excel_writer = "0.2"

use eframe::egui;
use rusqlite::{Connection, Result as SqlResult};
use rand::Rng;
use std::sync::{Arc, Mutex};
use simple_excel_writer::*;

#[derive(Clone)]
struct Snowflake {
    x: f32,
    y: f32,
    speed: f32,
    size: f32,
}

#[derive(Debug, Clone)]
struct User {
    id: i32,
    first_name: String,
    surname: String,
    email: String,
    number: i32,
    winner: bool,
}

struct Database {
    conn: Connection,
}

impl Database {
    fn new() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute(
            "CREATE TABLE users (
                id INTEGER PRIMARY KEY,
                first_name TEXT NOT NULL,
                surname TEXT NOT NULL,
                email TEXT NOT NULL,
                number INTEGER NOT NULL,
                winner INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;
        Ok(Database { conn })
    }

    fn insert_user(&self, firstname: &str, surname: &str, email: &str, number: i32) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO users (first_name, surname, email, number, winner) VALUES (?1, ?2, ?3, ?4, 0)",
            [firstname, surname, email, &number.to_string()],
        )?;
        Ok(())
    }

    fn get_all_users(&self) -> SqlResult<Vec<User>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, first_name, surname, email, number, winner FROM users ORDER BY id"
        )?;

        let users = stmt.query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                first_name: row.get(1)?,
                surname: row.get(2)?,
                email: row.get(3)?,
                number: row.get(4)?,
                winner: row.get::<_, i32>(5)? == 1,
            })
        })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(users)
    }

    fn calculate_winners(&self, max_number: i32) -> SqlResult<()> {
        self.conn.execute("UPDATE users SET winner = 0", [])?;
        let users = self.get_all_users()?;

        if users.is_empty() {
            return Ok(());
        }

        let mut users_with_distance: Vec<_> = users.iter()
            .map(|u| (u.id, (u.number - max_number).abs()))
            .collect();

        users_with_distance.sort_by_key(|&(_, dist)| dist);

        let winner_count = users_with_distance.len().min(5);
        for i in 0..winner_count {
            let user_id = users_with_distance[i].0;
            self.conn.execute(
                "UPDATE users SET winner = 1 WHERE id = ?1",
                [user_id],
            )?;
        }

        Ok(())
    }

    fn get_sorted_users(&self, max_number: i32) -> SqlResult<Vec<User>> {
        let mut users = self.get_all_users()?;

        users.sort_by(|a, b| {
            match (b.winner, a.winner) {
                (true, false) => std::cmp::Ordering::Greater,
                (false, true) => std::cmp::Ordering::Less,
                _ => {
                    let dist_a = (a.number - max_number).abs();
                    let dist_b = (b.number - max_number).abs();
                    dist_a.cmp(&dist_b)
                }
            }
        });

        Ok(users)
    }
}

struct DevWindow {
    open: bool,
    max_number: String,
}

struct TableWindow {
    open: bool,
}

struct MyApp {
    first_name: String,
    surname: String,
    email: String,
    number: String,
    snowflakes: Vec<Snowflake>,
    database: Arc<Mutex<Database>>,
    dev_window: DevWindow,
    table_window: TableWindow,
    message: String,
    background_texture: Option<egui::TextureHandle>,
    export_message: String,
}

impl MyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut rng = rand::thread_rng();
        let snowflakes: Vec<Snowflake> = (0..300)
            .map(|_| Snowflake {
                x: rng.gen_range(0.0..1.0),
                y: rng.gen_range(-0.8..0.0),
                speed: rng.gen_range(0.001..0.003),
                size: rng.gen_range(2.0..5.0),
            })
            .collect();

        let background_texture = Self::load_background_image(&cc.egui_ctx);

        Self {
            first_name: String::new(),
            surname: String::new(),
            email: String::new(),
            number: String::new(),
            snowflakes,
            database: Arc::new(Mutex::new(Database::new().unwrap())),
            dev_window: DevWindow {
                open: false,
                max_number: "300".to_string(),
            },
            table_window: TableWindow {
                open: false,
            },
            message: String::new(),
            background_texture,
            export_message: String::new(),
        }
    }

    fn load_background_image(ctx: &egui::Context) -> Option<egui::TextureHandle> {
        let possible_paths = vec![
            "src/img/p4.jpg",
            "img/p4.jpg",
            "./img/p4.jpg",
            "../img/p4.jpg",
            "p4.jpg",
        ];

        for img_path_str in &possible_paths {
            let img_path = std::path::Path::new(img_path_str);

            if let Ok(img) = image::open(img_path) {
                let img_buffer = img.to_rgba8();
                let size = [img_buffer.width() as usize, img_buffer.height() as usize];
                let pixels = img_buffer.as_flat_samples();

                let color_image = egui::ColorImage::from_rgba_unmultiplied(
                    size,
                    pixels.as_slice(),
                );

                println!("Background image loaded from: {}", img_path_str);
                return Some(ctx.load_texture(
                    "background",
                    color_image,
                    egui::TextureOptions::LINEAR,
                ));
            }
        }

        eprintln!("Warning: Could not load background image.");
        None
    }

    fn export_to_excel(&self) -> Result<String, String> {
        let db = self.database.lock().unwrap();
        let users = db.get_all_users()
            .map_err(|e| format!("Database error: {}", e))?;

        if users.is_empty() {
            return Err("No data to export!".to_string());
        }

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let filename = format!("registrations_{}.xlsx", timestamp);

        let mut workbook = Workbook::create(&filename);
        let mut sheet = workbook.create_sheet("Registrations");

        sheet.add_column(Column { width: 8.0 });
        sheet.add_column(Column { width: 15.0 });
        sheet.add_column(Column { width: 15.0 });
        sheet.add_column(Column { width: 25.0 });
        sheet.add_column(Column { width: 12.0 });
        sheet.add_column(Column { width: 10.0 });

        workbook.write_sheet(&mut sheet, |sheet_writer| {
            let sw = sheet_writer;

            sw.append_row(row![
                "ID",
                "First Name",
                "Surname",
                "Email",
                "Number",
                "Winner"
            ])?;

            for user in users.iter() {
                sw.append_row(row![
                    user.id.to_string(),
                    user.first_name.clone(),
                    user.surname.clone(),
                    user.email.clone(),
                    user.number.to_string(),
                    if user.winner { "YES" } else { "NO" }
                ])?;
            }

            Ok(())
        }).map_err(|e| format!("Write error: {:?}", e))?;

        workbook.close().map_err(|e| format!("Save error: {:?}", e))?;

        Ok(format!("Exported {} users to {}", users.len(), filename))
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update snowflakes
        for flake in &mut self.snowflakes {
            flake.y += flake.speed;
            if flake.y > 1.1 {
                flake.y = -0.1;
                flake.x = rand::thread_rng().gen_range(0.0..1.0);
            }
        }

        ctx.request_repaint();

        // Dev window toggle mit Ctrl+Shift+D
        if ctx.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.ctrl && i.modifiers.shift) {
            self.dev_window.open = !self.dev_window.open;
        }

        // Table window toggle mit Ctrl+Windows+L
        if ctx.input(|i| i.key_pressed(egui::Key::L) && i.modifiers.ctrl && i.modifiers.command) {
            self.table_window.open = !self.table_window.open;
        }

        // Developer window
        if self.dev_window.open {
            let mut dev_open = self.dev_window.open;
            egui::Window::new("Developer Settings")
                .open(&mut dev_open)
                .default_width(400.0)
                .show(ctx, |ui| {
                    ui.label("Max Number (Zielzahl):");
                    ui.text_edit_singleline(&mut self.dev_window.max_number);

                    ui.add_space(10.0);

                    if ui.button("Calculate Winners (Top 5 closest)").clicked() {
                        if let Ok(max_num) = self.dev_window.max_number.parse::<i32>() {
                            let db = self.database.lock().unwrap();
                            match db.calculate_winners(max_num) {
                                Ok(_) => self.export_message = "Winners calculated successfully!".to_string(),
                                Err(e) => self.export_message = format!("Error: {}", e),
                            }
                        } else {
                            self.export_message = "Invalid max number!".to_string();
                        }
                    }

                    ui.add_space(10.0);
                    ui.separator();

                    if ui.button("Export All Data to Excel").clicked() {
                        match self.export_to_excel() {
                            Ok(msg) => self.export_message = msg,
                            Err(e) => self.export_message = format!("Error: {}", e),
                        }
                    }

                    if !self.export_message.is_empty() {
                        ui.add_space(5.0);
                        ui.colored_label(
                            if self.export_message.contains("success") || self.export_message.contains("Exported") {
                                egui::Color32::GREEN
                            } else {
                                egui::Color32::RED
                            },
                            &self.export_message,
                        );
                    }

                    ui.add_space(10.0);
                    ui.separator();
                    ui.label("Shortcuts:");
                    ui.small("Ctrl+Shift+D - Dev Settings");
                    ui.small("Ctrl+Win+L - Table View");
                    ui.add_space(5.0);
                    ui.label("Developed by Pierre Maurice Hesse");
                });
            self.dev_window.open = dev_open;
        }

        // Table window
        if self.table_window.open {
            let mut table_open = self.table_window.open;
            egui::Window::new("Registrations Table")
                .open(&mut table_open)
                .default_width(700.0)
                .default_height(500.0)
                .show(ctx, |ui| {
                    let db = self.database.lock().unwrap();
                    let max_num = self.dev_window.max_number.parse::<i32>().unwrap_or(300);

                    match db.get_sorted_users(max_num) {
                        Ok(users) => {
                            if users.is_empty() {
                                ui.label("No registrations yet.");
                            } else {
                                ui.label(format!("Total registrations: {} | Target number: {}", users.len(), max_num));
                                ui.add_space(5.0);

                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    ui.heading("Winners (Top 5 closest)");
                                    ui.separator();

                                    for (idx, user) in users.iter().enumerate() {
                                        let distance = (user.number - max_num).abs();
                                        let bg_color = if user.winner {
                                            egui::Color32::from_rgb(50, 100, 50)
                                        } else if idx % 2 == 0 {
                                            egui::Color32::from_rgb(30, 30, 35)
                                        } else {
                                            egui::Color32::from_rgb(25, 25, 30)
                                        };

                                        ui.horizontal(|ui| {
                                            let frame = egui::Frame::none().fill(bg_color).inner_margin(5.0);
                                            frame.show(ui, |ui| {
                                                ui.set_min_width(650.0);

                                                if user.winner {
                                                    ui.label(egui::RichText::new("[WINNER]").color(egui::Color32::GOLD).size(14.0));
                                                }

                                                ui.label(format!("ID: {}", user.id));
                                                ui.separator();
                                                ui.label(&user.first_name);
                                                ui.label(&user.surname);
                                                ui.separator();
                                                ui.label(&user.email);
                                                ui.separator();
                                                ui.label(format!("Number: {}", user.number));
                                                ui.separator();
                                                ui.colored_label(
                                                    if distance < 10 {
                                                        egui::Color32::GREEN
                                                    } else if distance < 50 {
                                                        egui::Color32::YELLOW
                                                    } else {
                                                        egui::Color32::GRAY
                                                    },
                                                    format!("Distance: {}", distance)
                                                );
                                            });
                                        });
                                        ui.add_space(2.0);
                                    }
                                });
                            }
                        }
                        Err(e) => {
                            ui.colored_label(egui::Color32::RED, format!("Error: {}", e));
                        }
                    }
                });
            self.table_window.open = table_open;
        }

        // Main panel - OHNE RAHMEN UND PADDING
        egui::CentralPanel::default()
            .frame(egui::Frame::none()) // Entfernt alle Rahmen und Padding
            .show(ctx, |ui| {
                let painter = ui.painter();
                let rect = ui.max_rect();

                // Hintergrundbild über den gesamten Bildschirm
                if let Some(texture) = &self.background_texture {
                    painter.image(
                        texture.id(),
                        rect,
                        egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                        egui::Color32::WHITE,
                    );
                } else {
                    // Fallback, falls das Bild nicht geladen werden kann
                    painter.rect_filled(
                        rect,
                        0.0,
                        egui::Color32::from_rgb(15, 20, 35),
                    );
                }

                // Schneeflocken über dem Hintergrund
                for flake in &self.snowflakes {
                    painter.circle_filled(
                        egui::pos2(
                            rect.left() + flake.x * rect.width(),
                            rect.top() + flake.y * rect.height(),
                        ),
                        flake.size,
                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 200),
                    );
                }

                let form_width = (rect.width() * 0.35).clamp(280.0, 400.0);
                let form_height = (rect.height() * 0.5).clamp(280.0, 350.0);

                // Registrierungsformular mit Transparenz
                egui::Window::new("Winter Registration")
                    .fixed_pos(egui::pos2(
                        rect.center().x - form_width / 2.0,
                        rect.center().y - form_height / 2.0,
                    ))
                    .fixed_size(egui::vec2(form_width, form_height))
                    .collapsible(false)
                    .frame(egui::Frame {
                        fill: egui::Color32::from_rgba_unmultiplied(30, 30, 35, 180), // Hier die Transparenz ändern (0-255)
                        rounding: egui::Rounding::same(10.0),
                        stroke: egui::Stroke::new(1.0, egui::Color32::from_rgba_unmultiplied(255, 255, 255, 40)),
                        inner_margin: egui::Margin::same(15.0),
                        ..Default::default()
                    })
                    .show(ctx, |ui| {
                        ui.vertical_centered(|ui| {
                            ui.heading("Register");
                            ui.add_space(10.0);
                        });

                        ui.label("First Name:");
                        ui.text_edit_singleline(&mut self.first_name);

                        ui.label("Surname:");
                        ui.text_edit_singleline(&mut self.surname);

                        ui.label("Email:");
                        ui.text_edit_singleline(&mut self.email);

                        ui.label("Number (1 to ∞):");
                        ui.text_edit_singleline(&mut self.number);

                        ui.add_space(10.0);

                        if ui.button("Submit").clicked() {
                            if self.first_name.is_empty() || self.surname.is_empty() ||
                                self.email.is_empty() || self.number.is_empty() {
                                self.message = "Please fill all fields!".to_string();
                            } else if let Ok(num) = self.number.parse::<i32>() {
                                if num >= 1 {
                                    let db = self.database.lock().unwrap();
                                    match db.insert_user(&self.first_name, &self.surname, &self.email, num) {
                                        Ok(_) => {
                                            self.message = "Registration successful!".to_string();
                                            self.first_name.clear();
                                            self.surname.clear();
                                            self.email.clear();
                                            self.number.clear();
                                        }
                                        Err(e) => self.message = format!("Error: {}", e),
                                    }
                                } else {
                                    self.message = "Number must be >= 1".to_string();
                                }
                            } else {
                                self.message = "Invalid number format!".to_string();
                            }
                        }

                        if !self.message.is_empty() {
                            ui.add_space(5.0);
                            ui.colored_label(
                                if self.message.contains("successful") {
                                    egui::Color32::GREEN
                                } else {
                                    egui::Color32::RED
                                },
                                &self.message,
                            );
                        }

                        ui.add_space(5.0);
                        ui.separator();
                        ui.vertical_centered(|ui| {
                            ui.small("Developed by Pierre Maurice Hesse");
                        });
                    });
            });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([640.0, 480.0])
            .with_decorations(true), // Fensterrahmen bleiben
        ..Default::default()
    };

    eframe::run_native(
        "Snow Drift Registration - by Pierre Maurice Hesse",
        options,
        Box::new(|cc| Box::new(MyApp::new(cc))),
    )
}