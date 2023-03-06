use crate::biotracker::protocol::*;

pub struct Polygon {
    points: Vec<egui::Pos2>,
    drag_active: bool,
}

impl Polygon {
    pub fn new() -> Self {
        Self {
            points: vec![],
            drag_active: false,
        }
    }

    pub fn show(
        &mut self,
        id: egui::Id,
        ui: &mut egui::Ui,
        response: &egui::Response,
        painter: &egui::Painter,
        points: &Vec<Point>,
        stroke: &egui::Stroke,
    ) -> Option<Vec<Point>> {
        if !self.drag_active {
            self.points = points.iter().map(|p| egui::Pos2::new(p.x, p.y)).collect();
        }
        let to_screen = egui::emath::RectTransform::from_to(
            egui::Rect::from_min_size(egui::Pos2::ZERO, response.rect.size()),
            response.rect,
        );
        let control_point_radius = 8.0;
        let mut drag_finished = false;
        let control_point_shapes: Vec<egui::Shape> = self
            .points
            .iter_mut()
            .enumerate()
            .map(|(i, point)| {
                let size = egui::Vec2::splat(2.0 * control_point_radius);

                let point_in_screen = to_screen.transform_pos(*point);
                let point_rect = egui::Rect::from_center_size(point_in_screen, size);
                let point_id = id.with(i);
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
            self.points.iter().map(|p| to_screen * *p).collect();
        painter.add(egui::epaint::PathShape::closed_line(
            points_in_screen,
            *stroke,
        ));
        painter.extend(control_point_shapes);

        if drag_finished {
            self.drag_active = false;
            Some(
                self.points
                    .iter()
                    .map(|p| Point { x: p.x, y: p.y })
                    .collect(),
            )
        } else {
            None
        }
    }
}
