use super::app::BioTrackerUIContext;

pub struct Annotator {
    stroke: egui::Stroke,
    annotations: Vec<Annotation>,
    annotation_builder: Option<AnnotationBuilder>,
}

enum ShapeType {
    Rectangle,
    RoundedRectangle,
    Arrow,
}

enum Shape {
    Arrow(Arrow),
    Rectangle(Rectangle),
}

struct Rectangle {
    min: egui::Pos2,
    max: egui::Pos2,
    rounding: f32,
}

struct Arrow {
    origin: egui::Pos2,
    dir: egui::Vec2,
}

struct Annotation {
    shape: Shape,
    stroke: egui::Stroke,
    first_frame: u32,
}

struct AnnotationBuilder {
    pub ty: ShapeType,
    start_frame: u32,
    drag_start: Option<egui::Pos2>,
    shape: Option<Shape>,
    stroke: egui::Stroke,
}

impl Default for Annotator {
    fn default() -> Self {
        Self {
            stroke: egui::Stroke::new(4.0, egui::Color32::BLUE),
            annotations: Vec::new(),
            annotation_builder: None,
        }
    }
}

impl Shape {
    fn draw(&self, painter: &egui::Painter, stroke: egui::Stroke) {
        match self {
            Shape::Rectangle(Rectangle { min, max, rounding }) => {
                painter.rect(
                    egui::Rect::from_min_max(*min, *max),
                    egui::Rounding::from(*rounding),
                    egui::Color32::TRANSPARENT,
                    stroke,
                );
            }
            Shape::Arrow(Arrow { origin, dir }) => {
                painter.arrow(*origin, *dir, stroke);
            }
        }
    }
}

impl Annotation {
    fn draw(&self, painter: &egui::Painter) {
        self.shape.draw(painter, self.stroke);
    }
}

impl AnnotationBuilder {
    fn new(start_frame: u32, ty: ShapeType, stroke: egui::Stroke) -> Self {
        Self {
            start_frame,
            drag_start: None,
            shape: None,
            stroke,
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

    fn arrow(&mut self, response: &egui::Response) -> bool {
        if response.dragged() {
            if self.drag_start.is_none() {
                self.drag_start = response.interact_pointer_pos();
            }
            if let (Some(drag_start), Some(mouse_pos)) =
                (self.drag_start, response.interact_pointer_pos())
            {
                self.shape = Some(Shape::Arrow(Arrow {
                    origin: drag_start,
                    dir: mouse_pos - drag_start,
                }));
            }
        }
        if response.drag_released() {
            return true;
        }
        false
    }

    fn update(&mut self, response: &egui::Response, painter: &egui::Painter) -> Option<Annotation> {
        let finalize = match self.ty {
            ShapeType::Rectangle => self.rectangle(response, 0.0),
            ShapeType::RoundedRectangle => self.rectangle(response, std::f32::INFINITY),
            ShapeType::Arrow => self.arrow(response),
        };

        if let Some(shape) = &self.shape {
            if finalize {
                return Some(Annotation {
                    shape: self.shape.take().unwrap(),
                    stroke: self.stroke,
                    first_frame: self.start_frame,
                });
            }
            shape.draw(painter, self.stroke);
        }
        return None;
    }
}

impl Annotator {
    pub fn show_onscreen(&mut self, ui: &mut egui::Ui, ctx: &mut BioTrackerUIContext) {
        egui::Window::new("Add Annotation")
            .resizable(false)
            .collapsible(false)
            .open(&mut ctx.annotator_open)
            .show(ui.ctx(), |ui| {
                ui.horizontal_top(|ui| {
                    egui::widgets::stroke_ui(ui, &mut self.stroke, "Style");
                    if ui.button("⭕").clicked() {
                        self.annotation_builder = Some(AnnotationBuilder::new(
                            ctx.current_frame_number,
                            ShapeType::RoundedRectangle,
                            self.stroke,
                        ));
                    }
                    if ui.button("□").clicked() {
                        self.annotation_builder = Some(AnnotationBuilder::new(
                            ctx.current_frame_number,
                            ShapeType::Rectangle,
                            self.stroke,
                        ));
                    }
                    if ui.button("↘").clicked() {
                        self.annotation_builder = Some(AnnotationBuilder::new(
                            ctx.current_frame_number,
                            ShapeType::Arrow,
                            self.stroke,
                        ));
                    }
                    if ui.button("💬").clicked() {
                        log::error!("Text annotation is not implemented");
                    }
                });
            });
    }

    pub fn show_offscreen(
        &mut self,
        response: &egui::Response,
        painter: &egui::Painter,
        ctx: &mut BioTrackerUIContext,
    ) {
        if let Some(annotation_builder) = &mut self.annotation_builder {
            if let Some(annotation) = annotation_builder.update(response, painter) {
                self.annotations.push(annotation);
                self.annotation_builder = None;
            }
        }

        for annotation in &self.annotations {
            if annotation.first_frame <= ctx.current_frame_number {
                annotation.draw(painter);
            }
        }
    }
}
