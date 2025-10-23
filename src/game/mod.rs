use alloc::vec;
use alloc::vec::Vec;
use conquer_once::spin::OnceCell;
use libtinyos::{eprintln, println};
use rand::{Rng, SeedableRng, rngs::SmallRng};
use spin::Mutex;
use tinygraphics::{
    backend::GraphicsBackend,
    pixelcolor::Rgb888,
    prelude::{Dimensions, Point, Primitive, RgbColor, Size},
    primitives::{PrimitiveStyle, PrimitiveStyleBuilder, Rectangle, StrokeAlignment},
};

use crate::{
    graphics::graphics,
    interface::{KeyCode, query_keyboard_once},
};

// TODO: clean up blocking checks, ...

const X_ANCHOR: i32 = 300;
const Y_ANCHOR: i32 = 100;
const MAX_X: i32 = 500;
const MAX_Y: i32 = 300;

const GRANULE_SIZE: i32 = 10;
const LINES: usize = ((MAX_Y - Y_ANCHOR) / GRANULE_SIZE) as usize;
const COLS: usize = ((MAX_X - X_ANCHOR) / GRANULE_SIZE) as usize;

/*
coord system:
             MAX_X, col_idx == COLS
             |
[            |
    0, 0, 0, 0 <--- Y_ANCHOR, row_idx == 0
    0, 0, 0, 0
    0, 0, 0, 0 <-- MAX_Y, row_idx == LINES
]   |
    |
    X_ANCHOR
    col_idx == 0
*/

static RNG: OnceCell<Mutex<SmallRng>> = OnceCell::uninit();

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
        println!("starting up...");

        Self::redraw();
        let first = ShapeBuilder::long().build();
        first.draw();

        // graphics().inner().flush().unwrap();

        Self {
            score: 0,
            line_counts: [0; LINES],
            heights: [0; COLS],
            settled_piece: Shape::default(),
            falling_piece: first,
        }
    }

    fn next_piece(&mut self) {
        self.falling_piece = match RNG
            .get_or_init(|| Mutex::new(SmallRng::seed_from_u64(42)))
            .lock()
            .random_range(..5)
        {
            0_u32 => ShapeBuilder::long(),
            1_u32 => ShapeBuilder::quad(),
            2_u32 => ShapeBuilder::t(),
            3_u32 => ShapeBuilder::z(),
            4_u32 => ShapeBuilder::l(),
            _ => unreachable!(),
        }
        .build();
    }

    fn handle_input(&mut self, input: Option<&KeyCode>) {
        _ = match input {
            Some(KeyCode::ArrowDown) => self.falling_piece.down(),
            Some(KeyCode::ArrowLeft) => self.falling_piece.left_checked(|shape| {
                shape
                    .elements
                    .iter()
                    .any(|element| would_be_blocked(element, &self.heights))
            }),
            Some(KeyCode::ArrowRight) => self.falling_piece.right_checked(|shape| {
                shape
                    .elements
                    .iter()
                    .any(|element| would_be_blocked(element, &self.heights))
            }),
            Some(KeyCode::Esc) => {
                // TODO menu
                None
            }
            Some(KeyCode::Char('k')) => self
                .falling_piece
                .rotate_counterclockwise(|element| would_be_blocked(element, &self.heights)),
            Some(KeyCode::Char('l')) => self
                .falling_piece
                .rotate_clockwise(|element| would_be_blocked(element, &self.heights)),
            None => self.falling_piece.down(),
            _ => None,
        };
    }

    fn redraw() {
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
        // No need to redraw / flush currently, as we use the kernel fb via mmap
        // Self::redraw();
        if self
            .falling_piece
            .elements
            .iter()
            .any(|element| is_blocked(element, &self.heights))
        {
            self.handle_collision();
            self.next_piece();
        }
        self.settled_piece.draw();
        self.falling_piece.draw();

        // graphics().inner().flush().unwrap();
    }

    fn handle_collision(&mut self) {
        let mut full = Vec::new();
        for element in self.falling_piece.elements.iter() {
            let row = ((element.top_left().y - Y_ANCHOR) / GRANULE_SIZE) as usize;
            let col = ((element.top_left().x - X_ANCHOR) / GRANULE_SIZE) as usize;

            if row <= 1 {
                self.restart();
                return;
            }

            self.heights[col] = LINES as i32 - row as i32;
            self.line_counts[row] += 1;
            if self.line_counts[row] >= COLS as u8 {
                full.push(row);
                self.line_counts[row] = 0;
            }
        }

        self.settled_piece.merge(self.falling_piece.clone());
        if !full.is_empty() {
            // sort cleared lines, such that higher lines (lower idx) get popped first
            full.sort();
            self.clear_lines(&full);
        }
    }

    fn clear_lines(&mut self, lines: &[usize]) {
        // lines sorted form highest line (0) to lowest line (LINES)
        self.settled_piece.remove(|element| {
            lines.contains(&(((element.top_left().y - Y_ANCHOR) / GRANULE_SIZE) as usize))
        });

        let mut acc = Vec::with_capacity(lines.len());

        // drop lines down from highest to lowest line
        for (i, line) in lines.iter().enumerate() {
            let Some(mut falling) = self
                .settled_piece
                .split_at_y(*line as i32 * GRANULE_SIZE + Y_ANCHOR)
            else {
                continue;
            };
            for _ in 0..(lines.len() - i) {
                falling.down();
            }
            acc.push(falling);
        }

        for falling in acc {
            self.settled_piece.merge(falling);
        }

        let mut drop_amounts = [0; LINES];
        for &line in lines.iter() {
            for d in drop_amounts.iter_mut().take(line) {
                *d += 1;
            }
        }
        for from in (0..LINES).rev() {
            if drop_amounts[from] > 0 {
                let to = from + drop_amounts[from];
                self.line_counts[to] = self.line_counts[from];
                self.line_counts[from] = 0;
            }
        }

        self.calculate_heights();
        self.score += (lines.len() * COLS) as u32;
    }

    fn calculate_heights(&mut self) {
        for (col, height) in self.heights.iter_mut().enumerate() {
            let x = col as i32 * GRANULE_SIZE + X_ANCHOR;
            let top_most = self
                .settled_piece
                .extreme(
                    |lhs, rhs| {
                        rhs.top_left().x == x
                            && (rhs.top_left().y < lhs.top_left().y || lhs.top_left().x != x)
                    },
                    |element| element.top_left().x == x && element.top_left().y == Y_ANCHOR,
                )
                .map(|item| {
                    if item.top_left().x != x {
                        LINES
                    } else {
                        ((item.top_left().y - Y_ANCHOR) / GRANULE_SIZE) as usize
                    }
                })
                .unwrap_or(LINES);
            *height = (LINES - top_most) as i32;
        }
    }

    fn restart(&mut self) {
        eprintln!(
            "You lost the game with {} points. Restarting...",
            self.score
        );
        *self = Self::new();
    }
}

fn is_blocked(element: &PrimitiveBox, heights: &[i32]) -> bool {
    element.top_left().y + element.inner.size.height as i32
        >= MAX_Y
            - (heights[((element.top_left().x - X_ANCHOR) / GRANULE_SIZE) as usize]) * GRANULE_SIZE
}

fn would_be_blocked(element: &PrimitiveBox, heights: &[i32]) -> bool {
    element.top_left().y
        >= MAX_Y
            - (heights[((element.top_left().x - X_ANCHOR) / GRANULE_SIZE) as usize]) * GRANULE_SIZE
}

fn snap_to_grid(point: &mut Point, scaler: i32) {
    let dx = point.x % GRANULE_SIZE;
    let dy = point.y % GRANULE_SIZE;

    if dx < GRANULE_SIZE / scaler {
        point.x -= dx;
    } else {
        point.x += GRANULE_SIZE - dx;
    }
    if dy < GRANULE_SIZE / scaler {
        point.y -= dy;
    } else {
        point.y += GRANULE_SIZE - dy;
    }
}

trait Object {
    fn left(&mut self) -> Option<()>;
    fn right(&mut self) -> Option<()>;
    fn down(&mut self) -> Option<()>;
    fn draw(&self);
    fn top_left(&self) -> Point;
    fn bounding_box(&self) -> Rectangle;
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

    fn is_in_bounds(&self) -> bool {
        self.inner.top_left.x >= X_ANCHOR
            && self.inner.top_left.x + self.inner.size.width as i32 <= MAX_X
            && self.inner.top_left.y >= Y_ANCHOR
            && self.inner.top_left.y + self.inner.size.height as i32 <= MAX_Y
    }
}

impl Object for PrimitiveBox {
    fn left(&mut self) -> Option<()> {
        self.inner.top_left.x -= GRANULE_SIZE;
        Some(())
    }

    fn right(&mut self) -> Option<()> {
        self.inner.top_left.x += GRANULE_SIZE;
        Some(())
    }

    fn down(&mut self) -> Option<()> {
        self.inner.top_left.y += GRANULE_SIZE;
        Some(())
    }

    fn draw(&self) {
        graphics()
            .inner()
            .draw_primitive(&self.inner.into_styled(self.style))
            .unwrap();
    }

    fn top_left(&self) -> Point {
        self.inner.top_left
    }

    fn bounding_box(&self) -> Rectangle {
        self.inner.bounding_box()
    }
}

#[derive(Default, Debug, PartialEq, Eq, Clone)]
struct Shape {
    elements: Vec<PrimitiveBox>,
    pivot: Point,
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
            |left, right| right.top_left().x < left.top_left().x,
            |element| element.top_left().x == X_ANCHOR,
        )
    }

    fn rightmost(&self) -> Option<&PrimitiveBox> {
        self.extreme(
            |left, right| right.top_left().x > left.top_left().x,
            |element| element.top_left().x + element.bounding_box().size.width as i32 == MAX_X,
        )
    }

    fn lowest(&self) -> Option<&PrimitiveBox> {
        self.extreme(
            |left, right| right.top_left().y > left.top_left().y,
            |element| element.top_left().y + element.bounding_box().size.height as i32 == MAX_Y,
        )
    }

    fn highest(&self) -> Option<&PrimitiveBox> {
        self.extreme(
            |lhs, rhs| rhs.top_left().y < lhs.top_left().y,
            |element| element.top_left().y == Y_ANCHOR,
        )
    }

    fn merge(&mut self, other: Shape) {
        self.elements.extend(other.elements);
    }

    fn split_at_y(&mut self, y: i32) -> Option<Shape> {
        // keeps elements below y and returns elements above y
        let rhs = self
            .elements
            .extract_if(.., |element| element.top_left().y <= y)
            .collect::<Vec<PrimitiveBox>>();
        if rhs.is_empty() {
            return None;
        }

        Some(Shape {
            elements: rhs,
            pivot: Point::zero(),
        })
    }

    fn remove<F>(&mut self, condition: F)
    where
        F: Fn(&PrimitiveBox) -> bool,
    {
        self.elements.retain(|element| !condition(element));
    }

    #[allow(dead_code)]
    fn recompute_pivot(&mut self) {
        let mut pivot = self.bounding_box().center();
        snap_to_grid(&mut pivot, 1);
        self.pivot = pivot
    }

    fn rotate_clockwise<F>(&mut self, f: F) -> Option<()>
    where
        F: Fn(&PrimitiveBox) -> bool,
    {
        self.rotate(f, -1, 1)
    }

    fn rotate_counterclockwise<F>(&mut self, f: F) -> Option<()>
    where
        F: Fn(&PrimitiveBox) -> bool,
    {
        self.rotate(f, 1, -1)
    }

    fn rotate<F>(&mut self, f: F, x_mul: i32, y_mul: i32) -> Option<()>
    where
        F: Fn(&PrimitiveBox) -> bool,
    {
        let multiplicator = 2;
        let center = self
            .pivot
            .component_mul(Point::new(multiplicator, multiplicator));
        let mut clone = self.clone();
        for element in clone.elements.iter_mut() {
            let x = element.top_left().x * multiplicator - center.x;
            let y = element.top_left().y * multiplicator - center.y;
            element.inner.top_left.x = x_mul * y + center.x;
            element.inner.top_left.y = y_mul * x + center.y;
            snap_to_grid(&mut element.inner.top_left, multiplicator);
            element.inner.top_left.x /= multiplicator;
            element.inner.top_left.y /= multiplicator;
            if !element.is_in_bounds() || f(element) {
                return None;
            }
            assert_eq!(element.top_left().x % GRANULE_SIZE, 0);
            assert_eq!(element.top_left().y % GRANULE_SIZE, 0);
        }
        self.elements = clone.elements;
        Some(())
    }

    fn new(elements: Vec<PrimitiveBox>) -> Self {
        let mut s = Self {
            elements,
            pivot: Point::zero(),
        };
        s.recompute_pivot();
        s
    }

    fn left_checked<F>(&mut self, f: F) -> Option<()>
    where
        F: Fn(&Shape) -> bool,
    {
        let mut clone = self.clone();
        clone.left()?;
        if !f(&clone) {
            *self = clone;
            return Some(());
        }
        None
    }

    fn right_checked<F>(&mut self, f: F) -> Option<()>
    where
        F: Fn(&Shape) -> bool,
    {
        let mut clone = self.clone();
        clone.right()?;
        if !f(&clone) {
            *self = clone;
            return Some(());
        }
        None
    }
}

impl Object for Shape {
    fn left(&mut self) -> Option<()> {
        if let Some(leftmost) = self.leftmost()
            && leftmost.top_left().x > X_ANCHOR
        {
            self.pivot.x -= GRANULE_SIZE;
            for element in self.elements.iter_mut() {
                element.left();
            }
            return Some(());
        }
        None
    }

    fn right(&mut self) -> Option<()> {
        if let Some(rightmost) = self.rightmost()
            && rightmost.top_left().x + (rightmost.bounding_box().size.width as i32) < MAX_X
        {
            self.pivot.x += GRANULE_SIZE;
            for element in self.elements.iter_mut() {
                element.right();
            }
            return Some(());
        }
        None
    }

    fn down(&mut self) -> Option<()> {
        if let Some(lowest) = self.lowest()
            && lowest.top_left().y + (lowest.bounding_box().size.height as i32) < MAX_Y
        {
            self.pivot.y += GRANULE_SIZE;
            for element in self.elements.iter_mut() {
                element.down();
            }
            return Some(());
        }
        None
    }

    fn draw(&self) {
        for element in self.elements.iter() {
            element.draw();
        }
    }

    fn bounding_box(&self) -> Rectangle {
        if let Some(lowest) = self.lowest()
            && let Some(rightmost) = self.rightmost()
            && let Some(leftmost) = self.leftmost()
            && let Some(highest) = self.highest()
        {
            Rectangle::new(
                Point::new(leftmost.top_left().x, highest.top_left().y),
                Size::new(
                    rightmost.top_left().x as u32 + rightmost.bounding_box().size.width
                        - leftmost.top_left().x as u32,
                    lowest.top_left().y as u32 + lowest.bounding_box().size.height
                        - highest.top_left().y as u32,
                ),
            )
        } else {
            Rectangle::zero()
        }
    }

    fn top_left(&self) -> Point {
        self.bounding_box().top_left
    }
}

struct ShapeBuilder {
    inner: Shape,
}

impl ShapeBuilder {
    fn long() -> Self {
        let color = Rgb888::RED;
        Self {
            inner: Shape::new(vec![
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
            ]),
        }
    }

    fn quad() -> Self {
        let color = Rgb888::GREEN;

        Self {
            inner: Shape::new(vec![
                PrimitiveBox::new(
                    X_ANCHOR + (MAX_X - X_ANCHOR) / 2,
                    Y_ANCHOR + GRANULE_SIZE,
                    color,
                ),
                PrimitiveBox::new(
                    X_ANCHOR + (MAX_X - X_ANCHOR) / 2 + GRANULE_SIZE,
                    Y_ANCHOR + GRANULE_SIZE,
                    color,
                ),
                PrimitiveBox::new(X_ANCHOR + (MAX_X - X_ANCHOR) / 2, Y_ANCHOR, color),
                PrimitiveBox::new(
                    X_ANCHOR + (MAX_X - X_ANCHOR) / 2 + GRANULE_SIZE,
                    Y_ANCHOR,
                    color,
                ),
            ]),
        }
    }

    fn t() -> Self {
        let color = Rgb888::MAGENTA;

        Self {
            inner: Shape::new(vec![
                PrimitiveBox::new(
                    X_ANCHOR + (MAX_X - X_ANCHOR) / 2,
                    Y_ANCHOR + GRANULE_SIZE,
                    color,
                ),
                PrimitiveBox::new(
                    X_ANCHOR + (MAX_X - X_ANCHOR) / 2 + GRANULE_SIZE,
                    Y_ANCHOR + GRANULE_SIZE,
                    color,
                ),
                PrimitiveBox::new(
                    X_ANCHOR + (MAX_X - X_ANCHOR) / 2 - GRANULE_SIZE,
                    Y_ANCHOR + GRANULE_SIZE,
                    color,
                ),
                PrimitiveBox::new(X_ANCHOR + (MAX_X - X_ANCHOR) / 2, Y_ANCHOR, color),
            ]),
        }
    }

    fn z() -> Self {
        let color = Rgb888::BLUE;

        Self {
            inner: Shape::new(vec![
                PrimitiveBox::new(
                    X_ANCHOR + (MAX_X - X_ANCHOR) / 2 + GRANULE_SIZE,
                    Y_ANCHOR + GRANULE_SIZE,
                    color,
                ),
                PrimitiveBox::new(
                    X_ANCHOR + (MAX_X - X_ANCHOR) / 2 + 2 * GRANULE_SIZE,
                    Y_ANCHOR + GRANULE_SIZE,
                    color,
                ),
                PrimitiveBox::new(X_ANCHOR + (MAX_X - X_ANCHOR) / 2, Y_ANCHOR, color),
                PrimitiveBox::new(
                    X_ANCHOR + (MAX_X - X_ANCHOR) / 2 + GRANULE_SIZE,
                    Y_ANCHOR,
                    color,
                ),
            ]),
        }
    }

    fn l() -> Self {
        let color = Rgb888::YELLOW;

        Self {
            inner: Shape::new(vec![
                PrimitiveBox::new(
                    X_ANCHOR + (MAX_X - X_ANCHOR) / 2,
                    Y_ANCHOR + 2 * GRANULE_SIZE,
                    color,
                ),
                PrimitiveBox::new(
                    X_ANCHOR + (MAX_X - X_ANCHOR) / 2 + GRANULE_SIZE,
                    Y_ANCHOR + 2 * GRANULE_SIZE,
                    color,
                ),
                PrimitiveBox::new(
                    X_ANCHOR + (MAX_X - X_ANCHOR) / 2,
                    Y_ANCHOR + GRANULE_SIZE,
                    color,
                ),
                PrimitiveBox::new(X_ANCHOR + (MAX_X - X_ANCHOR) / 2, Y_ANCHOR, color),
            ]),
        }
    }

    fn build(self) -> Shape {
        self.inner
    }

    #[allow(dead_code)]
    fn with_color(mut self, color: Rgb888) -> Self {
        self.inner.elements.iter_mut().for_each(|element| {
            element.style = PrimitiveStyleBuilder::new()
                .fill_color(color)
                .stroke_color(color)
                .build()
        });
        self
    }
}
