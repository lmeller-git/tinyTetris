use alloc::vec;
use alloc::vec::Vec;
use libtinyos::{eprintln, println, syscall, yield_now};
use tinygraphics::{
    backend::GraphicsBackend,
    pixelcolor::Rgb888,
    prelude::{Point, Primitive, RgbColor, Size},
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StrokeAlignment},
};

use crate::{
    graphics::graphics,
    interface::{KeyCode, query_keyboard_once},
};

const X_ANCHOR: i32 = 100;
const Y_ANCHOR: i32 = 100;
const MAX_X: i32 = 300;
const MAX_Y: i32 = 300;

const GRANULE_SIZE: i32 = 10;
const LINES: usize = ((MAX_Y - Y_ANCHOR) / GRANULE_SIZE) as usize;
const COLS: usize = ((MAX_X - X_ANCHOR) / GRANULE_SIZE) as usize;

pub fn game_loop() {
    let mut buf: [u8; 10] = [0; 10];
    let mut state = GameState::new();
    loop {
        // currently this blocks. TODO: add block with timeout, such that the game progresses without input
        let next_keycodes = query_keyboard_once(&mut buf);
        state.handle_input(next_keycodes.first());
        state.validate();
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
struct GameState {
    score: u32,
    line_counts: [u8; LINES],
    heights: [i32; COLS],
    settled_piece: Shape,
    falling_piece: Shape,
}

impl GameState {
    fn new() -> Self {
        println!("drawing outline...");
        graphics()
            .inner()
            .draw_primitive(
                &Rectangle::new(
                    Point::new(X_ANCHOR, Y_ANCHOR),
                    Size::new((MAX_X - X_ANCHOR) as u32, (MAX_Y - Y_ANCHOR) as u32),
                )
                .into_styled(
                    PrimitiveStyleBuilder::new()
                        .fill_color(Rgb888::BLACK)
                        .stroke_color(Rgb888::WHITE)
                        .stroke_alignment(StrokeAlignment::Outside)
                        .stroke_width(4)
                        .build(),
                ),
            )
            .unwrap();

        println!("starting up...");

        let first = ShapeBuilder::long().build();
        first.draw();

        graphics().inner().flush().unwrap();

        Self {
            score: 0,
            line_counts: [0; LINES],
            heights: [0; COLS],
            settled_piece: Shape::default(),
            falling_piece: first,
        }
    }

    fn next_piece(&mut self) {
        self.falling_piece = ShapeBuilder::long().build();
    }

    fn is_lost(&self) -> bool {
        self.heights.iter().any(|item| *item > LINES as i32)
    }

    fn handle_input(&mut self, input: Option<&KeyCode>) {
        match input {
            Some(KeyCode::ArrowDown) => self.falling_piece.down(),
            Some(KeyCode::ArrowLeft) => self.falling_piece.left(),
            Some(KeyCode::ArrowRight) => self.falling_piece.right(),
            Some(KeyCode::Esc) => { // TODO menu
            }
            None => self.falling_piece.down(),
            _ => {}
        }
    }

    fn redraw(&self) {
        graphics()
            .inner()
            .draw_primitive(
                &Rectangle::new(
                    Point::new(X_ANCHOR, Y_ANCHOR),
                    Size::new((MAX_X - X_ANCHOR) as u32, (MAX_Y - Y_ANCHOR) as u32),
                )
                .into_styled(
                    PrimitiveStyleBuilder::new()
                        .fill_color(Rgb888::BLACK)
                        .stroke_color(Rgb888::WHITE)
                        .stroke_alignment(StrokeAlignment::Outside)
                        .stroke_width(4)
                        .build(),
                ),
            )
            .unwrap();
    }

    fn validate(&mut self) {
        self.redraw();
        if let Some(lowest_point) = self.falling_piece.lowest()
            && (lowest_point.top_left().y + lowest_point.size().height as i32 == MAX_Y
                || self.falling_piece.elements.iter().any(|element| {
                    element.top_left().y
                        >= MAX_X
                            - (self.heights
                                [((element.top_left().x - X_ANCHOR) / GRANULE_SIZE) as usize]
                                + 1)
                                * GRANULE_SIZE
                }))
        {
            self.handle_collision();
            self.next_piece();
        }
        self.settled_piece.draw();
        self.falling_piece.draw();

        // TODO restart on death or sth
        if self.is_lost() {
            eprintln!(
                "You lost the game with {} points. It will now shutdown the OS.",
                self.score
            );
            yield_now();
            unsafe { syscall!(11) };
        }

        graphics().inner().flush().unwrap();
    }

    fn handle_collision(&mut self) {
        let mut full = Vec::new();
        for element in self.falling_piece.elements.iter() {
            let idx = ((element.top_left().y - Y_ANCHOR) / GRANULE_SIZE) as usize;
            let col = ((element.top_left().x - X_ANCHOR) / GRANULE_SIZE) as usize;
            self.heights[col] += 1;
            self.line_counts[idx] += 1;
            if self.line_counts[idx] == COLS as u8 {
                full.push(idx);
                self.line_counts[idx] = 0;
            }
        }

        self.settled_piece.merge(self.falling_piece.clone());
        if !full.is_empty() {
            full.sort_by(|a, b| b.cmp(a));
            self.clear_lines(&full);
        }
    }

    fn clear_lines(&mut self, lines: &[usize]) {
        self.settled_piece.remove(|element| {
            lines.contains(&(((element.top_left().y - Y_ANCHOR) / GRANULE_SIZE) as usize))
        });
        let mut temp = Vec::with_capacity(lines.len());
        for (i, line) in lines.iter().enumerate() {
            let to_fall = lines.len() - i;
            let Some(mut falling) = self
                .settled_piece
                .split_at_y(*line as i32 * GRANULE_SIZE + Y_ANCHOR)
            else {
                continue;
            };

            // TODO add down_n
            for _ in 0..to_fall {
                falling.down();
            }
            temp.push(falling);
        }

        for shape in temp.into_iter() {
            self.settled_piece.merge(shape);
        }
        for height in self.heights.iter_mut() {
            *height -= lines.len() as i32;
        }
        self.score += (lines.len() * COLS) as u32;
    }
}

trait Object {
    fn left(&mut self);
    fn right(&mut self);
    fn down(&mut self);
    fn draw(&self);
    fn top_left(&self) -> &Point;
    fn size(&self) -> &Size {
        &Size {
            width: GRANULE_SIZE as u32,
            height: GRANULE_SIZE as u32,
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq, Clone)]
struct PrimitiveBox {
    inner: Rectangle,
    style: PrimitiveStyle<Rgb888>,
}

impl PrimitiveBox {
    fn new(x: i32, y: i32, color: Rgb888) -> Self {
        Self {
            inner: Rectangle {
                top_left: Point { x, y },
                size: Size::new(GRANULE_SIZE as u32, GRANULE_SIZE as u32),
            },
            style: PrimitiveStyleBuilder::new()
                .fill_color(color)
                .stroke_color(color)
                .build(),
        }
    }
}

impl Object for PrimitiveBox {
    fn left(&mut self) {
        if self.inner.top_left.x == X_ANCHOR {
            return;
        }
        self.inner.top_left.x -= GRANULE_SIZE;
    }

    fn right(&mut self) {
        if self.inner.top_left.x + self.inner.size.width as i32 == MAX_X {
            return;
        }
        self.inner.top_left.x += GRANULE_SIZE;
    }

    fn down(&mut self) {
        if self.inner.top_left.y + self.inner.size.height as i32 == MAX_Y {
            return;
        }

        self.inner.top_left.y += GRANULE_SIZE;
    }

    fn draw(&self) {
        graphics()
            .inner()
            .draw_primitive(&self.inner.into_styled(self.style))
            .unwrap();
    }

    fn top_left(&self) -> &Point {
        &self.inner.top_left
    }

    fn size(&self) -> &Size {
        &self.inner.size
    }
}

#[derive(Default, Debug, PartialEq, Eq, Clone)]
struct Shape {
    elements: Vec<PrimitiveBox>,
}

impl Shape {
    fn extreme<F, M>(&self, cmp: F, stop: M) -> Option<&PrimitiveBox>
    where
        F: Fn(&PrimitiveBox, &PrimitiveBox) -> bool,
        M: Fn(&PrimitiveBox) -> bool,
    {
        let mut elements = self.elements.iter();
        let mut current = elements.next()?;
        for element in elements {
            if stop(current) {
                break;
            }
            if cmp(current, element) {
                current = element;
            }
        }
        Some(current)
    }

    fn leftmost(&self) -> Option<&PrimitiveBox> {
        self.extreme(
            |left, right| left.top_left().x < right.top_left().x,
            |element| element.top_left().x == X_ANCHOR,
        )
    }

    fn rightmost(&self) -> Option<&PrimitiveBox> {
        self.extreme(
            |left, right| left.top_left().x > right.top_left().x,
            |element| element.top_left().x + element.size().width as i32 == X_ANCHOR,
        )
    }

    fn lowest(&self) -> Option<&PrimitiveBox> {
        self.extreme(
            |left, right| left.top_left().y < right.top_left().y,
            |element| element.top_left().y + element.size().height as i32 == MAX_Y,
        )
    }

    fn merge(&mut self, other: Shape) {
        self.elements.extend(other.elements);
    }

    fn split_at_y(&mut self, y: i32) -> Option<Shape> {
        // keeps elements blow y and returns elements above y
        let rhs = self
            .elements
            .extract_if(.., |element| element.top_left().y <= y) // assuming y is a multiple of GRANULARITY
            .collect::<Vec<PrimitiveBox>>();
        if rhs.is_empty() {
            return None;
        }

        Some(Shape { elements: rhs })
    }

    fn remove<F>(&mut self, condition: F)
    where
        F: Fn(&PrimitiveBox) -> bool,
    {
        self.elements.retain(|element| !condition(element));
    }
}

impl Object for Shape {
    fn left(&mut self) {
        if let Some(leftmost) = self.leftmost()
            && leftmost.top_left().x > X_ANCHOR
        {
            for element in self.elements.iter_mut() {
                element.left();
            }
        }
    }

    fn right(&mut self) {
        if let Some(rightmost) = self.rightmost()
            && rightmost.top_left().x + (rightmost.size().width as i32) < MAX_X
        {
            for element in self.elements.iter_mut() {
                element.right();
            }
        }
    }

    fn down(&mut self) {
        if let Some(lowest) = self.lowest()
            && lowest.top_left().y + (lowest.size().height as i32) < MAX_Y
        {
            for element in self.elements.iter_mut() {
                element.down();
            }
        }
    }

    fn draw(&self) {
        for element in self.elements.iter() {
            element.draw();
        }
    }

    fn top_left(&self) -> &Point {
        todo!()
    }

    fn size(&self) -> &Size {
        todo!()
    }
}

struct ShapeBuilder {
    inner: Shape,
}

impl ShapeBuilder {
    fn long() -> Self {
        let color = Rgb888::RED;
        Self {
            inner: Shape {
                elements: vec![
                    PrimitiveBox::new(
                        X_ANCHOR + (MAX_X - X_ANCHOR) / 2,
                        Y_ANCHOR + 3 * GRANULE_SIZE,
                        color,
                    ),
                    PrimitiveBox::new(
                        X_ANCHOR + (MAX_X - X_ANCHOR) / 2,
                        Y_ANCHOR + 2 * GRANULE_SIZE,
                        color,
                    ),
                    PrimitiveBox::new(
                        X_ANCHOR + (MAX_X - X_ANCHOR) / 2,
                        Y_ANCHOR + GRANULE_SIZE,
                        color,
                    ),
                    PrimitiveBox::new(X_ANCHOR + (MAX_X - X_ANCHOR) / 2, Y_ANCHOR, color),
                ],
            },
        }
    }

    fn build(self) -> Shape {
        self.inner
    }
}
