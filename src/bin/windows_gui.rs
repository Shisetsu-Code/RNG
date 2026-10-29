#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
mod windows_gui {
    use std::ffi::c_void;
    use std::fs::{self, File};
    use std::os::windows::process::CommandExt;
    use std::path::PathBuf;
    use std::process::{Child, Command, Stdio};
    use std::ptr;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::sync::{Mutex, OnceLock};
    use std::thread;
    use std::time::{Duration, Instant};

    type Handle = *mut c_void;

    #[repr(C)]
    struct WndClassW {
        style: u32,
        wnd_proc: Option<unsafe extern "system" fn(Handle, u32, usize, isize) -> isize>,
        class_extra: i32,
        window_extra: i32,
        instance: Handle,
        icon: Handle,
        cursor: Handle,
        background: Handle,
        menu_name: *const u16,
        class_name: *const u16,
    }

    #[repr(C)]
    #[derive(Default)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Msg {
        hwnd: Handle,
        message: u32,
        wparam: usize,
        lparam: isize,
        time: u32,
        pt: Point,
        private: u32,
    }

    #[link(name = "user32")]
    unsafe extern "system" {
        fn RegisterClassW(wnd_class: *const WndClassW) -> u16;
        fn CreateWindowExW(
            ex_style: u32,
            class_name: *const u16,
            window_name: *const u16,
            style: u32,
            x: i32,
            y: i32,
            width: i32,
            height: i32,
            parent: Handle,
            menu: Handle,
            instance: Handle,
            param: *mut c_void,
        ) -> Handle;
        fn DefWindowProcW(window: Handle, message: u32, wparam: usize, lparam: isize) -> isize;
        fn DestroyWindow(window: Handle) -> i32;
        fn ShowWindow(window: Handle, command: i32) -> i32;
        fn UpdateWindow(window: Handle) -> i32;
        fn GetMessageW(message: *mut Msg, window: Handle, min: u32, max: u32) -> i32;
        fn TranslateMessage(message: *const Msg) -> i32;
        fn DispatchMessageW(message: *const Msg) -> isize;
        fn PostQuitMessage(exit_code: i32);
        fn PostMessageW(window: Handle, message: u32, wparam: usize, lparam: isize) -> i32;
        fn GetWindowTextLengthW(window: Handle) -> i32;
        fn GetWindowTextW(window: Handle, buffer: *mut u16, max: i32) -> i32;
        fn SetWindowTextW(window: Handle, text: *const u16) -> i32;
        fn SendMessageW(window: Handle, message: u32, wparam: usize, lparam: isize) -> isize;
        fn EnableWindow(window: Handle, enabled: i32) -> i32;
        fn MessageBoxW(parent: Handle, message: *const u16, caption: *const u16, flags: u32) -> i32;
        fn LoadCursorW(instance: Handle, name: *const u16) -> Handle;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(module: *const u16) -> Handle;
    }

    const WS_CHILD: u32 = 0x4000_0000;
    const WS_VISIBLE: u32 = 0x1000_0000;
    const WS_BORDER: u32 = 0x0080_0000;
    const WS_OVERLAPPEDWINDOW: u32 = 0x00CF_0000;
    const WS_VSCROLL: u32 = 0x0020_0000;
    const WS_TABSTOP: u32 = 0x0001_0000;
    const ES_MULTILINE: u32 = 0x0004;
    const ES_READONLY: u32 = 0x0800;
    const ES_AUTOVSCROLL: u32 = 0x0040;
    const BS_AUTOCHECKBOX: u32 = 0x0003;
    const BM_GETCHECK: u32 = 0x00F0;
    const BM_SETCHECK: u32 = 0x00F1;
    const BST_CHECKED: usize = 1;
    const WM_COMMAND: u32 = 0x0111;
    const WM_DESTROY: u32 = 0x0002;
    const WM_APP_COMPLETE: u32 = 0x8001;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    const BTN_START: usize = 101;
    const BTN_CANCEL: usize = 102;

    #[derive(Default)]
    struct Ui {
        packages: AtomicUsize,
        directory: AtomicUsize,
        status: AtomicUsize,
        report: AtomicUsize,
        run_button: AtomicUsize,
        cancel_button: AtomicUsize,
        compact: AtomicUsize,
        split: AtomicUsize,
        reuse: AtomicUsize,
        child: Mutex<Option<Child>>,
        completion: Mutex<Option<Completion>>,
        cancelled: AtomicBool,
    }

    struct Completion {
        ok: bool,
        text: String,
    }

    static UI: OnceLock<Ui> = OnceLock::new();

    fn state() -> &'static Ui {
        UI.get_or_init(Ui::default)
    }

    fn wide(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn as_handle(raw: usize) -> Handle {
        raw as Handle
    }

    fn write_text(window: usize, value: &str) {
        let text = wide(value);
        // SAFETY: the window is a control created on this UI thread and text is NUL-terminated.
        unsafe {
            SetWindowTextW(as_handle(window), text.as_ptr());
        }
    }

    fn read_text(window: usize) -> String {
        // SAFETY: GetWindowText writes up to its explicit buffer length.
        unsafe {
            let count = GetWindowTextLengthW(as_handle(window));
            let mut buffer = vec![0_u16; count.max(0) as usize + 1];
            let read = GetWindowTextW(
                as_handle(window),
                buffer.as_mut_ptr(),
                i32::try_from(buffer.len()).unwrap_or(i32::MAX),
            );
            String::from_utf16_lossy(&buffer[..read.max(0) as usize])
        }
    }

    fn is_checked(window: usize) -> bool {
        // SAFETY: BM_GETCHECK is synchronous and does not use pointers.
        unsafe { SendMessageW(as_handle(window), BM_GETCHECK, 0, 0) as usize == BST_CHECKED }
    }

    fn message_box(parent: Handle, message: &str, error: bool) {
        let title = wide("RNG 6x6");
        let content = wide(message);
        // SAFETY: both UTF-16 buffers are NUL-terminated and remain alive for the call.
        unsafe {
            MessageBoxW(
                parent,
                content.as_ptr(),
                title.as_ptr(),
                if error { 0x10 } else { 0x40 },
            );
        }
    }

    fn executable_directory() -> Result<PathBuf, String> {
        let exe = std::env::current_exe()
            .map_err(|error| format!("No se encuentra la ruta del ejecutable: {error}"))?;
        Ok(exe
            .parent()
            .ok_or("No se encuentra el directorio del ejecutable")?
            .to_path_buf())
    }

    fn start_generation(window: Handle) -> Result<(), String> {
        let ui = state();
        if ui.child.lock().map_err(|e| e.to_string())?.is_some() {
            return Err("Ya hay una generacion en ejecucion".to_owned());
        }
        let packages_text = read_text(ui.packages.load(Ordering::SeqCst));
        let count: u64 = packages_text
            .trim()
            .parse()
            .map_err(|_| "Ingresa una cantidad entera de paquetes mayor que cero")?;
        if count == 0 {
            return Err("La cantidad de paquetes debe ser mayor que cero".to_owned());
        }
        let output_text = read_text(ui.directory.load(Ordering::SeqCst));
        if output_text.trim().is_empty() {
            return Err("Ingresa una carpeta de salida".to_owned());
        }
        let directory = PathBuf::from(output_text.trim());
        fs::create_dir_all(&directory)
            .map_err(|error| format!("No se puede crear la carpeta de salida: {error}"))?;

        let executable = executable_directory()?.join("RNG-6x6-CLI.exe");
        if !executable.is_file() {
            return Err(format!(
                "No existe {}.\nExtrae TODOS los archivos del ZIP en la misma carpeta.",
                executable.display()
            ));
        }
        let log_path = directory.join("ejecucion.log");
        let error_path = directory.join("errores.log");
        let stdout = File::create(&log_path)
            .map_err(|error| format!("No se puede escribir el log: {error}"))?;
        let stderr = File::create(&error_path)
            .map_err(|error| format!("No se puede escribir el log de errores: {error}"))?;

        let mut command = Command::new(&executable);
        command
            .arg("--packages")
            .arg(count.to_string())
            .arg("--out")
            .arg(&directory)
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr))
            .creation_flags(CREATE_NO_WINDOW);

        if !is_checked(ui.compact.load(Ordering::SeqCst)) {
            command.arg("--full");
        }
        if is_checked(ui.split.load(Ordering::SeqCst)) {
            command.arg("--split-files");
        }
        let catalog = directory.join("catalogo_128000.json");
        if is_checked(ui.reuse.load(Ordering::SeqCst)) && catalog.exists() {
            command.arg("--reuse-catalog").arg(&catalog);
        }

        let child = command
            .spawn()
            .map_err(|error| format!("No se puede iniciar el motor Rust: {error}"))?;
        *ui.child.lock().map_err(|e| e.to_string())? = Some(child);
        ui.cancelled.store(false, Ordering::SeqCst);
        write_text(
            ui.status.load(Ordering::SeqCst),
            &format!("Generando {count} paquetes ({} apuestas)...", u128::from(count) * 100),
        );
        write_text(
            ui.report.load(Ordering::SeqCst),
            &format!(
                "El motor esta ejecutandose en segundo plano.\nDirectorio: {}\nLos resultados se guardan en archivos JSON y CSV.",
                directory.display()
            ),
        );
        // SAFETY: control handles refer to controls created on this thread.
        unsafe {
            EnableWindow(as_handle(ui.run_button.load(Ordering::SeqCst)), 0);
            EnableWindow(as_handle(ui.cancel_button.load(Ordering::SeqCst)), 1);
        }

        let hwnd = window as usize;
        thread::spawn(move || {
            let success = loop {
                let result = {
                    let guard = ui.child.lock();
                    match guard {
                        Ok(mut guard) => {
                            if let Some(child) = guard.as_mut() {
                                match child.try_wait() {
                                    Ok(Some(status)) => {
                                        *guard = None;
                                        Some(Ok(status.success()))
                                    }
                                    Ok(None) => None,
                                    Err(error) => {
                                        *guard = None;
                                        Some(Err(format!("Error de proceso: {error}")))
                                    }
                                }
                            } else {
                                Some(Err("El proceso termino inesperadamente".to_owned()))
                            }
                        }
                        Err(error) => Some(Err(format!("Error de sincronizacion: {error}"))),
                    }
                };
                if let Some(done) = result {
                    break done;
                }
                thread::sleep(Duration::from_millis(200));
            };
            let output = fs::read_to_string(&log_path).unwrap_or_default();
            let error_output = fs::read_to_string(&error_path).unwrap_or_default();
            let cancelled = ui.cancelled.load(Ordering::SeqCst);
            let (ok, text) = match success {
                Ok(true) if !cancelled => (true, last_chars(&output, 6000)),
                Ok(_) if cancelled => (
                    false,
                    "Generacion cancelada. Pueden quedar archivos parciales.".to_owned(),
                ),
                Ok(false) => (
                    false,
                    format!(
                        "El motor finalizo con un error:\n{}\n{}",
                        error_output,
                        last_chars(&output, 2000)
                    ),
                ),
                Err(error) => (false, error),
                _ => (false, "Error desconocido".to_owned()),
            };
            if let Ok(mut complete) = ui.completion.lock() {
                *complete = Some(Completion { ok, text });
            }
            // SAFETY: PostMessageW receives the HWND as an opaque value and reports failure if
            // the user closed the window before the worker finished.
            unsafe {
                PostMessageW(as_handle(hwnd), WM_APP_COMPLETE, 0, 0);
            }
        });
        Ok(())
    }

    fn last_chars(source: &str, max_chars: usize) -> String {
        let total = source.chars().count();
        source.chars().skip(total.saturating_sub(max_chars)).collect()
    }

    fn cancel_generation() {
        let ui = state();
        ui.cancelled.store(true, Ordering::SeqCst);
        if let Ok(mut child) = ui.child.lock() {
            if let Some(child) = child.as_mut() {
                let _ = child.kill();
            }
        }
        write_text(ui.status.load(Ordering::SeqCst), "Cancelando generacion...");
    }

    fn finish_generation() {
        let ui = state();
        if let Ok(mut result) = ui.completion.lock() {
            if let Some(completion) = result.take() {
                write_text(
                    ui.status.load(Ordering::SeqCst),
                    if completion.ok { "Completado" } else { "Error o cancelacion" },
                );
                write_text(ui.report.load(Ordering::SeqCst), &completion.text);
            }
        }
        // SAFETY: controls are owned by the UI thread.
        unsafe {
            EnableWindow(as_handle(ui.run_button.load(Ordering::SeqCst)), 1);
            EnableWindow(as_handle(ui.cancel_button.load(Ordering::SeqCst)), 0);
        }
    }

    unsafe extern "system" fn window_proc(
        window: Handle,
        message: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        match message {
            WM_COMMAND => {
                if (wparam & 0xFFFF) == BTN_START {
                    if let Err(error) = start_generation(window) {
                        message_box(window, &error, true);
                        write_text(state().status.load(Ordering::SeqCst), &error);
                    }
                    0
                } else if (wparam & 0xFFFF) == BTN_CANCEL {
                    cancel_generation();
                    0
                } else {
                    // SAFETY: delegated to Win32 for messages this window does not handle.
                    unsafe { DefWindowProcW(window, message, wparam, lparam) }
                }
            }
            WM_APP_COMPLETE => {
                finish_generation();
                0
            }
            WM_DESTROY => {
                if let Ok(mut child) = state().child.lock() {
                    if let Some(child) = child.as_mut() {
                        let _ = child.kill();
                    }
                }
                // SAFETY: terminates the UI message loop.
                unsafe { PostQuitMessage(0) };
                0
            }
            _ => {
                // SAFETY: default handling of all unrecognized window messages.
                unsafe { DefWindowProcW(window, message, wparam, lparam) }
            }
        }
    }

    fn create_child(
        parent: Handle,
        instance: Handle,
        kind: &str,
        text: &str,
        style: u32,
        dimensions: [i32; 4],
        id: usize,
    ) -> Result<usize, String> {
        let class = wide(kind);
        let caption = wide(text);
        // SAFETY: Win32 copies window class/text contents before returning. The menu
        // argument for a WS_CHILD control is the numeric control ID.
        let child = unsafe {
            CreateWindowExW(
                0,
                class.as_ptr(),
                caption.as_ptr(),
                WS_CHILD | WS_VISIBLE | style,
                dimensions[0],
                dimensions[1],
                dimensions[2],
                dimensions[3],
                parent,
                as_handle(id),
                instance,
                ptr::null_mut(),
            )
        };
        if child.is_null() {
            Err(format!("Windows no pudo crear el control '{text}'"))
        } else {
            Ok(child as usize)
        }
    }

    fn create_window() -> Result<Handle, String> {
        let instance = unsafe { GetModuleHandleW(ptr::null()) };
        if instance.is_null() {
            return Err("No se puede localizar el modulo de Windows".to_owned());
        }
        let class_name = wide("RNG6x6NativeWindowV2");
        let class = WndClassW {
            style: 0,
            wnd_proc: Some(window_proc),
            class_extra: 0,
            window_extra: 0,
            instance,
            icon: ptr::null_mut(),
            cursor: unsafe { LoadCursorW(ptr::null_mut(), 32512_usize as *const u16) }, // IDC_ARROW
            background: as_handle(6), // COLOR_WINDOW + 1
            menu_name: ptr::null(),
            class_name: class_name.as_ptr(),
        };
        // SAFETY: class name and WNDCLASSW are valid for the full RegisterClassW call.
        if unsafe { RegisterClassW(&class) } == 0 {
            return Err("No se puede registrar la ventana nativa en Windows".to_owned());
        }
        let title = wide("RNG 6x6 - 128000 tipos, paquetes de 100");
        // SAFETY: RegisterClassW has registered the named class above.
        let hwnd = unsafe {
            CreateWindowExW(
                0,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW,
                100,
                100,
                755,
                595,
                ptr::null_mut(),
                ptr::null_mut(),
                instance,
                ptr::null_mut(),
            )
        };
        if hwnd.is_null() {
            return Err("No se puede crear la ventana principal".to_owned());
        }
        let result = install_controls(hwnd, instance);
        if let Err(error) = result {
            // SAFETY: hwnd is a live window created above.
            unsafe {
                DestroyWindow(hwnd);
            }
            return Err(error);
        }
        Ok(hwnd)
    }

    fn install_controls(window: Handle, instance: Handle) -> Result<(), String> {
        let ui = state();

        create_child(
            window, instance, "STATIC", "RNG 6x6 - Generador de paquetes",
            0, [20, 15, 650, 32], 0,
        )?;
        create_child(
            window, instance, "STATIC",
            "128000 matrices | 100 apuestas por paquete | RTP teorico 96,5%",
            0, [20, 52, 680, 25], 0,
        )?;

        create_child(
            window, instance, "STATIC", "Cantidad de paquetes:", 0,
            [20, 99, 250, 25], 0,
        )?;
        let packages = create_child(
            window, instance, "EDIT", "100", WS_BORDER | WS_TABSTOP,
            [20, 130, 280, 29], 0,
        )?;
        ui.packages.store(packages, Ordering::SeqCst);

        create_child(
            window, instance, "STATIC", "Ejemplo: 100 paquetes = 10.000 apuestas",
            0, [315, 134, 390, 25], 0,
        )?;

        create_child(
            window, instance, "STATIC", "Carpeta de salida:", 0,
            [20, 184, 380, 25], 0,
        )?;

        let default_path = executable_directory()?.join("salida_rng");
        let directory = create_child(
            window, instance, "EDIT", &default_path.display().to_string(),
            WS_BORDER | WS_TABSTOP, [20, 216, 685, 30], 0,
        )?;
        ui.directory.store(directory, Ordering::SeqCst);

        let reuse = create_child(
            window, instance, "BUTTON", "Reutilizar catalogo de 128000 tipos si existe",
            BS_AUTOCHECKBOX | WS_TABSTOP, [20, 272, 680, 27], 0,
        )?;
        ui.reuse.store(reuse, Ordering::SeqCst);
        unsafe {
            SendMessageW(as_handle(reuse), BM_SETCHECK, BST_CHECKED, 0);
        }
        let compact = create_child(
            window, instance, "BUTTON", "Compacto: 100 IDs por paquete (permite repetidos)",
            BS_AUTOCHECKBOX | WS_TABSTOP, [20, 304, 680, 27], 0,
        )?;
        ui.compact.store(compact, Ordering::SeqCst);
        // El formato compacto es el recomendado para millones de apuestas.
        unsafe {
            SendMessageW(as_handle(compact), BM_SETCHECK, BST_CHECKED, 0);
        }
        ui.split.store(
            create_child(
                window, instance, "BUTTON", "Ademas, guardar un JSON individual por paquete",
                BS_AUTOCHECKBOX | WS_TABSTOP, [20, 337, 680, 27], 0,
            )?,
            Ordering::SeqCst,
        );
        ui.run_button.store(
            create_child(
                window, instance, "BUTTON", "GENERAR PAQUETES", WS_TABSTOP,
                [20, 383, 320, 45], BTN_START,
            )?,
            Ordering::SeqCst,
        );
        let cancel = create_child(
            window, instance, "BUTTON", "CANCELAR", WS_TABSTOP,
            [360, 383, 140, 45], BTN_CANCEL,
        )?;
        ui.cancel_button.store(cancel, Ordering::SeqCst);
        unsafe {
            EnableWindow(as_handle(cancel), 0);
        }

        ui.status.store(
            create_child(
                window, instance, "STATIC", "Listo para generar paquetes.",
                0, [20, 443, 690, 25], 0,
            )?,
            Ordering::SeqCst,
        );
        ui.report.store(
            create_child(
                window,
                instance,
                "EDIT",
                "Ingresa la cantidad de paquetes y presiona GENERAR.\r\n\
                 El catalogo de 128000 tipos se guarda en catalogo_128000.json.\r\n\
                 Los paquetes de 100 se guardan en paquetes.jsonl.",
                WS_BORDER | ES_MULTILINE | ES_READONLY | ES_AUTOVSCROLL | WS_VSCROLL,
                [20, 475, 685, 80],
                0,
            )?,
            Ordering::SeqCst,
        );
        Ok(())
    }

    /// Test complete startup and the same generation path used by the GUI button.
    /// Runs without opening an interactive desktop window, for GitHub Actions.
    fn self_test() -> Result<(), String> {
        let directory = executable_directory()?;
        if !directory.join("RNG-6x6-CLI.exe").is_file() {
            return Err("falta RNG-6x6-CLI.exe junto al ejecutable de la GUI".to_owned());
        }
        let window = create_window()?;
        let ui = state();

        let result = (|| {
            if ui.packages.load(Ordering::SeqCst) == 0
                || ui.directory.load(Ordering::SeqCst) == 0
                || ui.run_button.load(Ordering::SeqCst) == 0
            {
                return Err("no se crearon todos los controles".to_owned());
            }
            let output = std::env::temp_dir().join(format!(
                "rng native gui smoke {}",
                std::process::id()
            ));
            write_text(ui.packages.load(Ordering::SeqCst), "3");
            write_text(
                ui.directory.load(Ordering::SeqCst),
                &output.display().to_string(),
            );

            // Exercise the exact path invoked when the user presses GENERAR PAQUETES.
            start_generation(window)?;
            let deadline = Instant::now() + Duration::from_secs(40);
            let result = loop {
                if let Some(completion) = ui
                    .completion
                    .lock()
                    .map_err(|error| error.to_string())?
                    .take()
                {
                    break completion;
                }
                if Instant::now() >= deadline {
                    cancel_generation();
                    return Err("La generacion de tres paquetes no finalizo".to_owned());
                }
                thread::sleep(Duration::from_millis(100));
            };
            if !result.ok {
                return Err(format!("La GUI no pudo completar la generacion:\n{}", result.text));
            }
            let catalog_path = output.join("catalogo_128000.json");
            let catalog: rng_grid::Catalog = serde_json::from_slice(
                &fs::read(&catalog_path)
                    .map_err(|error| format!("Falta el catalogo de resultados: {error}"))?,
            )
            .map_err(|error| error.to_string())?;
            rng_grid::validate_catalog(&catalog)?;
            let packages = fs::read_to_string(output.join("paquetes.jsonl"))
                .map_err(|error| format!("Falta paquetes.jsonl: {error}"))?;
            let lines: Vec<&str> = packages.lines().collect();
            if lines.len() != 3 {
                return Err(format!("La GUI genero {} paquetes en lugar de 3", lines.len()));
            }
            for line in lines {
                let entry: serde_json::Value =
                    serde_json::from_str(line).map_err(|error| error.to_string())?;
                let ids = entry["outcome_ids"]
                    .as_array()
                    .ok_or("Falta la lista de IDs del paquete")?;
                if ids.len() != 100 || ids.iter().any(|value| {
                    !matches!(value.as_u64(), Some(id) if (1..=128000).contains(&id))
                }) {
                    return Err("El paquete no contiene 100 IDs validos entre 1 y 128000".to_owned());
                }
            }
            fs::remove_dir_all(output)
                .map_err(|error| format!("No se pudo limpiar prueba: {error}"))?;
            Ok(())
        })();

        // SAFETY: this window was created on this thread and owns the controls above.
        unsafe {
            DestroyWindow(window);
        }
        result
    }

    pub fn run() {
        if std::env::args().any(|arg| arg == "--self-test") {
            if let Err(error) = self_test() {
                let path = std::env::temp_dir().join("rng-gui-self-test-error.txt");
                let _ = fs::write(&path, &error);
                eprintln!("{error}");
                std::process::exit(1);
            }
            return;
        }

        match create_window() {
            Ok(window) => {
                // SAFETY: window is live, and all controls are initialized above.
                unsafe {
                    ShowWindow(window, 1);
                    UpdateWindow(window);
                    let mut msg = Msg {
                        hwnd: ptr::null_mut(),
                        message: 0,
                        wparam: 0,
                        lparam: 0,
                        time: 0,
                        pt: Point::default(),
                        private: 0,
                    };
                    while GetMessageW(&mut msg, ptr::null_mut(), 0, 0) > 0 {
                        TranslateMessage(&msg);
                        DispatchMessageW(&msg);
                    }
                }
            }
            Err(error) => {
                message_box(ptr::null_mut(), &error, true);
                std::process::exit(1);
            }
        }
    }
}

#[cfg(windows)]
fn main() {
    windows_gui::run();
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Esta interfaz grafica nativa solo se ejecuta en Windows.");
}
