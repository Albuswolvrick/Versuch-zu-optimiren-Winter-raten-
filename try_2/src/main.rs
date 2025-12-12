// Cargo.toml dependencies needed:
// [dependencies]
// eframe = "0.24"
// egui = "0.24"
// rusqlite = { version = "0.30", features = ["bundled"] }
// rand = "0.8"
// image = "0.24"
// simple_excel_writer = "0.2"

//mod other .rs file ; //this is for when ading a newe rs file


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
    number: String,
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
                number TEXT NOT NULL
            )",
            [],
        )?;
        Ok(Database { conn })
    }

    fn insert_user(&self, first: &str, surname: &str, email: &str, number: &str) -> SqlResult<()> {
        self.conn.execute(
            "INSERT INTO users (first_name, surname, email, number) VALUES (?1, ?2, ?3, ?4)",
            [first, surname, email, number],
        )?;
        Ok(())
    }

    // NEW: Function to retrieve all users from database
    fn get_all_users(&self) -> SqlResult<Vec<User>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, first_name, surname, email, number FROM users ORDER BY id"
        )?;

        let users = stmt.query_map([], |row| {
            Ok(User {
                id: row.get(0)?,
                first_name: row.get(1)?,
                surname: row.get(2)?,
                email: row.get(3)?,
                number: row.get(4)?,
            })
        })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(users)
    }
}

struct DevWindow {
    open: bool,
    max_number: String,
}

struct MyApp {
    first_name: String,
    surname: String,
    email: String,
    number: String,
    snowflakes: Vec<Snowflake>,
    database: Arc<Mutex<Database>>,
    dev_window: DevWindow,
    message: String,
    background_texture: Option<egui::TextureHandle>,
    export_message: String,
}

impl MyApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut rng = rand::thread_rng();
        let snowflakes: Vec<Snowflake> = (0..500)
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
                max_number: "100".to_string(),
            },
            message: String::new(),
            background_texture,
            export_message: String::new(),
        }
    }

    fn load_background_image(ctx: &egui::Context) -> Option<egui::TextureHandle> {
        let img_path = std::path::Path::new("/img/p4.jpg");

        if let Ok(img) = image::open(img_path) {
            let img_buffer = img.to_rgba8();
            let size = [img_buffer.width() as usize, img_buffer.height() as usize];
            let pixels = img_buffer.as_flat_samples();

            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                size,
                pixels.as_slice(),
            );

            Some(ctx.load_texture(
                "background",
                color_image,
                egui::TextureOptions::LINEAR,
            ))
        } else {
            eprintln!("Warning: Could not load background image from /img/p4.jpg");
            None
        }
    }

    // NEW: Function to export data to Excel
    fn export_to_excel(&self) -> Result<String, String> {
        // Get all users from database
        let db = self.database.lock().unwrap();
        let users = db.get_all_users()
            .map_err(|e| format!("Database error: {}", e))?;

        if users.is_empty() {
            return Err("No data to export!".to_string());
        }

        // Generate filename with timestamp
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let filename = format!("registrations_{}.xlsx", timestamp);

        // Create a new workbook
        let mut workbook = Workbook::create(&filename);
        let mut sheet = workbook.create_sheet("Registrations");

        // Set column widths
        sheet.add_column(Column { width: 8.0 });   // ID
        sheet.add_column(Column { width: 15.0 });  // First Name
        sheet.add_column(Column { width: 15.0 });  // Surname
        sheet.add_column(Column { width: 25.0 });  // Email
        sheet.add_column(Column { width: 12.0 });  // Number

        // Write header row with bold formatting
        workbook.write_sheet(&mut sheet, |sheet_writer| {
            let sw = sheet_writer;

            // Headers (row 0)
            sw.append_row(row![
               // "ID",
                "First Name",
                "Surname",
                "Email",
                "Number"
            ])?;

            // Write data rows
            for user in users.iter() {
                sw.append_row(row![
                  //  user.id,
                    user.first_name.clone(),
                    user.surname.clone(),
                    user.email.clone(),
                    user.number.clone()
                ])?;
            }

            Ok(())
        }).map_err(|e| format!("Write error: {:?}", e))?;

        // Close and save
        workbook.close().map_err(|e| format!("Save error: {:?}", e))?;

        Ok(format!("Exported {} users to {}", users.len(), filename))
    }
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let screen_rect = ctx.screen_rect();
        let _screen_height = screen_rect.height();

        // Update snowflakes
        for flake in &mut self.snowflakes {
            flake.y += flake.speed;
            if flake.y > 1.1 {
                flake.y = -0.1;
                flake.x = rand::thread_rng().gen_range(0.0..1.0);
            }
        }

        ctx.request_repaint();

        // Dev window toggle with Ctrl+shift+D
        if ctx.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.ctrl) {
            self.dev_window.open = !self.dev_window.open;
        }

        // Developer window
        if self.dev_window.open {
            let mut dev_open = self.dev_window.open;
            egui::Window::new("Developer Settings")
                .open(&mut dev_open)
                .show(ctx, |ui| {
                    ui.label("set Number:");
                    ui.text_edit_singleline(&mut self.dev_window.max_number);
                    //  ui.label("Press Ctrl+D to toggle this window");
                    ui.add_space(10.0);
                    ui.separator();

                    // NEW: Export button in dev window
                    if ui.button("📊 Export All Data to Excel").clicked() {
                        match self.export_to_excel() {
                            Ok(msg) => self.export_message = msg,
                            Err(e) => self.export_message = format!("Error: {}", e),
                        }
                    }

                    if !self.export_message.is_empty() {
                        ui.add_space(5.0);
                        ui.colored_label(
                            if self.export_message.contains("Exported") {
                                egui::Color32::GREEN
                            } else {
                                egui::Color32::RED
                            },
                            &self.export_message,
                        );
                    }

                    ui.add_space(10.0);
                    ui.separator();
                    ui.label("Developed by Pierre Maurice Hesse");
                });
            self.dev_window.open = dev_open;
        }

        // Main panel
        egui::CentralPanel::default().show(ctx, |ui| {
            let painter = ui.painter();
            let rect = ui.max_rect();

            // Draw background
            if let Some(texture) = &self.background_texture {
                painter.image(
                    texture.id(),
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            } else {
                painter.rect_filled(
                    rect,
                    0.0,
                    egui::Color32::from_rgb(15, 20, 35),
                );
            }

            // Draw snowflakes
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

            // Calculate responsive form dimensions
            let form_width = (rect.width() * 0.35).clamp(280.0, 400.0);
            let form_height = (rect.height() * 0.5).clamp(280.0, 350.0);

            // Registration form
            egui::Window::new("Winter Registration")
                .fixed_pos(egui::pos2(
                    rect.center().x - form_width / 2.0,
                    rect.center().y - form_height / 2.0,
                ))
                .fixed_size(egui::vec2(form_width, form_height))
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
                        } else if self.number.parse::<u32>().is_ok() && self.number.parse::<u32>().unwrap() >= 1 {
                            let db = self.database.lock().unwrap();
                            match db.insert_user(&self.first_name, &self.surname, &self.email, &self.number) {
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
                        // ui.small("Press Ctrl+D for dev settings");
                    });
                });
        });
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([640.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Snow Drift Registration - by Pierre Maurice Hesse",
        options,
        Box::new(|cc| Box::new(MyApp::new(cc))),
    )
}