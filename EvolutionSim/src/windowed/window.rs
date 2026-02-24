#![allow(temporary_cstring_as_ptr)]

use std::alloc::{self, Layout};
use std::ffi::{CStr, c_void};
use std::ptr::{self, write};
use std::rc::Rc;
use std::{ffi::CString, os::raw::c_char};

use crate::Config;

extern crate glfw;

use crate::population::Population;
use crate::DebugCell::DebugRefCell;
use crate::{gene::NodeID, neuron_presence, windowed::shader::Shader};

use super::WindowingStatus;

type UserPointer = (Config, Rc<DebugRefCell<WindowingStatus>>);

#[repr(C)]
pub struct Window {
    ptr: *mut glfw::ffi::GLFWwindow,
    background_VAO: u32,
    cell_VAO: u32,
    vertical_shader: Shader,
    horizontal_shader: Shader,
    background_shader: Shader,
    cell_shader: Shader,
}

impl Window {
    pub fn createWindow(
        config: &Config,
        windowing_status: &Rc<DebugRefCell<WindowingStatus>>,
        width: i32,
        height: i32,
    ) -> Option<Window> {
        let ptr = unsafe {
            let result = glfw::ffi::glfwInit();
            if result != glfw::ffi::GLFW_TRUE {
                let mut string = ptr::null::<core::ffi::c_char>();
                // SAFETY:
                //
                // TODO:
                let result = glfw::ffi::glfwGetError(&mut string);
                if !string.is_null() {
                    let c_str = CStr::from_ptr(string);
                    eprintln!("{c_str:?}");
                }
                return None;
            }

            let ptr = glfw::ffi::glfwCreateWindow(
                width,
                height,
                CString::new("BIOSIM").unwrap().as_ptr() as *const c_char,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            );

            if ptr == std::ptr::null_mut() as *mut glfw::ffi::GLFWwindow {
                let mut string = ptr::null::<core::ffi::c_char>();
                // SAFETY:
                //
                // TODO:
                let result = glfw::ffi::glfwGetError(&mut string);
                if !string.is_null() {
                    let c_str = CStr::from_ptr(string);
                    eprintln!("{c_str:?}");
                }
                return None;
            };

            glfw::ffi::glfwMakeContextCurrent(ptr);

            glfw::ffi::glfwSwapInterval(1);

            gl::load_with(loadfn);

            glfw::ffi::glfwSetWindowCloseCallback(ptr, Some(windowCloseCallback));

            glfw::ffi::glfwSetFramebufferSizeCallback(ptr, Some(framebufferSizeCallback));

            glfw::ffi::glfwSetKeyCallback(ptr, Some(keyCallback));

            let layout = Layout::new::<UserPointer>();
            let configPtr = alloc::alloc(layout) as *mut UserPointer;
            write(configPtr, (config.clone(), windowing_status.clone()));

            glfw::ffi::glfwSetWindowUserPointer(ptr, configPtr as *mut c_void);

            ptr
        };

        let horizontal = Shader::new("shaders/horizontal.vs", "shaders/base.fs");
        let vertical = Shader::new("shaders/vertical.vs", "shaders/base.fs");
        let background = Shader::new("shaders/background.vs", "shaders/base.fs");
        let cell = Shader::new("shaders/cell.vs", "shaders/cell.fs");

        let mut window = Window {
            ptr,
            background_VAO: 0,
            cell_VAO: 0,
            horizontal_shader: horizontal,
            vertical_shader: vertical,
            background_shader: background,
            cell_shader: cell,
        };

        window.create_square_VAOs();

        Some(window)
    }

    fn create_square_VAOs(&mut self) {
        let backgroundVertices: [f32; 12] = [
            -1.0, -1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, -1.0, -1.0, -1.0,
        ];

        let mut VAO: u32 = 0;
        unsafe {
            gl::GenVertexArrays(1, &mut VAO);
            gl::BindVertexArray(VAO);
        }

        let mut VBO: u32 = 0;
        unsafe {
            gl::GenBuffers(1, &mut VBO);

            gl::BindBuffer(gl::ARRAY_BUFFER, VBO);

            gl::BufferData(
                gl::ARRAY_BUFFER,
                backgroundVertices.len() as isize * 4,
                backgroundVertices.as_ptr() as *const std::ffi::c_void,
                gl::STATIC_DRAW,
            );

            gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, 2 * 4, ptr::null());

            gl::EnableVertexAttribArray(0);
        }

        self.background_VAO = VAO;

        let cell_vertices: [f32; 12] = [
            0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, -1.0, 0.0, -1.0,
        ];

        let mut VAO: u32 = 0;
        unsafe {
            gl::GenVertexArrays(1, &mut VAO);
            gl::BindVertexArray(VAO);
        }

        let mut VBO: u32 = 0;
        unsafe {
            gl::GenBuffers(1, &mut VBO);

            gl::BindBuffer(gl::ARRAY_BUFFER, VBO);

            gl::BufferData(
                gl::ARRAY_BUFFER,
                cell_vertices.len() as isize * 4,
                cell_vertices.as_ptr() as *const std::ffi::c_void,
                gl::STATIC_DRAW,
            );

            gl::EnableVertexAttribArray(0);
            gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, 2 * 4, ptr::null());

            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        }

        self.cell_VAO = VAO;
    }

    pub fn render(&self, config: &Config, population: &Population) {
        unsafe {
            gl::ClearColor(0.0, 0.0, 0.0, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);

            gl::BindVertexArray(self.background_VAO);

            self.background_shader.apply();
            self.background_shader
                .set_uniform_vec3("color", 1.0, 1.0, 1.0);

            gl::DrawArrays(gl::TRIANGLES, 0, 6);

            gl::BindVertexArray(0);

            {
                let mut VAO = 0;

                gl::GenVertexArrays(1, &mut VAO);
                gl::BindVertexArray(VAO);

                let mut VBO = 0;

                gl::GenBuffers(1, &mut VBO);
                gl::BindBuffer(gl::ARRAY_BUFFER, VBO);

                let mut buffer: Vec<f32> = vec![
                    //12 f32s to denote cell vertices
                    0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, -1.0, 0.0, -1.0,
                ];

                let livingCells = {
                    let indices = population.getLivingIndices();
                    let mut living = Vec::with_capacity(indices.len());
                    for index in indices {
                        living.push((
                            population.getCellMovementData(index),
                            population.getCellMiscData(index),
                        ));
                    }
                    living
                };

                let len = livingCells.len();

                for (movement, other) in livingCells {
                    buffer.push(
                        ((movement.getCoords().0) as f32) / (config.get_grid_width() as f32) * 2.0
                            - 1.0,
                    );
                    buffer.push(
                        ((movement.getCoords().1 + 1) as f32) / (config.get_grid_height() as f32)
                            * 2.0
                            - 1.0,
                    );
                    buffer.push(other.color.0 as f32 / 255.0);
                    buffer.push(other.color.1 as f32 / 255.0);
                    buffer.push(other.color.2 as f32 / 255.0);
                }

                gl::BufferData(
                    gl::ARRAY_BUFFER,
                    (buffer.len() * 4) as isize,
                    buffer.as_ptr() as *const std::ffi::c_void,
                    gl::STATIC_DRAW,
                );

                gl::EnableVertexAttribArray(0);
                gl::VertexAttribPointer(0, 2, gl::FLOAT, gl::FALSE, 2 * 4, ptr::null());

                gl::EnableVertexAttribArray(1);
                gl::VertexAttribPointer(
                    1,
                    2,
                    gl::FLOAT,
                    gl::FALSE,
                    5 * 4,
                    (12 * 4) as *const std::ffi::c_void,
                );
                gl::VertexAttribDivisor(1, 1);

                gl::EnableVertexAttribArray(2);
                gl::VertexAttribPointer(
                    2,
                    3,
                    gl::FLOAT,
                    gl::FALSE,
                    5 * 4,
                    (14 * 4) as *const std::ffi::c_void,
                );
                gl::VertexAttribDivisor(2, 1);

                self.cell_shader.apply();
                self.cell_shader
                    .set_uniform_int("width", config.get_grid_width() as i32);
                self.cell_shader
                    .set_uniform_int("height", config.get_grid_height() as i32);

                gl::DrawArraysInstanced(gl::TRIANGLES, 0, 6, len as i32);
            }

            glfw::ffi::glfwSwapBuffers(self.ptr);
        }
    }

    #[inline]
    pub fn shouldClose(&self) -> bool {
        unsafe { glfw::ffi::glfwWindowShouldClose(self.ptr) == 1 }
    }

    #[inline]
    pub fn poll(&self) {
        unsafe {
            glfw::ffi::glfwPollEvents();
        }
    }

    #[inline]
    pub fn make_current(&self) {
        unsafe {
            glfw::ffi::glfwMakeContextCurrent(self.ptr);
        }
    }
}

impl Drop for Window {
    fn drop(&mut self) {
        unsafe {
            let ptr = glfw::ffi::glfwGetWindowUserPointer(self.ptr);
            glfw::ffi::glfwSetWindowUserPointer(self.ptr, ptr::null_mut());

            let layout = Layout::new::<Config>();
            alloc::dealloc(ptr as *mut u8, layout);

            glfw::ffi::glfwTerminate();
        }
    }
}

extern "C" fn windowCloseCallback(window: *mut glfw::ffi::GLFWwindow) {
    unsafe {
        glfw::ffi::glfwSetWindowShouldClose(window, glfw::ffi::GLFW_TRUE);
    }
}

extern "C" fn framebufferSizeCallback(
    _window: *mut glfw::ffi::GLFWwindow,
    width: i32,
    height: i32,
) {
    unsafe {
        if width > height {
            gl::Viewport(0, 0, height, height);
        } else if height >= width {
            gl::Viewport(0, (height - width) / 2, width, width);
        }
    }
}

extern "C" fn keyCallback(
    window: *mut glfw::ffi::GLFWwindow,
    key: i32,
    _scancode: i32,
    action: i32,
    _mods: i32,
) {
    if key == glfw::ffi::GLFW_KEY_R && action == glfw::ffi::GLFW_PRESS {
        let ptr = unsafe { get_window_user_ptr(window) };
        unsafe { (*ptr).1.borrowMut().should_reset = true };
    } else if key == glfw::ffi::GLFW_KEY_SPACE && action == glfw::ffi::GLFW_PRESS {
        let ptr = unsafe { get_window_user_ptr(window) };
        let paused = !unsafe { (*ptr).1.borrow().is_paused };
        unsafe { (*ptr).1.borrowMut().is_paused = paused };
    } else if key == glfw::ffi::GLFW_KEY_E && action == glfw::ffi::GLFW_PRESS {
        print!("\nNeuron Frequencies: \n");
        unsafe {
            for (index, count) in neuron_presence.iter().enumerate() {
                println!("{}: {}", NodeID::from_index(index), *count);
            }
            println!();
        }
    } else if key == glfw::ffi::GLFW_KEY_ESCAPE {
        unsafe { glfw::ffi::glfwSetWindowShouldClose(window, glfw::ffi::GLFW_TRUE) };
    } else if key == glfw::ffi::GLFW_KEY_S && action == glfw::ffi::GLFW_PRESS {
    } else if key == glfw::ffi::GLFW_KEY_C && action == glfw::ffi::GLFW_PRESS {
        let ptr = unsafe { get_window_user_ptr(window) };

        println!("\n{}", unsafe { (*ptr).0.to_string() });
    }
}

unsafe fn get_window_user_ptr(window: *mut glfw::ffi::GLFWwindow) -> *mut UserPointer {
    let ptr = glfw::ffi::glfwGetWindowUserPointer(window);
    if ptr.is_null() {
        todo!()
    }

    ptr as *mut UserPointer
}

fn loadfn(symbol: &'static str) -> *const c_void {
    let Some(func) = glfw::with_c_str(symbol, |procname| unsafe {
        glfw::ffi::glfwGetProcAddress(procname)
    }) else {
        return ptr::null();
    };

    func as *const c_void
}

pub fn wait(
    window: &Window,
    windowing_status: &Rc<DebugRefCell<WindowingStatus>>,
    accounted_time: &mut f64,
    secs: f64,
) {
    while unsafe { glfw::ffi::glfwGetTime() } - *accounted_time < secs {
        window.poll();
        if window.shouldClose() {
            break;
        }
        if windowing_status.borrow().is_paused {
            unsafe {
                *accounted_time = glfw::ffi::glfwGetTime();
            }
        }
    }
    *accounted_time += secs;
}
