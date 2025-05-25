// src/main.rs
use eframe::egui;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

// Структура для хранения информации о видео
#[derive(Clone)]
struct VideoInfo {
    path: PathBuf,
    duration: Duration,
    width: u32,
    height: u32,
    fps: f64,
    has_audio: bool,
}

// Структура для представления клипа
#[derive(Clone)]
struct Clip {
    source_video: Arc<VideoInfo>,
    start_time: Duration,
    end_time: Duration,
    id: usize,
    position: f32, // Позиция на таймлайне
}

// Основное состояние приложения
struct VideoEditorApp {
    loaded_video: Option<Arc<VideoInfo>>,
    clips: Vec<Clip>,
    selected_clip: Option<usize>,
    timeline_zoom: f32,
    playhead_position: f32,
    preview_texture: Option<egui::TextureHandle>,
    next_clip_id: usize,
    dragging_clip: Option<usize>,
    drag_offset: f32,
    timeline_scroll: f32,
    is_playing: bool,
    last_frame_time: std::time::Instant,
}

impl Default for VideoEditorApp {
    fn default() -> Self {
        Self {
            loaded_video: None,
            clips: Vec::new(),
            selected_clip: None,
            timeline_zoom: 1.0,
            playhead_position: 0.0,
            preview_texture: None,
            next_clip_id: 0,
            dragging_clip: None,
            drag_offset: 0.0,
            timeline_scroll: 0.0,
            is_playing: false,
            last_frame_time: std::time::Instant::now(),
        }
    }
}

impl VideoEditorApp {
    fn load_video(&mut self, path: PathBuf) {
        // Используем простой парсер для получения базовой информации о MOV файле
        let video_info = self.parse_mov_file(&path);
        
        if let Some(info) = video_info {
            self.loaded_video = Some(Arc::new(info));
            
            // Создаем начальный клип со всем видео
            if let Some(video) = &self.loaded_video {
                let clip = Clip {
                    source_video: video.clone(),
                    start_time: Duration::from_secs(0),
                    end_time: video.duration,
                    id: self.next_clip_id,
                    position: 0.0,
                };
                self.next_clip_id += 1;
                self.clips.push(clip);
            }
        }
    }
    
    fn parse_mov_file(&self, path: &PathBuf) -> Option<VideoInfo> {
        // Простой парсер MOV файла для получения базовой информации
        // В реальном приложении здесь бы использовался полноценный парсер
        use std::fs::File;
        
        let file = File::open(path).ok()?;
        let metadata = file.metadata().ok()?;
        
        // Для демонстрации используем приблизительные значения
        // В реальности нужно парсить атомы MOV файла
        Some(VideoInfo {
            path: path.clone(),
            duration: Duration::from_secs(metadata.len() / 1_000_000), // Очень грубая оценка
            width: 1920,
            height: 1080,
            fps: 30.0,
            has_audio: true,
        })
    }
    
    fn split_clip(&mut self, clip_id: usize, split_time: Duration) {
        if let Some(clip_index) = self.clips.iter().position(|c| c.id == clip_id) {
            let original_clip = self.clips[clip_index].clone();
            
            if split_time > original_clip.start_time && split_time < original_clip.end_time {
                // Обновляем оригинальный клип
                self.clips[clip_index].end_time = split_time;
                
                // Создаем новый клип
                let new_clip = Clip {
                    source_video: original_clip.source_video.clone(),
                    start_time: split_time,
                    end_time: original_clip.end_time,
                    id: self.next_clip_id,
                    position: original_clip.position + (split_time - original_clip.start_time).as_secs_f32(),
                };
                self.next_clip_id += 1;
                
                // Вставляем новый клип после оригинального
                self.clips.insert(clip_index + 1, new_clip);
            }
        }
    }
    
    fn delete_selected_clip(&mut self) {
        if let Some(selected_id) = self.selected_clip {
            self.clips.retain(|c| c.id != selected_id);
            self.selected_clip = None;
        }
    }
    
    fn export_timeline(&self) {
        // Здесь будет логика экспорта
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("MOV файлы", &["mov"])
            .save_file()
        {
            println!("Экспорт в: {:?}", path);
            // Реальный экспорт потребует использования внешних инструментов
        }
    }
}

impl eframe::App for VideoEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Обновление позиции воспроизведения
        if self.is_playing {
            let now = std::time::Instant::now();
            let delta = now.duration_since(self.last_frame_time).as_secs_f32();
            self.last_frame_time = now;
            
            self.playhead_position += delta;
            
            // Проверяем, не достигли ли конца
            if let Some(video) = &self.loaded_video {
                if self.playhead_position >= video.duration.as_secs_f32() {
                    self.playhead_position = 0.0;
                    self.is_playing = false;
                }
            }
            
            ctx.request_repaint();
        }
        
        // Верхнее меню
        egui::TopBottomPanel::top("menu_bar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("📁 Открыть видео").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("MOV файлы", &["mov", "MOV"])
                        .pick_file()
                    {
                        self.load_video(path);
                    }
                }
                
                ui.separator();
                
                if ui.button("💾 Экспорт").clicked() {
                    self.export_timeline();
                }
                
                ui.separator();
                
                // Контролы воспроизведения
                if self.is_playing {
                    if ui.button("⏸ Пауза").clicked() {
                        self.is_playing = false;
                    }
                } else {
                    if ui.button("▶ Воспроизведение").clicked() {
                        self.is_playing = true;
                        self.last_frame_time = std::time::Instant::now();
                    }
                }
                
                if ui.button("⏹ Стоп").clicked() {
                    self.is_playing = false;
                    self.playhead_position = 0.0;
                }
                
                ui.separator();
                
                // Инструменты редактирования
                if ui.button("✂ Разрезать").on_hover_text("Shift+Click на клипе").clicked() {
                    // Разрезать в позиции playhead
                    if let Some(selected) = self.selected_clip {
                        let split_time = Duration::from_secs_f32(self.playhead_position);
                        self.split_clip(selected, split_time);
                    }
                }
                
                if ui.button("🗑 Удалить").clicked() {
                    self.delete_selected_clip();
                }
            });
        });
        
        // Панель предпросмотра
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Предпросмотр");
            
            // Область для отображения видео
            let available_size = ui.available_size();
            let preview_height = available_size.y * 0.5;
            
            ui.allocate_ui_with_layout(
                egui::vec2(available_size.x, preview_height),
                egui::Layout::centered_and_justified(egui::Direction::TopDown),
                |ui| {
                    ui.group(|ui| {
                        if self.loaded_video.is_some() {
                            // Здесь будет отображаться кадр видео
                            let rect = ui.available_rect_before_wrap();
                            ui.painter().rect_filled(
                                rect,
                                5.0,
                                egui::Color32::from_rgb(20, 20, 20),
                            );
                            
                            // Отображаем информацию о видео
                            if let Some(video) = &self.loaded_video {
                                // Симуляция видео кадра
                                let frame_color = egui::Color32::from_rgb(
                                    (self.playhead_position * 50.0).sin().abs() as u8 + 50,
                                    (self.playhead_position * 30.0).cos().abs() as u8 + 50,
                                    100,
                                );
                                
                                let video_rect = egui::Rect::from_center_size(
                                    rect.center(),
                                    egui::vec2(rect.width() * 0.8, rect.height() * 0.8),
                                );
                                
                                ui.painter().rect_filled(
                                    video_rect,
                                    5.0,
                                    frame_color,
                                );
                                
                                let text = format!(
                                    "{}x{} @ {:.1} fps\nВремя: {:.1}s",
                                    video.width, video.height, video.fps, self.playhead_position
                                );
                                ui.painter().text(
                                    rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    text,
                                    egui::FontId::proportional(16.0),
                                    egui::Color32::WHITE,
                                );
                            }
                        } else {
                            ui.label("Перетащите MOV файл или нажмите 'Открыть видео'");
                        }
                    });
                },
            );
            
            ui.separator();
            
            // Временная шкала
            ui.heading("Временная шкала");
            
            // Контролы масштабирования
            ui.horizontal(|ui| {
                ui.label("Масштаб:");
                if ui.button("−").clicked() {
                    self.timeline_zoom = (self.timeline_zoom * 0.8).max(0.1);
                }
                ui.label(format!("{:.0}%", self.timeline_zoom * 100.0));
                if ui.button("+").clicked() {
                    self.timeline_zoom = (self.timeline_zoom * 1.2).min(5.0);
                }
                
                ui.separator();
                
                if let Some(video) = &self.loaded_video {
                    ui.label(format!(
                        "Длительность: {:.1}s | Позиция: {:.1}s",
                        video.duration.as_secs_f32(),
                        self.playhead_position
                    ));
                }
            });
            
            ui.separator();
            
            let timeline_height = available_size.y * 0.35;
            
            // Временная шкала с клипами
            egui::ScrollArea::horizontal()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    let timeline_width = if let Some(video) = &self.loaded_video {
                        (video.duration.as_secs_f32() * 100.0 * self.timeline_zoom).max(available_size.x)
                    } else {
                        available_size.x
                    };
                    
                    let track_height = 80.0;
                    
                    // Видео дорожка
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("📹 Видео");
                            ui.separator();
                            
                            let (response, painter) = ui.allocate_painter(
                                egui::vec2(timeline_width, track_height),
                                egui::Sense::click_and_drag(),
                            );
                            
                            let rect = response.rect;
                            
                            // Фон дорожки
                            painter.rect_filled(
                                rect,
                                5.0,
                                egui::Color32::from_rgb(35, 35, 35),
                            );
                            
                            // Временная сетка
                            let seconds_per_pixel = 1.0 / (100.0 * self.timeline_zoom);
                            let grid_spacing = if self.timeline_zoom > 2.0 { 1.0 } else if self.timeline_zoom > 0.5 { 5.0 } else { 10.0 };
                            
                            for i in 0..((timeline_width * seconds_per_pixel / grid_spacing) as usize + 1) {
                                let x = rect.left() + (i as f32 * grid_spacing / seconds_per_pixel);
                                painter.line_segment(
                                    [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                                    egui::Stroke::new(1.0, egui::Color32::from_rgb(50, 50, 50)),
                                );
                                
                                // Метки времени
                                if i % 5 == 0 {
                                    painter.text(
                                        egui::pos2(x + 2.0, rect.top() + 2.0),
                                        egui::Align2::LEFT_TOP,
                                        format!("{}s", i as f32 * grid_spacing),
                                        egui::FontId::proportional(10.0),
                                        egui::Color32::from_rgb(150, 150, 150),
                                    );
                                }
                            }
                            
                            // Отрисовка клипов
                            for clip in &self.clips {
                                let start_x = rect.left() + clip.position * 100.0 * self.timeline_zoom;
                                let duration = (clip.end_time - clip.start_time).as_secs_f32();
                                let width = duration * 100.0 * self.timeline_zoom;
                                
                                let clip_rect = egui::Rect::from_min_size(
                                    egui::pos2(start_x, rect.top() + 5.0),
                                    egui::vec2(width, track_height - 10.0),
                                );
                                
                                let color = if Some(clip.id) == self.selected_clip {
                                    egui::Color32::from_rgb(120, 170, 220)
                                } else {
                                    egui::Color32::from_rgb(80, 120, 160)
                                };
                                
                                painter.rect_filled(clip_rect, 5.0, color);
                                
                                // Название клипа
                                painter.text(
                                    clip_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    format!("Клип {}", clip.id + 1),
                                    egui::FontId::proportional(12.0),
                                    egui::Color32::WHITE,
                                );
                                
                                // Длительность клипа
                                painter.text(
                                    egui::pos2(clip_rect.left() + 5.0, clip_rect.bottom() - 15.0),
                                    egui::Align2::LEFT_BOTTOM,
                                    format!("{:.1}s", duration),
                                    egui::FontId::proportional(10.0),
                                    egui::Color32::from_rgb(200, 200, 200),
                                );
                            }
                            
                            // Обработка кликов для выбора и разделения клипов
                            if response.clicked() {
                                if let Some(pos) = response.interact_pointer_pos() {
                                    let time_pos = (pos.x - rect.left()) / (100.0 * self.timeline_zoom);
                                    
                                    // Проверяем, попали ли в клип
                                    for clip in &self.clips {
                                        let clip_start = clip.position;
                                        let clip_duration = (clip.end_time - clip.start_time).as_secs_f32();
                                        let clip_end = clip_start + clip_duration;
                                        
                                        if time_pos >= clip_start && time_pos <= clip_end {
                                            if ui.input(|i| i.modifiers.shift) {
                                                // Shift+Click - разделить клип
                                                let split_time = clip.start_time + Duration::from_secs_f32(time_pos - clip_start);
                                                self.split_clip(clip.id, split_time);
                                            } else {
                                                // Обычный клик - выбрать клип
                                                self.selected_clip = Some(clip.id);
                                            }
                                            break;
                                        }
                                    }
                                }
                            }
                            
                            // Линия воспроизведения
                            let playhead_x = rect.left() + self.playhead_position * 100.0 * self.timeline_zoom;
                            painter.line_segment(
                                [
                                    egui::pos2(playhead_x, rect.top() - 5.0),
                                    egui::pos2(playhead_x, rect.bottom() + 5.0),
                                ],
                                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 100, 100)),
                            );
                            
                            // Треугольник над линией воспроизведения
                            let triangle = vec![
                                egui::pos2(playhead_x - 5.0, rect.top() - 5.0),
                                egui::pos2(playhead_x + 5.0, rect.top() - 5.0),
                                egui::pos2(playhead_x, rect.top()),
                            ];
                            painter.add(egui::Shape::convex_polygon(
                                triangle,
                                egui::Color32::from_rgb(255, 100, 100),
                                egui::Stroke::NONE,
                            ));
                        });
                    });
                    
                    ui.add_space(10.0);
                    
                    // Аудио дорожка
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label("🎵 Аудио");
                            ui.separator();
                            
                            let (_, painter) = ui.allocate_painter(
                                egui::vec2(timeline_width, track_height),
                                egui::Sense::hover(),
                            );
                            
                            let rect = painter.clip_rect();
                            
                            // Фон дорожки
                            painter.rect_filled(
                                rect,
                                5.0,
                                egui::Color32::from_rgb(35, 35, 35),
                            );
                            
                            // Отрисовка аудио волны для каждого клипа
                            for clip in &self.clips {
                                let start_x = rect.left() + clip.position * 100.0 * self.timeline_zoom;
                                let duration = (clip.end_time - clip.start_time).as_secs_f32();
                                let width = duration * 100.0 * self.timeline_zoom;
                                
                                let clip_rect = egui::Rect::from_min_size(
                                    egui::pos2(start_x, rect.top() + 5.0),
                                    egui::vec2(width, track_height - 10.0),
                                );
                                
                                // Фон аудио клипа
                                painter.rect_filled(
                                    clip_rect,
                                    5.0,
                                    egui::Color32::from_rgb(50, 80, 50),
                                );
                                
                                // Симуляция аудио волны
                                let wave_color = egui::Color32::from_rgb(100, 200, 100);
                                let center_y = clip_rect.center().y;
                                
                                for x in (start_x as i32..(start_x + width) as i32).step_by(3) {
                                    let t = (x as f32 - start_x) / width;
                                    let amplitude = ((t * 20.0).sin() * 0.5 + 0.5) * (clip_rect.height() * 0.4);
                                    
                                    painter.line_segment(
                                        [
                                            egui::pos2(x as f32, center_y - amplitude),
                                            egui::pos2(x as f32, center_y + amplitude),
                                        ],
                                        egui::Stroke::new(2.0, wave_color),
                                    );
                                }
                            }
                            
                            // Линия воспроизведения для аудио
                            let playhead_x = rect.left() + self.playhead_position * 100.0 * self.timeline_zoom;
                            painter.line_segment(
                                [
                                    egui::pos2(playhead_x, rect.top()),
                                    egui::pos2(playhead_x, rect.bottom()),
                                ],
                                egui::Stroke::new(2.0, egui::Color32::from_rgb(255, 100, 100)),
                            );
                        });
                    });
                });
        });
        
        // Обработка перетаскивания файлов
        if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
            let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
            for file in dropped_files {
                if let Some(path) = &file.path {
                    if path.extension().and_then(|s| s.to_str()).map(|s| s.to_lowercase()) == Some("mov".to_string()) {
                        self.load_video(path.clone());
                        break;
                    }
                }
            }
        }
    }
}

fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_drag_and_drop(true)
            .with_title("MOV Video Editor"),
        ..Default::default()
    };
    
    eframe::run_native(
        "MOV Video Editor",
        options,
        Box::new(|_cc| Ok(Box::new(VideoEditorApp::default()))),
    )
}