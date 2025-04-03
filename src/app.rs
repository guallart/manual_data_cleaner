use crate::inside_curve::check_inside_curve;
use chrono::{Duration, Local, NaiveDateTime};
use eframe::egui;
use eframe::egui::ecolor::Rgba;
use eframe::egui::plot::{Line, Plot, Points};
use eframe::egui::{Button, ComboBox, DragValue, TextEdit};
use itertools::izip;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

enum DataPoint {
    Valid(f64),
    NaN,
    Excluded(f64, String),
}

struct TimeSeries {
    name: String,
    data: Vec<DataPoint>,
}

fn unwrap_name(name: &str) -> Result<(String, String), String> {
    let names: Vec<&str> = name.split('~').collect();
    match names.len() {
        2 | 3 => Ok((names[0].to_string(), names[1].to_string())),
        _ => Err("Unsupported number of names".to_string()),
    }
}

pub struct ManualDataCleanerApp {
    msg: String,
    xaxis: usize,
    yaxis: usize,
    excludex: bool,
    excludey: bool,
    file_path: String,
    file_loaded: bool,
    timeseries: Vec<TimeSeries>,
    nan: f64,
    index: Vec<String>,
    reason: String,
    exclusion_names: Vec<String>,
    time_buffer: u64,
    exclusion_curve: Vec<[f64; 2]>,
    exclusion_curve_is_closed: bool,
    show_excluded: bool,
}

impl Default for ManualDataCleanerApp {
    fn default() -> Self {
        Self {
            msg: "".to_owned(),
            xaxis: 0,
            yaxis: 0,
            excludex: true,
            excludey: true,
            file_path: "".to_owned(),
            file_loaded: false,
            timeseries: Vec::new(),
            nan: 99999.0,
            index: Vec::new(),
            reason: "".to_owned(),
            exclusion_names: Vec::new(),
            time_buffer: 10,
            exclusion_curve: Vec::new(),
            exclusion_curve_is_closed: false,
            show_excluded: false,
        }
    }
}

impl ManualDataCleanerApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Default::default()
    }

    fn parse_data_file(&mut self) -> Result<(), String> {
        // Clear existing data
        self.index.clear();
        self.timeseries.clear();

        // Read file content
        let content =
            fs::read_to_string(&self.file_path).map_err(|e| format!("File read error: {}", e))?;

        let mut lines = content.lines();

        // Parse headers
        let headers = lines
            .next()
            .ok_or("Empty file")?
            .split('\t')
            .collect::<Vec<&str>>();

        // Handle first column as index
        self.index = match headers.first() {
            Some(&_name) => Vec::new(),
            None => return Err("No headers found".into()),
        };

        // Create TimeSeries for remaining columns
        self.timeseries = headers
            .iter()
            .skip(1)
            .map(|&h| TimeSeries {
                name: h.to_string(),
                data: Vec::new(),
            })
            .collect();

        // Parse data rows
        for (line_num, line) in lines.enumerate() {
            let values: Vec<&str> = line.split('\t').collect();

            // Store index value
            self.index.push(
                values
                    .first()
                    .ok_or(format!("Line {}: Missing index value", line_num + 2))?
                    .to_string(),
            );

            // Store timeseries values
            for (i, value) in values.iter().skip(1).enumerate() {
                if let Some(series) = self.timeseries.get_mut(i) {
                    let num = value.parse::<f64>().map_err(|_| {
                        format!(
                            "Line {}: Invalid numeric value '{}' in column '{}'",
                            line_num + 2,
                            value,
                            series.name
                        )
                    })?;

                    series.data.push(match num {
                        x if x.is_nan() || x == self.nan => DataPoint::NaN,
                        x => DataPoint::Valid(x),
                    });
                }
            }
        }

        Ok(())
    }

    fn process_points<F>(&self, handler: F) -> Vec<[f64; 2]>
    where
        F: Fn(&DataPoint, &DataPoint) -> Option<[f64; 2]>,
    {
        let x_series = &self.timeseries[self.xaxis];
        let y_series = &self.timeseries[self.yaxis];

        izip!(&x_series.data, &y_series.data)
            .filter_map(|(x, y)| handler(x, y))
            .collect()
    }

    fn convert_points(&self) -> Vec<[f64; 2]> {
        self.process_points(|x, y| {
            match (x, y) {
                (DataPoint::Valid(x_val), DataPoint::Valid(y_val)) => Some([*x_val, *y_val]),
                _ => Some([-100.0, -100.0]), // Default value for invalid
            }
        })
    }

    fn extract_valid_points(&self) -> Vec<[f64; 2]> {
        self.process_points(|x, y| {
            match (x, y) {
                (DataPoint::Valid(x_val), DataPoint::Valid(y_val)) => Some([*x_val, *y_val]),
                _ => None, // Filter out invalid
            }
        })
    }

    fn extract_excluded_points(&self) -> Vec<[f64; 2]> {
        self.process_points(|x, y| {
            match (x, y) {
                (DataPoint::Excluded(x_val, _), DataPoint::Excluded(y_val, _)) => {
                    Some([*x_val, *y_val])
                }
                _ => None, // Filter out invalid
            }
        })
    }

    fn export_exclusions(&self, path: PathBuf) -> std::io::Result<()> {
        let exclusions = self
            .timeseries
            .iter()
            .flat_map(|ts| {
                ts.data
                    .iter()
                    .zip(&self.index)
                    .filter_map(|(val, timestamp)| match val {
                        DataPoint::Excluded(_, reason) => {
                            Some(((*timestamp).clone(), (*reason).clone()))
                        }
                        _ => None,
                    })
                    .map(|(timestamp, reason)| {
                        let (mast, sensor) = unwrap_name(&ts.name).unwrap();
                        let time =
                            NaiveDateTime::parse_from_str(&timestamp, "%Y-%m-%d %H:%M").unwrap();
                        let time_ini = time - Duration::minutes(self.time_buffer as i64);
                        let time_end = time + Duration::minutes(self.time_buffer as i64);
                        (mast, sensor, reason, time_ini, time_end)
                    })
                    .collect::<Vec<(String, String, String, NaiveDateTime, NaiveDateTime)>>()
            })
            .collect::<Vec<(String, String, String, NaiveDateTime, NaiveDateTime)>>();

        let mut groups: HashMap<(&str, &str, &str), Vec<_>> = HashMap::new();
        for item in &exclusions {
            groups
                .entry((&item.0, &item.1, &item.2))
                .or_default()
                .push((item.3, item.4));
        }

        let mut merged = Vec::new();
        let now = Local::now().naive_local();
        for ((s1, s2, s3), mut ranges) in groups.into_iter() {
            ranges.sort_by_key(|(start, _)| *start);

            let (mut current_start, mut current_end) = ranges[0];

            for (start, end) in ranges.into_iter().skip(1) {
                if start <= current_end {
                    current_end = current_end.max(end);
                } else {
                    merged.push((
                        s1.to_string(),
                        s2.to_string(),
                        s3.to_string(),
                        current_start,
                        current_end,
                        now,
                    ));
                    current_start = start;
                    current_end = end;
                }
            }

            merged.push((
                s1.to_string(),
                s2.to_string(),
                s3.to_string(),
                current_start,
                current_end,
                now,
            ));
        }

        let file = File::create(Path::new(&path))?;
        let mut writer = BufWriter::new(file);
        let fmt = "%Y-%m-%d %H:%M:%S";
        for ex in merged.iter() {
            writeln!(
                writer,
                "{}\t{}\t{}\t{}\t{}\t{}",
                ex.0,
                ex.1,
                ex.2,
                ex.3.format(fmt),
                ex.4.format(fmt),
                ex.5.format(fmt)
            )?;
        }

        Ok(())
    }

    fn exclude_timeseries_data(&mut self, axis: usize, is_inside_curve: &[bool]) {
        self.timeseries[axis]
            .data
            .iter_mut()
            .zip(is_inside_curve.iter())
            .for_each(|(val, exclude)| {
                if *exclude {
                    match val {
                        DataPoint::Valid(v) => *val = DataPoint::Excluded(*v, self.reason.clone()),
                        _ => (),
                    }
                }
            });
    }

    fn exclude_data(&mut self) {
        if self.reason.is_empty() {
            self.msg = "Write a reason for exclusion".to_owned();
        } else if self.exclusion_curve.len() < 3 {
            self.msg = "At least 3 points are needed to define an exclusion area".to_owned();
        } else if !self.exclusion_curve_is_closed {
            self.msg = "The exclusion area must be closed".to_owned();
        } else {
            let curve = self.exclusion_curve.clone();
            let data = self.convert_points();
            let is_inside = check_inside_curve(curve, data);

            if !self.exclusion_names.contains(&self.reason) {
                self.exclusion_names.push(self.reason.clone());
            }

            if self.excludex {
                self.exclude_timeseries_data(self.xaxis, &is_inside);
            }

            if self.excludey {
                self.exclude_timeseries_data(self.yaxis, &is_inside);
            }

            self.exclusion_curve.clear();
            self.exclusion_curve_is_closed = false;
            self.msg = format!("Data excluded by '{}' reason", self.reason).to_owned();
        }
    }
}

impl eframe::App for ManualDataCleanerApp {
    fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
        eframe::egui::SidePanel::left("left_panel")
            .show_separator_line(true)
            .show(ctx, |ui| {
                eframe::egui::Grid::new("left_grid")
                    .striped(false)
                    .num_columns(3)
                    .spacing([10.0, 10.0])
                    .show(ui, |ui| {
                        ui.end_row();

                        ui.label("Missing value");
                        ui.add_sized([100., 20.], DragValue::new(&mut self.nan));
                        let load_button = ui.add_sized([100., 20.], Button::new("Load File"));
                        if load_button.clicked() {
                            if let Some(path) = rfd::FileDialog::new().pick_file() {
                                self.file_path = path.display().to_string();
                                match self.parse_data_file() {
                                    Ok(()) => {
                                        self.msg = "File loaded successfully".into();
                                        self.file_loaded = true;
                                    }
                                    Err(e) => self.msg = format!("Load error: {}", e),
                                }
                            } else {
                                self.msg = "No file selected.".into();
                            }
                        }

                        ui.end_row();

                        ui.label("Loaded file");
                        ui.label(if self.file_path.is_empty() {
                            "No file selected"
                        } else {
                            self.file_path
                                .split("\\")
                                .last()
                                .unwrap_or("No file selected")
                        });
                        ui.end_row();
                        ui.end_row();

                        let mut options: Vec<String> =
                            self.timeseries.iter().map(|ts| ts.name.clone()).collect();

                        if options.is_empty() {
                            options.push("".to_string());
                            options.push("".to_string());
                        } else if options.len() > 1 && !self.file_loaded {
                            self.xaxis = 0;
                            self.yaxis = 1;
                        }

                        ui.label("X-axis");
                        ComboBox::new("Select x axis", "")
                            .selected_text(&options[self.xaxis])
                            .show_ui(ui, |ui| {
                                for (index, option) in options.iter().enumerate() {
                                    if ui
                                        .selectable_value(&mut self.xaxis, index, option)
                                        .clicked()
                                    {
                                        self.xaxis = index;
                                    }
                                }
                            });

                        ui.checkbox(&mut self.excludex, "Exclude x axis");
                        ui.end_row();

                        ui.label("Y-axis");
                        ComboBox::new("Select y axis", "")
                            .selected_text(&options[self.yaxis])
                            .show_ui(ui, |ui| {
                                for (index, option) in options.iter().enumerate() {
                                    if ui
                                        .selectable_value(&mut self.yaxis, index, option)
                                        .clicked()
                                    {
                                        self.yaxis = index;
                                    }
                                }
                            });

                        ui.checkbox(&mut self.excludey, "Exclude y axis");
                        ui.end_row();
                        ui.end_row();

                        ui.label("Exclusion reason");
                        ui.add(
                            TextEdit::singleline(&mut self.reason)
                                .hint_text("Write the reason for exclusion")
                                .desired_width(300.0),
                        );

                        let exclude_button = ui.add_sized([100., 20.], Button::new("Exclude"));
                        if exclude_button.clicked() {
                            self.exclude_data();
                        }

                        ui.end_row();

                        ui.label(""); // dummy row
                        ui.checkbox(&mut self.show_excluded, "Show excluded data");
                        let clear_button =
                            ui.add_sized([100., 20.], Button::new("Clear selection"));
                        if clear_button.clicked() {
                            self.exclusion_curve.clear();
                            self.exclusion_curve_is_closed = false;
                        }
                        ui.end_row();
                        ui.end_row();

                        ui.label("Time buffer");
                        ui.add_sized(
                            [100., 20.],
                            DragValue::new(&mut self.time_buffer).suffix(" min"),
                        );
                        let export_button = ui.add_sized([100., 20.], Button::new("Export"));
                        if export_button.clicked() {
                            if let Some(path) = rfd::FileDialog::new().save_file() {
                                match self.export_exclusions(path) {
                                    Ok(()) => self.msg = "Exclusions exported successfully".into(),
                                    Err(e) => self.msg = format!("Export error: {}", e),
                                };
                            } else {
                                self.msg = "No file selected.".into();
                            }
                        }
                        ui.end_row();
                        ui.end_row();
                        ui.end_row();
                    });
                ui.add_space(50.0);
                ui.label(
                    egui::RichText::new(&self.msg).color(egui::Color32::from_rgb(255, 200, 200)),
                );
            });

        eframe::egui::CentralPanel::default().show(ctx, |ui| {
            if self.file_loaded {
                let points_valid = self.extract_valid_points();
                let points_excluded = self.extract_excluded_points();
                if !points_valid.is_empty() {
                Plot::new("data_plot")
                    .view_aspect(1.0)
                    .width(700.0)
                    .height(700.0)
                    .auto_bounds_x()
                    .auto_bounds_y()
                    .show(ui, |plot_ui| {
                        plot_ui.points(Points::new(points_valid).radius(2.0).color(Rgba::from_rgb(0.9, 0.9, 0.9)));

                        if self.show_excluded {
                            plot_ui.points(Points::new(points_excluded).radius(2.0).color(Rgba::from_rgb(0.9, 0.2, 0.2)));
                        }

                        let color = if self.exclusion_curve_is_closed {Rgba::GREEN} else {Rgba::RED};
                        plot_ui.points(Points::new(self.exclusion_curve.clone()).radius(5.0).color(color));
                        plot_ui.line(Line::new(self.exclusion_curve.clone())
                            .width(2.0)
                            .color(color));
                        
                        let ctx = plot_ui.ctx();
                        let input = ctx.input(|i| i.clone());
                        
                        if input.pointer.primary_clicked() && input.key_down(egui::Key::E) {

                            if let Some(click_pos) = input.pointer.interact_pos() {

                                let min_bounds = plot_ui.plot_bounds().min();
                                let max_bounds = plot_ui.plot_bounds().max();
                                let minx = min_bounds[0] as f64;
                                let miny = min_bounds[1] as f64;
                                let maxx = max_bounds[0] as f64;
                                let maxy = max_bounds[1] as f64;

                                let mut data_pos = plot_ui.transform().value_from_position(click_pos);
                                if data_pos.x > minx && data_pos.x < maxx && data_pos.y > miny && data_pos.y < maxy {
                                    if self.exclusion_curve.len() > 2 {
                                        let first_point = self.exclusion_curve.first().unwrap();
                                        let dist = ((data_pos.x - first_point[0]).powi(2) + (data_pos.y - first_point[1]).powi(2)).sqrt();
                                        if dist < 0.3 {
                                            self.exclusion_curve_is_closed = true;
                                            data_pos.x = first_point[0];
                                            data_pos.y = first_point[1];
                                        }
                                    }
                                    
                                    self.exclusion_curve.push([data_pos.x, data_pos.y]);
                                }
                            }
                        }
                    });
                }
            } else {
                ui.add_space(25.0);
                ui.label("<---\tLoad any timeseries file exported from WindFarmer: Analyst");
                ui.add_space(85.0);
                ui.label("<---\tSelect X and Y axis to plot");
                ui.add_space(50.0);
                ui.label("<---\tSelect some data over the plot with \"E+click\" , write a reason for the exclusion and click on Exclude");
                ui.add_space(70.0);
                ui.label("<---\tClick on Export to save the exclusions");
            }
        });
    }
}
