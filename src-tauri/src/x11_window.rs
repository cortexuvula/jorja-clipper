#[cfg(target_os = "linux")]
use x11rb::{
    connection::Connection,
    protocol::xproto::{
        ChangeWindowAttributesAux, ConfigureWindowAux, ConnectionExt, CreateWindowAux,
        Screen, WindowClass,
    },
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
    CURRENT_TIME,
};

#[cfg(target_os = "linux")]
pub struct X11Window {
    conn: RustConnection,
    screen_num: usize,
    window_id: u32,
    parent_id: u32,
    mapped: bool,
}

#[cfg(target_os = "linux")]
impl X11Window {
    /// Create a new X11 child window inside the given parent window
    pub fn create_child(parent_window_id: u32) -> Result<Self, String> {
        let (conn, screen_num) =
            RustConnection::connect(None).map_err(|e| format!("Failed to connect to X11: {}", e))?;

        let screen = &conn.setup().roots[screen_num];

        // Create child window
        let window_id = conn.generate_id().map_err(|e| format!("Failed to generate window ID: {}", e))?;

        let win_aux = CreateWindowAux::new()
            .background_pixel(screen.white_pixel)
            .border_pixel(screen.black_pixel);

        // Position at 0,0 relative to parent, will be repositioned later
        conn.create_window(
            screen.root_depth,
            window_id,
            parent_window_id,
            0, // x relative to parent
            0, // y relative to parent
            1, // width (will be resized)
            1, // height (will be resized)
            0, // border width
            WindowClass::INPUT_OUTPUT,
            0, // visual (copy from parent)
            &win_aux,
        )
        .map_err(|e| format!("Failed to create window: {}", e))?;

        conn.flush().map_err(|e| format!("Failed to flush: {}", e))?;

        Ok(Self {
            conn,
            screen_num,
            window_id,
            parent_id: parent_window_id,
            mapped: false,
        })
    }

    /// Get the X11 window ID for use with mpv's --wid
    pub fn window_id(&self) -> u64 {
        self.window_id as u64
    }

    /// Position and resize the child window
    pub fn configure(&self, x: i32, y: i32, width: u32, height: u32) -> Result<(), String> {
        let aux = ConfigureWindowAux::new()
            .x(x)
            .y(y)
            .width(width)
            .height(height);

        self.conn
            .configure_window(self.window_id, &aux)
            .map_err(|e| format!("Failed to configure window: {}", e))?;

        self.conn.flush().map_err(|e| format!("Failed to flush: {}", e))?;
        Ok(())
    }

    /// Show the window (map it)
    pub fn show(&mut self) -> Result<(), String> {
        if !self.mapped {
            self.conn
                .map_window(self.window_id)
                .map_err(|e| format!("Failed to map window: {}", e))?;
            self.conn.flush().map_err(|e| format!("Failed to flush: {}", e))?;
            self.mapped = true;
        }
        Ok(())
    }

    /// Hide the window (unmap it)
    pub fn hide(&mut self) -> Result<(), String> {
        if self.mapped {
            self.conn
                .unmap_window(self.window_id)
                .map_err(|e| format!("Failed to unmap window: {}", e))?;
            self.conn.flush().map_err(|e| format!("Failed to flush: {}", e))?;
            self.mapped = false;
        }
        Ok(())
    }
}

#[cfg(target_os = "linux")]
impl Drop for X11Window {
    fn drop(&mut self) {
        // Unmap if still mapped
        if self.mapped {
            let _ = self.conn.unmap_window(self.window_id);
        }
        // Destroy the window
        let _ = self.conn.destroy_window(self.window_id);
        let _ = self.conn.flush();
    }
}

#[cfg(not(target_os = "linux"))]
pub struct X11Window;

#[cfg(not(target_os = "linux"))]
impl X11Window {
    pub fn create_child(_parent_window_id: u32) -> Result<Self, String> {
        Err("X11 windows only supported on Linux".to_string())
    }
    pub fn window_id(&self) -> u64 {
        0
    }
    pub fn configure(&self, _x: i32, _y: i32, _width: u32, _height: u32) -> Result<(), String> {
        Ok(())
    }
    pub fn show(&mut self) -> Result<(), String> {
        Ok(())
    }
    pub fn hide(&mut self) -> Result<(), String> {
        Ok(())
    }
}
