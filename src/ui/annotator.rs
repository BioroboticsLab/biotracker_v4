use super::{app::BioTrackerUIContext, entity_dropdown::EntityDropdown};

#[derive(Default)]
pub struct Annotator {
    selected_entity: EntityDropdown,
    annotations: Vec<Annotation>,
    annotation_builder: Option<AnnotationBuilder>,
}

enum ShapeType {
    Rectangle,
    RoundedRectangle,
    Arrow,
}

enum Shape {
    Rectangle(Rectangle),
    Arrow(Arrow),
}

struct Rectangle {
    min: egui::Pos2,
    max: egui::Pos2,
    rounding: f32,
}

struct Arrow {
    center: egui::Pos2,
    scale: f64,
}

struct Annotation {
    shape: Shape,
    first_frame: u32,
}

struct AnnotationBuilder {
    pub ty: ShapeType,
    start_frame: u32,
    drag_start: Option<egui::Pos2>,
    shape: Option<Shape>,
}

impl Shape {
    fn draw(&self, painter: &egui::Painter) {
        match self {
            Shape::Rectangle(Rectangle { min, max, rounding }) => {
                painter.rect(
                    egui::Rect::from_min_max(*min, *max),
                    egui::Rounding::from(*rounding),
                    egui::Color32::TRANSPARENT,
                    egui::Stroke::new(1.0, egui::Color32::WHITE),
                );
            }
            Shape::Arrow(Arrow { center, scale }) => {
                painter.arrow(
                    *center,
                    egui::vec2(0.0, -1.0) * (*scale as f32),
                    egui::Stroke::new(1.0, egui::Color32::WHITE),
                );
            }
        }
    }
}

impl AnnotationBuilder {
    fn new(start_frame: u32, ty: ShapeType) -> Self {
        Self {
            start_frame,
            drag_start: None,
            shape: None,
            ty,
        }
    }

    fn rectangle(&mut self, response: &egui::Response, rounding: f32) -> bool {
        if response.dragged() {
            if self.drag_start.is_none() {
                self.drag_start = response.interact_pointer_pos();
            }
            if let (Some(min), Some(max)) = (self.drag_start, response.interact_pointer_pos()) {
                self.shape = Some(Shape::Rectangle(Rectangle { min, max, rounding }));
            }
        }
        if response.drag_released() {
            return true;
        }
        false
    }

    fn arrow(&mut self, response: &egui::Response, painter: &egui::Painter) -> bool {
        if response.hovered() {
            self.shape = Some(Shape::Arrow(Arrow {
                center: painter.ctx().input().pointer.hover_pos().unwrap(),
                scale: 10.0,
            }));
        }
        if response.clicked() {
            return true;
        }
        false
    }

    fn update(&mut self, response: &egui::Response, painter: &egui::Painter) -> Option<Annotation> {
        let finalize = match self.ty {
            ShapeType::Rectangle => self.rectangle(response, 0.0),
            ShapeType::RoundedRectangle => self.rectangle(response, std::f32::INFINITY),
            ShapeType::Arrow => self.arrow(response, painter),
        };
        if finalize {
            return Some(Annotation {
                shape: self.shape.take().unwrap(),
                first_frame: self.start_frame,
            });
        } else {
            if let Some(shape) = &self.shape {
                shape.draw(painter);
            }
            return None;
        }
    }
}

impl Annotator {
    pub fn show(
        &mut self,
        response: &egui::Response,
        painter: &egui::Painter,
        ctx: &mut BioTrackerUIContext,
    ) {
        egui::Window::new("Add Annotation")
            .resizable(false)
            .collapsible(false)
            .open(&mut ctx.annotator_open)
            .show(painter.ctx(), |ui| {
                self.selected_entity
                    .show(ui, &ctx.experiment.entity_ids, "Attach to Entity");
                ui.horizontal_top(|ui| {
                    if ui.button("⭕").clicked() {
                        self.annotation_builder = Some(AnnotationBuilder::new(
                            ctx.current_frame_number,
                            ShapeType::RoundedRectangle,
                        ));
                    }
                    if ui.button("□").clicked() {
                        self.annotation_builder = Some(AnnotationBuilder::new(
                            ctx.current_frame_number,
                            ShapeType::Rectangle,
                        ));
                    }
                    if ui.button("↘").clicked() {
                        self.annotation_builder = Some(AnnotationBuilder::new(
                            ctx.current_frame_number,
                            ShapeType::Arrow,
                        ));
                    }
                    if ui.button("💬").clicked() {
                        todo!();
                    }
                });
            });

        if let Some(annotation_builder) = &mut self.annotation_builder {
            if let Some(annotation) = annotation_builder.update(response, painter) {
                self.annotations.push(annotation);
                self.annotation_builder = None;
            }
        }

        for annotation in &self.annotations {
            if annotation.first_frame <= ctx.current_frame_number {
                annotation.shape.draw(painter);
            }
        }
    }
}
