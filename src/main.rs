
use vulkano_win::VkSurfaceBuild;
use vulkano::sync::GpuFuture;
use vulkano::framebuffer::Subpass;
use vulkano::framebuffer::Framebuffer;
use vulkano::buffer::CpuAccessibleBuffer;
use vulkano::command_buffer::AutoCommandBufferBuilder;
use vulkano::swapchain::acquire_next_image;
use vulkano::swapchain::Swapchain;
use vulkano::swapchain::SwapchainCreationError;
use vulkano::pipeline::viewport::Viewport;
use vulkano::device::Device;
use vulkano::command_buffer::DynamicState;
use vulkano::{impl_vertex, single_pass_renderpass};

use std::sync::Arc;
use std::str;

use structopt::StructOpt;

const DEFAULT_DIMENSIONS: [u32; 2] = [2448/4, 2048/4];

mod shaders;
mod cli;
mod events;

#[derive(Debug, Clone)]
struct Vertex { position: [f32; 2] }
impl_vertex!(Vertex, position);

#[repr(C)]
#[derive(Copy, Clone)]
struct PushConstant {
    offset: [f32; 2],
}

struct EngineState {
    recreate_swapchain: bool,
    done: bool,
    hidpi: f64,
    dimensions: [f64; 2],
    push_consts: PushConstant,
    dyn_state: DynamicState,
}

fn main() -> Result<(), Box<std::error::Error>> {
    let args = cli::Cli::from_args();

    let mut dimensions = DEFAULT_DIMENSIONS;

    let extensions = vulkano_win::required_extensions();
    let instance = vulkano::instance::Instance::new(None, &extensions, None)
        .expect("failed to create instance");

    let physical = vulkano::instance::PhysicalDevice::enumerate(&instance)
                            .next().expect("no device available");
    eprintln!("Using device: {} (type: {:?})", physical.name(), physical.ty());

    let mut events_loop = winit::EventsLoop::new();
    let surface = winit::WindowBuilder::new()
        .with_title("Camera")
        .with_dimensions((dimensions[0], dimensions[1]).into())
        //.with_min_dimensions((dimensions[0], dimensions[1]).into())
        .with_decorations(true)
        //.with_fullscreen(Some(events_loop.get_primary_monitor()))
        .build_vk_surface(&events_loop, instance.clone())
        .expect("failed to build window");

    let queue = physical.queue_families().find(|&q|
        q.supports_graphics() && surface.is_supported(q).unwrap_or(false)
    ).expect("couldn't find a graphical queue family");
    let device_ext = vulkano::device::DeviceExtensions {
        khr_swapchain: true,
        .. vulkano::device::DeviceExtensions::none()
    };
    let (device, mut queues) = Device::new(
        physical, physical.supported_features(), &device_ext,
        [(queue, 0.5)].iter().cloned()
    ).expect("failed to create device");
    let queue = queues.next().unwrap();


    let (mut swapchain, mut images) = {
        let caps = surface.capabilities(physical)
            .expect("failed to get surface capabilities");

        dimensions = caps.current_extent.unwrap_or(dimensions);
        let usage = caps.supported_usage_flags;
        let alpha = caps.supported_composite_alpha.iter().next().unwrap();
        let format = caps.supported_formats[0].0;

        Swapchain::new(
            device.clone(), surface.clone(), caps.min_image_count,
            format, dimensions, 1, usage, &queue,
            vulkano::swapchain::SurfaceTransform::Identity, alpha,
            args.mode, true, None
        ).expect("failed to create swapchain")
    };

    let vertex_buffer = CpuAccessibleBuffer::<[Vertex]>::from_iter(
        device.clone(), vulkano::buffer::BufferUsage::all(),
       [
            Vertex { position: [-0.2,  0.0 ] },
            Vertex { position: [ 0.2,  0.0 ] },
            Vertex { position: [ 0.0, -0.2 ] },
            Vertex { position: [ 0.0,  0.2 ] },
       ].iter().cloned()
    ).expect("failed to create buffer");

    let vs = shaders::vs::Shader::load(device.clone())
        .expect("failed to create shader module");
    let fs = shaders::fs::Shader::load(device.clone())
        .expect("failed to create shader module");

    let renderpass = Arc::new(
        single_pass_renderpass!(device.clone(),
            attachments: {
                color: {
                    load: Clear,
                    store: Store,
                    format: swapchain.format(),
                    samples: 1,
                }
            },
            pass: {
                color: [color],
                depth_stencil: {}
            }
        ).unwrap()
    );

    let pipeline = Arc::new(vulkano::pipeline::GraphicsPipeline::start()
        .vertex_input_single_buffer::<Vertex>()
        .vertex_shader(vs.main_entry_point(), ())
        .line_list()
        .viewports_dynamic_scissors_irrelevant(1)
        .fragment_shader(fs.main_entry_point(), ())
        .blend_alpha_blending()
        .render_pass(Subpass::from(renderpass.clone(), 0).unwrap())
        .build(device.clone())
        .expect("Failed to build main pipeline")
    );

    let mut framebuffers: Vec<Arc<Framebuffer<_,_>>> = images.iter()
        .map(|image|
            Arc::new(Framebuffer::start(renderpass.clone())
                 .add(image.clone()).unwrap()
                 .build().unwrap())
        ).collect::<Vec<Arc<Framebuffer<_,_>>>>();

    let prev_frame = Box::new(vulkano::sync::now(device.clone()));
    let mut previous_frame = prev_frame as Box<GpuFuture>;

    let hidpi = surface.window().get_hidpi_factor();
    let mut state = EngineState {
        recreate_swapchain: false,
        done: false,
        hidpi: hidpi,
        dimensions: [
            dimensions[0] as f64,
            dimensions[1] as f64,
        ],
        push_consts: PushConstant { offset: [0., 0.] },
        dyn_state: DynamicState {
            line_width: None,
            viewports: Some(vec![Viewport {
                origin: [0.0, 0.0],
                dimensions: [dimensions[0] as f32, dimensions[1] as f32],
                depth_range: 0.0 .. 1.0,
            }]),
            scissors: None,
        },
    };

    loop {
        previous_frame.cleanup_finished();
        events_loop.poll_events(|event| events::handle(event, &mut state));

        if state.recreate_swapchain {
            let default_dims = [
                (state.dimensions[0]*state.hidpi) as u32,
                (state.dimensions[1]*state.hidpi) as u32,
            ];
            let dims = surface.capabilities(physical)
                .expect("failed to get surface capabilities")
                .current_extent.unwrap_or(default_dims);

            match swapchain.recreate_with_dimension(dims) {
                Ok((new_swapchain, new_images)) => {
                    swapchain = new_swapchain;
                    images = new_images;
                },
                Err(SwapchainCreationError::UnsupportedDimensions) => {
                    continue;
                },
                Err(err) => panic!("{:?}", err)
            };

            framebuffers = images.iter().map(|image|
                Arc::new(Framebuffer::start(renderpass.clone())
                         .add(image.clone()).unwrap()
                         .build().unwrap())
                ).collect::<Vec<_>>();

            match &mut state.dyn_state.viewports {
                Some(v) if v.len() == 1 => {
                    v[0].dimensions = [dims[0] as f32, dims[1] as f32];
                },
                _ => panic!("unexpected viewports value"),
            };

            state.recreate_swapchain = false;
        }

        let next_img = acquire_next_image(swapchain.clone(), None);
        let (image_num, future) = match next_img {
            Ok(r) => r,
            Err(vulkano::swapchain::AcquireError::OutOfDate) => {
                state.recreate_swapchain = true;
                continue;
            },
            Err(err) => panic!("{:?}", err)
        };

        let cb = AutoCommandBufferBuilder
            ::primary_one_time_submit(device.clone(), queue.family())
            .unwrap()
            .begin_render_pass(
                framebuffers[image_num].clone(), false,
                vec![[0.0, 0.0, 0.0, 1.0].into()]).unwrap()
            .draw(
                pipeline.clone(),
                &state.dyn_state,
                vertex_buffer.clone(),
                (), state.push_consts,
            ).expect("Main pipeline draw fail")
            .end_render_pass().unwrap().build().unwrap();

        let future = previous_frame.join(future)
            .then_execute(queue.clone(), cb).unwrap()
            .then_swapchain_present(queue.clone(), swapchain.clone(), image_num)
            .then_signal_fence_and_flush().unwrap();
        previous_frame = Box::new(future) as Box<vulkano::sync::GpuFuture>;

        if state.done { return Ok(()); }
    }
}
