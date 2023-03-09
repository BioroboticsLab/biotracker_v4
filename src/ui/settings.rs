use super::{
    app::{BioTrackerUIComponents, BioTrackerUIContext},
    component_config::ConfigJson,
};
use crate::biotracker::protocol::*;

pub fn annotation_settings(ui: &mut egui::Ui, components: &mut BioTrackerUIComponents) {
    let video_view = &mut components.video_view;
    ui.label("Draw unmatched entity features");
    ui.checkbox(&mut video_view.draw_features, "");
    ui.end_row();
    ui.label("Draw matched entities");
    ui.checkbox(&mut video_view.draw_entities, "");
    ui.end_row();
    ui.label("Draw node labels");
    ui.checkbox(&mut video_view.draw_node_labels, "");
    ui.end_row();
    ui.label("Draw ID labels");
    ui.checkbox(&mut video_view.draw_ids, "");
    ui.end_row();
    ui.label("Draw Rectification");
    ui.checkbox(&mut video_view.draw_rectification, "");
    ui.end_row();
    ui.label("Draw Tracking Area");
    ui.checkbox(&mut video_view.draw_tracking_area, "");
    ui.end_row();
}

pub fn file_open_buttons(ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
    if ui.button("🎬").on_hover_text("Open video").clicked() {
        open_video(ctx);
    }
    if ui.button("🖭").on_hover_text("Load Track").clicked() {
        open_track(ctx);
    }
}

pub fn video_settings(ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
    let mut video_path = match ctx.experiment.video_info.as_ref() {
        Some(video_info) => video_info.path.clone(),
        None => String::from(""),
    };
    ui.label("Video Source");
    egui::TextEdit::singleline(&mut video_path)
        .hint_text("Select video source.")
        .show(ui);
    ui.horizontal(|ui| {
        file_open_buttons(ui, ctx);
    });
    ui.end_row();

    let (mut width, mut height) = if let Some(video_info) = ctx.experiment.video_info.as_ref() {
        (video_info.width, video_info.height)
    } else {
        (0, 0)
    };
    ui.add(egui::Label::new("Width"));
    ui.add(egui::DragValue::new(&mut width));
    ui.end_row();
    ui.add(egui::Label::new("Height"));
    ui.add(egui::DragValue::new(&mut height));
    ui.end_row();
    ui.add(egui::Label::new("Undistortion Mode"));

    egui::ComboBox::from_id_source("undistortion_mode")
        .selected_text(undistort_description(
            &UndistortMode::from_i32(ctx.experiment.undistort_mode).unwrap(),
        ))
        .show_ui(ui, |ui| {
            for mode in [
                UndistortMode::None,
                UndistortMode::Image,
                UndistortMode::Poses,
            ] {
                if ui
                    .selectable_value(
                        &mut ctx.experiment.undistort_mode,
                        mode as i32,
                        undistort_description(&mode),
                    )
                    .clicked()
                {
                    ctx.bt.command(Command::UndistortMode(mode as i32));
                }
            }
        });
    ui.add(egui::DragValue::new(&mut height));
    ui.end_row();
}

pub fn experiment_settings(ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
    let mut entity_count = ctx.experiment.entity_ids.len();
    ui.add(egui::Label::new("Entities"));
    if ui
        .add(egui::DragValue::new(&mut entity_count))
        .on_hover_text("Number of expected entities")
        .changed()
    {
        let difference = entity_count as i32 - ctx.experiment.entity_ids.len() as i32;
        if difference > 0 {
            for _ in 0..difference {
                ctx.bt.command(Command::AddEntity(Empty {}));
            }
        } else {
            for i in 0..difference.abs() {
                if i > ctx.experiment.entity_ids.len() as i32 {
                    break;
                }
                let id = ctx.experiment.entity_ids[i as usize];
                ctx.bt.command(Command::RemoveEntity(id));
            }
        }
    }
    ui.end_row();
    let mut fps = ctx.experiment.target_fps;
    ui.add(egui::Label::new("Target FPS")).changed();
    if ui.add(egui::DragValue::new(&mut fps)).changed() {
        ctx.bt.command(Command::TargetFps(fps as f32));
    }

    if ui
        .checkbox(&mut ctx.experiment.realtime_mode, "Realtime Tracking")
        .on_hover_text(
            "If checked, frames will be skipped if tracking is slower then the target FPS.",
        )
        .changed()
    {
        ctx.bt
            .command(Command::RealtimeMode(ctx.experiment.realtime_mode));
    }
    ui.end_row();
    ui.end_row();
}

pub fn arena_settings(ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
    let arena = ctx.experiment.arena.as_mut().unwrap();
    let mut send_update = false;
    ui.add(egui::Label::new("Arena Width"));
    send_update |= ui
        .add(egui::DragValue::new(&mut arena.width_cm).suffix("cm"))
        .changed();
    ui.end_row();
    ui.add(egui::Label::new("Arena Height"));
    send_update |= ui
        .add(egui::DragValue::new(&mut arena.height_cm).suffix("cm"))
        .changed();
    ui.end_row();

    ui.add(egui::Label::new("Tracking Area Vertices"));
    let mut vertices = arena.tracking_area_corners.len();
    if ui.add(egui::DragValue::new(&mut vertices)).changed() {
        send_update = true;
        if vertices > arena.tracking_area_corners.len() {
            let new_points = vec![Point::default(); vertices - arena.tracking_area_corners.len()];
            arena.tracking_area_corners.extend(new_points);
        } else {
            arena.tracking_area_corners.truncate(vertices);
        }
    }
    ui.end_row();

    if send_update {
        ctx.bt.command(Command::UpdateArena(arena.clone()));
    }
}

pub fn settings_window(
    ui: &mut egui::Ui,
    ctx: &mut BioTrackerUIContext,
    components: &mut BioTrackerUIComponents,
) {
    let mut open = ctx.experiment_setup_open;
    egui::Window::new("Settings")
        .resizable(false)
        .open(&mut open)
        .show(ui.ctx(), |ui| {
            egui::Grid::new("experiment_setup").show(ui, |ui| {
                ui.heading("Experiment");
                ui.separator();
                ui.end_row();
                experiment_settings(ui, ctx);

                ui.heading("Arena");
                ui.separator();
                ui.end_row();
                arena_settings(ui, ctx);

                ui.heading("Video Source");
                ui.separator();
                ui.end_row();
                video_settings(ui, ctx);

                ui.heading("Annotations");
                ui.separator();
                ui.end_row();
                annotation_settings(ui, components);

                for component in ctx.experiment.components.iter_mut() {
                    if component.id != "HungarianMatcher" {
                        continue;
                    }
                    ui.heading(&component.id);
                    ui.separator();
                    ui.end_row();
                    if ConfigJson::new()
                        .show(ui, &mut component.config_json)
                        .changed()
                    {
                        ctx.bt.command(Command::UpdateComponent(component.clone()));
                    }
                }
            });
        });
    ctx.experiment_setup_open = open;
}

pub fn filemenu() -> Option<String> {
    match rfd::FileDialog::new().pick_file() {
        Some(pathbuf) => Some(
            pathbuf
                .to_str()
                .ok_or(anyhow::anyhow!("Failed to get string from pathbuf"))
                .unwrap()
                .to_owned(),
        ),
        None => None,
    }
}

pub fn open_track(ctx: &mut BioTrackerUIContext) {
    if let Some(path) = filemenu() {
        ctx.bt.command(Command::OpenTrack(path));
    }
}

pub fn open_video(ctx: &mut BioTrackerUIContext) {
    if let Some(path) = filemenu() {
        ctx.bt.command(Command::OpenVideo(path.to_owned()));
    }
}

fn undistort_description(mode: &UndistortMode) -> &str {
    match mode {
        UndistortMode::None => "No undistortion",
        UndistortMode::Image => "Undistort Image",
        UndistortMode::Poses => "Undistort Poses",
    }
}
