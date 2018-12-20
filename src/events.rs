use super::EngineState;
use winit::Event;
use winit::WindowEvent::*;
use winit::dpi::LogicalPosition;
use winit;

pub(crate) fn handle(event: Event, state: &mut EngineState) {
    if let Event::WindowEvent{ event, .. } = event {
        match event {
            CloseRequested => state.done = true,
            HiDpiFactorChanged(val) => state.hidpi = val,
            Resized(size) => {
                state.recreate_swapchain = true;
                state.dimensions = [size.width, size.height];
            },
            CursorMoved { position: LogicalPosition { x, y }, .. } => {
                state.push_consts.offset = [
                    (2.*x/state.dimensions[0] - 1.) as f32,
                    (2.*y/state.dimensions[1] - 1.) as f32,
                ];
            },
            _ => (),
        }
    }
}
