use super::app::BioTrackerUIContext;

pub struct Annotator {
    stroke: egui::Stroke,
    annotations: Vec<Annotation>,
    annotation_builder: Option<AnnotationBuilder>,
    text_buffer: String,
    font: egui::FontId,
}

enum Shape {
    Arrow(Arrow),
    Rectangle(Rectangle),
    Text(Text),
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

struct Text {
    origin: egui::Pos2,
    text: String,
    font: egui::FontId,
}

struct Annotation {
    shape: Shape,
    stroke: egui::Stroke,
    first_frame: u32,
}

struct AnnotationBuilder {
    start_frame: u32,
    drag_start: Option<egui::Pos2>,
    shape: Option<Shape>,
    stroke: egui::Stroke,
    ignore_first_update: bool,
}

impl Default for Annotator {
    fn default() -> Self {
        Self {
            stroke: egui::Stroke::new(4.0, egui::Color32::BLUE),
            annotations: Vec::new(),
            annotation_builder: None,
            text_buffer: String::new(),
            font: egui::FontId {
                size: 12.0,
                family: egui::FontFamily::Proportional,
            },
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
            Shape::Text(Text { origin, text, font }) => {
                painter.text(
                    *origin,
                    egui::Align2::CENTER_CENTER,
                    &text,
                    font.clone(),
                    stroke.color,
                );
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
    fn new(start_frame: u32, stroke: egui::Stroke) -> Self {
        Self {
            start_frame,
            drag_start: None,
            shape: None,
            stroke,
            ignore_first_update: true,
        }
    }

    fn rectangle(mut self, rounding: f32) -> Self {
        self.shape = Some(Shape::Rectangle(Rectangle {
            min: egui::Pos2::ZERO,
            max: egui::Pos2::ZERO,
            rounding,
        }));
        self
    }

    fn arrow(mut self) -> Self {
        self.shape = Some(Shape::Arrow(Arrow {
            origin: egui::Pos2::ZERO,
            dir: egui::Vec2::ZERO,
        }));
        self
    }

    fn text(mut self, text: String, font: egui::FontId) -> Self {
        self.shape = Some(Shape::Text(Text {
            origin: egui::Pos2::ZERO,
            text,
            font,
        }));
        self
    }

    fn update_rectangle(&mut self, response: &egui::Response) -> bool {
        let rectangle = match self.shape.as_mut() {
            Some(Shape::Rectangle(r)) => r,
            _ => return false,
        };
        if response.dragged() {
            if self.drag_start.is_none() {
                self.drag_start = response.interact_pointer_pos();
            }
            if let (Some(min), Some(max)) = (self.drag_start, response.interact_pointer_pos()) {
                rectangle.min = min;
                rectangle.max = max;
            }
        }
        if response.drag_released() {
            return true;
        }
        false
    }

    fn update_arrow(&mut self, response: &egui::Response) -> bool {
        let arrow = match self.shape.as_mut() {
            Some(Shape::Arrow(a)) => a,
            _ => return false,
        };
        if response.dragged() {
            if self.drag_start.is_none() {
                self.drag_start = response.interact_pointer_pos();
            }
            if let (Some(drag_start), Some(mouse_pos)) =
                (self.drag_start, response.interact_pointer_pos())
            {
                arrow.origin = drag_start;
                arrow.dir = mouse_pos - drag_start;
            }
        }
        if response.drag_released() {
            return true;
        }
        false
    }

    fn update_text(&mut self, response: &egui::Response) -> bool {
        let text = match self.shape.as_mut() {
            Some(Shape::Text(t)) => t,
            _ => return false,
        };
        if let Some(mouse_pos) = response.hover_pos() {
            text.origin = mouse_pos;
        }
        if response.clicked() {
            return true;
        }
        false
    }

    fn update(&mut self, response: &egui::Response, painter: &egui::Painter) -> Option<Annotation> {
        if self.ignore_first_update {
            self.ignore_first_update = false;
            return None;
        }
        let finalize = match self.shape.as_ref() {
            Some(Shape::Rectangle(_)) => self.update_rectangle(response),
            Some(Shape::Arrow(_)) => self.update_arrow(response),
            Some(Shape::Text(_)) => self.update_text(response),
            None => false,
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
                egui::widgets::stroke_ui(ui, &mut self.stroke, "Style");
                ui.horizontal_top(|ui| {
                    if ui.button("⭕").clicked() {
                        self.annotation_builder = Some(
                            AnnotationBuilder::new(ctx.current_frame_number, self.stroke)
                                .rectangle(std::f32::INFINITY),
                        );
                    }
                    if ui.button("□").clicked() {
                        self.annotation_builder = Some(
                            AnnotationBuilder::new(ctx.current_frame_number, self.stroke)
                                .rectangle(0.0),
                        );
                    }
                    if ui.button("↘").clicked() {
                        self.annotation_builder = Some(
                            AnnotationBuilder::new(ctx.current_frame_number, self.stroke).arrow(),
                        );
                    }
                });
                egui::introspection::font_id_ui(ui, &mut self.font);
                ui.horizontal_top(|ui| {
                    ui.text_edit_singleline(&mut self.text_buffer);
                    if ui.button("💬").clicked() {
                        self.annotation_builder = Some(
                            AnnotationBuilder::new(ctx.current_frame_number, self.stroke)
                                .text(self.text_buffer.clone(), self.font.clone()),
                        );
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
