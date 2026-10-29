#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]
// GUI launcher: the native WinForms interface is maintained in RNG-6x6-GUI.ps1.
// This launcher never needs a console and displays errors in a Windows dialog.

#[cfg(windows)]
mod windows_gui {
    use std::ffi::c_void;
    use std::os::windows::process::CommandExt;
    use std::path::PathBuf;
    use std::process::Command;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn MessageBoxW(
            window: *mut c_void,
            text: *const u16,
            caption: *const u16,
            style: u32,
        ) -> i32;
    }

    fn show_error(message: &str) {
        let safe = message.chars().take(3000).collect::<String>();
        let text: Vec<u16> = safe.encode_utf16().chain(std::iter::once(0)).collect();
        let caption: Vec<u16> = "RNG 6x6 - Error"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();

        // SAFETY: text and caption are NUL-terminated and remain allocated
        // for the duration of this synchronous user32 call.
        unsafe {
            MessageBoxW(
                std::ptr::null_mut(),
                text.as_ptr(),
                caption.as_ptr(),
                0x10, // MB_ICONERROR
            );
        }
    }

    fn run_gui() -> Result<(), String> {
        let path = std::env::current_exe()
            .map_err(|error| format!("No se puede determinar la ubicacion del programa: {error}"))?;
        let directory = path.parent().ok_or("No se puede determinar la carpeta del programa")?;
        let script: PathBuf = directory.join("RNG-6x6-GUI.ps1");
        let cli: PathBuf = directory.join("RNG-6x6-CLI.exe");

        if !script.is_file() || !cli.is_file() {
            return Err(format!(
                "Faltan archivos para ejecutar la interfaz.\n\n\
                 Extrae TODO el ZIP a una misma carpeta.\n\n\
                 Necesarios:\n{}\n{}",
                script.display(),
                cli.display()
            ));
        }

        let output = Command::new("powershell.exe")
            .arg("-NoProfile")
            .arg("-ExecutionPolicy")
            .arg("Bypass")
            .arg("-STA")
            .arg("-WindowStyle")
            .arg("Hidden")
            .arg("-File")
            .arg(&script)
            .current_dir(directory)
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .map_err(|error| {
                format!(
                    "No fue posible iniciar Windows PowerShell:\n{error}\n\n\
                     Alternativa: ejecuta INICIAR-WINDOWS.cmd."
                )
            })?;

        if !output.status.success() {
            return Err(format!(
                "La interfaz de Windows no pudo iniciarse.\n\n\
                 Codigo: {}\n\nSalida de error:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(())
    }

    pub fn run() {
        if let Err(error) = run_gui() {
            show_error(&error);
        }
    }
}

#[cfg(windows)]
fn main() {
    windows_gui::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Esta interfaz grafica solo se ejecuta en Windows.");
}
