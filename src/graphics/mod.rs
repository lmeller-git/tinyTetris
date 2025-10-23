use conquer_once::spin::OnceCell;
use spin::{Mutex, MutexGuard};
use tinygraphics::{
    backend::{KernelFBWrapper, PrimitiveDrawer},
    pixelcolor::Rgb888,
};

static GRAPHICS: OnceCell<GraphicsHandler<'static>> = OnceCell::uninit();

type Backend<'a> = PrimitiveDrawer<'a, KernelFBWrapper, Rgb888>;

pub fn init_gfx() {
    GRAPHICS.init_once(GraphicsHandler::new);
}

pub fn graphics<'a>() -> &'a GraphicsHandler<'static> {
    GRAPHICS.get().unwrap()
}

pub struct GraphicsHandler<'a> {
    drawer: Mutex<Backend<'a>>,
}

impl<'a> GraphicsHandler<'a> {
    fn new() -> Self {
        let backend = PrimitiveDrawer::default();
        Self {
            drawer: backend.into(),
        }
    }

    pub fn inner<'lock>(&'lock self) -> MutexGuard<'lock, Backend<'a>> {
        self.drawer.lock()
    }
}
