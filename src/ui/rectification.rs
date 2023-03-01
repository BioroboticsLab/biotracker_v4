use super::app::BioTrackerUIContext;
use crate::biotracker::protocol::*;

pub struct Rectification {
    control_points: Vec<egui::Pos2>,
    aux_stroke: egui::Stroke,
    drag_active: bool,
}

impl Default for Rectification {
    fn default() -> Self {
        Self {
            control_points: vec![egui::Pos2::new(0.0, 0.0); 4],
            aux_stroke: egui::Stroke::new(2.0, egui::Color32::RED.linear_multiply(0.25)),
            drag_active: false,
        }
    }
}

impl Rectification {
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        ctx: &mut BioTrackerUIContext,
    ) {
        if !self.drag_active {
            let arena = ctx.experiment.arena.as_ref().unwrap();
            for (i, p) in arena.rectification_corners.iter().enumerate() {
                self.control_points[i] = egui::Pos2::new(p.x as f32, p.y as f32);
            }
        }
        let to_screen = egui::emath::RectTransform::from_to(
            egui::Rect::from_min_size(egui::Pos2::ZERO, response.rect.size()),
            response.rect,
        );
        let control_point_radius = 8.0;
        let mut drag_finished = false;
        let control_point_shapes: Vec<egui::Shape> = self
            .control_points
            .iter_mut()
            .enumerate()
            .map(|(i, point)| {
                let size = egui::Vec2::splat(2.0 * control_point_radius);

                let point_in_screen = to_screen.transform_pos(*point);
                let point_rect = egui::Rect::from_center_size(point_in_screen, size);
                let point_id = response.id.with(i);
                let point_response = ui.interact(point_rect, point_id, egui::Sense::drag());

                *point += point_response.drag_delta();
                *point = to_screen.from().clamp(*point);
                let point_in_screen = to_screen.transform_pos(*point);
                let stroke = ui.style().interact(&point_response).fg_stroke;

                if point_response.drag_released() {
                    drag_finished = true;
                }
                if point_response.dragged() {
                    self.drag_active = true;
                }

                egui::Shape::circle_stroke(point_in_screen, control_point_radius, stroke)
            })
            .collect();
        let points_in_screen: Vec<egui::Pos2> =
            self.control_points.iter().map(|p| to_screen * *p).collect();
        painter.add(egui::epaint::PathShape::closed_line(
            points_in_screen,
            self.aux_stroke,
        ));
        painter.extend(control_point_shapes);

        if drag_finished {
            self.send_update(ctx);
            self.drag_active = false;
        }
    }

    fn send_update(&self, ctx: &mut BioTrackerUIContext) {
        let corners = self
            .control_points
            .iter()
            .map(|p| Point { x: p.x, y: p.y })
            .collect();
        ctx.bt
            .command(Command::UpdateArena(Arena {
                rectification_corners: corners,
                ..ctx.experiment.arena.clone().expect("Arena not set")
            }))
            .unwrap();
    }
}
