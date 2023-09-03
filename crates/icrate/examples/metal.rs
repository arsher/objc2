#![deny(unsafe_op_in_unsafe_fn)]

use core::{cell::RefCell, ptr::NonNull};

use icrate::{
    AppKit::{
        NSApplication, NSApplicationActivationPolicyRegular, NSApplicationDelegate,
        NSBackingStoreBuffered, NSWindow, NSWindowStyleMaskClosable, NSWindowStyleMaskResizable,
        NSWindowStyleMaskTitled,
    },
    Foundation::{
        ns_string, MainThreadMarker, NSDate, NSNotification, NSObject, NSObjectProtocol, NSPoint,
        NSRect, NSSize,
    },
    Metal::{
        MTLCommandBuffer, MTLCommandEncoder, MTLCommandQueue, MTLCreateSystemDefaultDevice,
        MTLDevice, MTLDrawable, MTLLibrary, MTLPrimitiveTypeTriangle, MTLRenderCommandEncoder,
        MTLRenderPipelineDescriptor, MTLRenderPipelineState,
    },
    MetalKit::{MTKView, MTKViewDelegate},
};
use objc2::{
    declare::{Ivar, IvarDrop},
    declare_class, msg_send, msg_send_id,
    mutability::MainThreadOnly,
    rc::Id,
    runtime::ProtocolObject,
    ClassType,
};

#[rustfmt::skip]
const SHADERS: &str = r#"
    #include <metal_stdlib>
        
    struct SceneProperties {
        float time;
    };        
    
    struct VertexInput {
        metal::packed_float3 position;
        metal::packed_float3 color;
    };
    
    struct VertexOutput {
        metal::float4 position [[position]];
        metal::float4 color;
    };
    
    vertex VertexOutput vertex_main(
        device const SceneProperties& properties [[buffer(0)]],
        device const VertexInput* vertices [[buffer(1)]],
        uint vertex_idx [[vertex_id]]
    ) {
        VertexOutput out;
        VertexInput in = vertices[vertex_idx];
        out.position =
            metal::float4(
                metal::float2x2(
                    metal::cos(properties.time), -metal::sin(properties.time),
                    metal::sin(properties.time),  metal::cos(properties.time)
                ) * in.position.xy,
                in.position.z,
                1);
        out.color = metal::float4(in.color, 1);
        return out;
    }
    
    fragment metal::float4 fragment_main(VertexOutput in [[stage_in]]) {
        return in.color;
    }
"#;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct SceneProperties {
    pub time: f32,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct VertexInput {
    pub position: Position,
    pub color: Color,
}

#[derive(Copy, Clone)]
// NOTE: this has the same ABI as `MTLPackedFloat3`
#[repr(C)]
pub struct Position {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

#[derive(Copy, Clone)]
// NOTE: this has the same ABI as `MTLPackedFloat3`
#[repr(C)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

type IdCell<T> = Box<RefCell<Option<Id<T>>>>;

macro_rules! idcell {
    ($name:ident <= $this:expr) => {
        let $name = $this.$name.borrow();
        let $name = $name
            .as_ref()
            .expect(concat!(stringify!($name), " ivar should be initialized"));
    };
}

// declare the Objective-C class machinery
declare_class!(
    // declare the delegate class with our instance variables
    #[rustfmt::skip] // FIXME: rustfmt breaks the macro parsing apparently
    struct Delegate {
        start_date: IvarDrop<Id<NSDate>, "_start_date">,
        command_queue: IvarDrop<IdCell<ProtocolObject<dyn MTLCommandQueue>>, "_command_queue">,
        pipeline_state: IvarDrop<IdCell<ProtocolObject<dyn MTLRenderPipelineState>>, "_pipeline_state">,
        window: IvarDrop<IdCell<NSWindow>, "_window">,
    }
    mod ivars;

    // declare the class type
    unsafe impl ClassType for Delegate {
        type Super = NSObject;
        type Mutability = MainThreadOnly;
        const NAME: &'static str = "Delegate";
    }

    // define the Delegate methods (e.g., initializer)
    unsafe impl Delegate {
        #[method(init)]
        unsafe fn init(this: *mut Self) -> Option<NonNull<Self>> {
            let this: Option<&mut Self> = msg_send![super(this), init];
            this.map(|this| {
                Ivar::write(&mut this.start_date, unsafe { NSDate::now() });
                Ivar::write(&mut this.command_queue, IdCell::default());
                Ivar::write(&mut this.pipeline_state, IdCell::default());
                Ivar::write(&mut this.window, IdCell::default());
                NonNull::from(this)
            })
        }
    }

    // define the delegate methods for the `NSApplicationDelegate` protocol
    unsafe impl NSApplicationDelegate for Delegate {
        #[method(applicationDidFinishLaunching:)]
        #[allow(non_snake_case)]
        unsafe fn applicationDidFinishLaunching(&self, _notification: &NSNotification) {
            let mtm = MainThreadMarker::from(self);
            // create the app window
            let window = {
                let content_rect = NSRect::new(NSPoint::new(0., 0.), NSSize::new(768., 768.));
                let style = NSWindowStyleMaskClosable
                    | NSWindowStyleMaskResizable
                    | NSWindowStyleMaskTitled;
                let backing_store_type = NSBackingStoreBuffered;
                let flag = false;
                unsafe {
                    NSWindow::initWithContentRect_styleMask_backing_defer(
                        mtm.alloc(),
                        content_rect,
                        style,
                        backing_store_type,
                        flag,
                    )
                }
            };

            // get the default device
            let device = {
                let ptr = unsafe { MTLCreateSystemDefaultDevice() };
                unsafe { Id::retain(ptr) }.expect("Failed to get default system device.")
            };

            // create the command queue
            let command_queue = device
                .newCommandQueue()
                .expect("Failed to create a command queue.");

            // create the metal view
            let mtk_view = {
                let frame_rect = unsafe { window.frame() };
                unsafe { MTKView::initWithFrame_device(mtm.alloc(), frame_rect, Some(&device)) }
            };

            // create the pipeline descriptor
            let pipeline_descriptor = MTLRenderPipelineDescriptor::new();

            unsafe {
                pipeline_descriptor
                    .colorAttachments()
                    .objectAtIndexedSubscript(0)
                    .setPixelFormat(mtk_view.colorPixelFormat());
            }

            // compile the shaders
            let library = device
                .newLibraryWithSource_options_error(ns_string!(SHADERS), None)
                .expect("Failed to create a library.");

            // configure the vertex shader
            let vertex_function = library.newFunctionWithName(ns_string!("vertex_main"));
            pipeline_descriptor.setVertexFunction(vertex_function.as_deref());

            // configure the fragment shader
            let fragment_function = library.newFunctionWithName(ns_string!("fragment_main"));
            pipeline_descriptor.setFragmentFunction(fragment_function.as_deref());

            // create the pipeline state
            let pipeline_state = device
                .newRenderPipelineStateWithDescriptor_error(&pipeline_descriptor)
                .expect("Failed to create a pipeline state.");

            // configure the metal view delegate
            unsafe {
                let object = ProtocolObject::from_ref(self);
                mtk_view.setDelegate(Some(object));
            }

            // configure the window
            unsafe {
                window.setContentView(Some(&mtk_view));
                window.center();
                window.setTitle(ns_string!("metal example"));
                window.makeKeyAndOrderFront(None);
            }

            // initialize the delegate state
            self.command_queue.replace(Some(command_queue));
            self.pipeline_state.replace(Some(pipeline_state));
            self.window.replace(Some(window));
        }
    }

    // define the delegate methods for the `MTKViewDelegate` protocol
    unsafe impl MTKViewDelegate for Delegate {
        #[method(drawInMTKView:)]
        #[allow(non_snake_case)]
        unsafe fn drawInMTKView(&self, mtk_view: &MTKView) {
            idcell!(command_queue <= self);
            idcell!(pipeline_state <= self);

            // FIXME: icrate `MTKView` doesn't have a generated binding for `currentDrawable` yet
            // (because it needs a definition of `CAMetalDrawable`, which we don't support yet) so
            // we have to use a raw `msg_send_id` call here instead.
            let current_drawable: Option<Id<ProtocolObject<dyn MTLDrawable>>> =
                msg_send_id![mtk_view, currentDrawable];

            // prepare for drawing
            let Some(current_drawable) = current_drawable else {
                return;
            };
            let Some(command_buffer) = command_queue.commandBuffer() else {
                return;
            };
            let Some(pass_descriptor) = (unsafe { mtk_view.currentRenderPassDescriptor() }) else {
                return;
            };
            let Some(encoder) = command_buffer.renderCommandEncoderWithDescriptor(&pass_descriptor)
            else {
                return;
            };

            // compute the scene properties
            let scene_properties_data = &SceneProperties {
                time: unsafe { self.start_date.timeIntervalSinceNow() } as f32,
            };
            // write the scene properties to the vertex shader argument buffer at index 0
            let scene_properties_bytes = NonNull::from(scene_properties_data);
            unsafe {
                encoder.setVertexBytes_length_atIndex(
                    scene_properties_bytes.cast::<core::ffi::c_void>(),
                    core::mem::size_of_val(scene_properties_data),
                    0,
                )
            };

            // compute the triangle geometry
            let vertex_input_data: &[VertexInput] = &[
                VertexInput {
                    position: Position {
                        x: -f32::sqrt(3.0) / 4.0,
                        y: -0.25,
                        z: 0.,
                    },
                    color: Color {
                        r: 1.,
                        g: 0.,
                        b: 0.,
                    },
                },
                VertexInput {
                    position: Position {
                        x: f32::sqrt(3.0) / 4.0,
                        y: -0.25,
                        z: 0.,
                    },
                    color: Color {
                        r: 0.,
                        g: 1.,
                        b: 0.,
                    },
                },
                VertexInput {
                    position: Position {
                        x: 0.,
                        y: 0.5,
                        z: 0.,
                    },
                    color: Color {
                        r: 0.,
                        g: 0.,
                        b: 1.,
                    },
                },
            ];
            // write the triangle geometry to the vertex shader argument buffer at index 1
            let vertex_input_bytes = NonNull::from(vertex_input_data);
            unsafe {
                encoder.setVertexBytes_length_atIndex(
                    vertex_input_bytes.cast::<core::ffi::c_void>(),
                    core::mem::size_of_val(vertex_input_data),
                    1,
                )
            };

            // configure the encoder with the pipeline and draw the triangle
            encoder.setRenderPipelineState(pipeline_state);
            unsafe {
                encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveTypeTriangle, 0, 3)
            };
            encoder.endEncoding();

            // schedule the command buffer for display and commit
            command_buffer.presentDrawable(&current_drawable);
            command_buffer.commit();
        }

        #[method(mtkView:drawableSizeWillChange:)]
        #[allow(non_snake_case)]
        unsafe fn mtkView_drawableSizeWillChange(&self, _view: &MTKView, _size: NSSize) {
            // println!("mtkView_drawableSizeWillChange");
        }
    }
);

unsafe impl NSObjectProtocol for Delegate {}

impl Delegate {
    pub fn new(mtm: MainThreadMarker) -> Id<Self> {
        unsafe { msg_send_id![mtm.alloc(), init] }
    }
}

fn main() {
    let mtm = MainThreadMarker::new().unwrap();
    // configure the app
    let app = unsafe { NSApplication::sharedApplication(mtm) };
    unsafe { app.setActivationPolicy(NSApplicationActivationPolicyRegular) };

    // initialize the delegate
    let delegate = Delegate::new(mtm);

    // configure the application delegate
    unsafe {
        let object = ProtocolObject::from_ref(&*delegate);
        app.setDelegate(Some(object))
    };

    // run the app
    unsafe { app.run() };
}