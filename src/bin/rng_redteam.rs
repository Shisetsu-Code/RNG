use rng_grid::audit::{CHECKPOINTS, Mode, ModeReport, Trace, audit, validate_trace};
use rng_grid::{Catalog, generate_catalog, validate_catalog};
use serde::Serialize;
use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[cfg(feature = "gpu")]
#[path = "../redteam_gpu.rs"]
mod gpu;

#[derive(Debug, Clone, Copy)]
enum Selection {
    All,
    Without,
    Independent,
    Weak,
}

#[derive(Serialize)]
struct SecurityReport {
    version: u32,
    tested_packages_per_mode: u64,
    generated_bets_per_mode: u128,
    catalog_size: usize,
    catalog_rtp_percent: f64,
    gpu: Option<GpuReport>,
    runs: Vec<ModeReport>,
    overall_runtime_seconds: f64,
    notes: Vec<String>,
}

#[derive(Default, Serialize)]
struct GpuReport {
    backend: String,
    adapter: String,
    score_samples: u64,
    benchmark_compute_seconds: f64,
    cpu_gpu_checks: u64,
    cpu_gpu_mismatches: u64,
    checkpoint_mean_conditional_rtp_percent: Vec<f64>,
    positive_ev_count_by_checkpoint: Vec<u64>,
}

struct Options {
    packages: u64,
    mode: Selection,
    output: PathBuf,
    catalog: Option<PathBuf>,
    gpu_enabled: bool,
    gpu_batch: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            packages: 10_000,
            mode: Selection::All,
            output: PathBuf::from("auditoria_rng"),
            catalog: None,
            gpu_enabled: false,
            gpu_batch: 50_000,
        }
    }
}

fn usage() {
    println!(
        "LABORATORIO RNG 6x6: GENERADOR vs ATACANTE\n\
         \n\
         Uso: rng_redteam.exe [opciones]\n\
         --packages N               Paquetes de 100 apuestas por modelo (default: 10000)\n\
         --mode all                  3 modos: seguro sin reemplazo, independiente y control debil\n\
         --mode without              Solo paquetes actuales, 100 sin reemplazo\n\
         --mode independent          100 sorteos independientes con reemplazo\n\
         --mode weak                 Control deliberadamente inseguro (secuencia +1)\n\
         --catalog archivo.json      Reutiliza catalogo_128.json existente\n\
         --out carpeta               Directorio de informes (default: auditoria_rng)\n\
         --gpu                       Analisis GPU adicional: wgpu/DX12/Vulkan\n\
         --gpu-batch N               Tamaño de lote GPU en paquetes (default: 50000)\n\
         --help                      Muestra esta ayuda\n\
         \n\
         Los ataques solo usan resultados ANTERIORES y el catalogo publico.\n\
         No se explotan servidores reales ni se usan credenciales.\n\
         GPU nunca genera semillas, claves ni resultados de produccion."
    );
}

fn parse_args() -> Result<Option<Options>, Box<dyn Error>> {
    let mut options = Options::default();
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                usage();
                return Ok(None);
            }
            "--packages" => {
                options.packages = args
                    .next()
                    .ok_or("falta numero de paquetes")?
                    .parse()?;
            }
            "--mode" => {
                options.mode = match args.next().ok_or("falta nombre de modo")?.as_str() {
                    "all" => Selection::All,
                    "without" => Selection::Without,
                    "independent" => Selection::Independent,
                    "weak" => Selection::Weak,
                    other => return Err(format!("modo invalido: {other}").into()),
                };
            }
            "--out" => {
                options.output =
                    PathBuf::from(args.next().ok_or("falta carpeta de salida")?);
            }
            "--catalog" => {
                options.catalog = Some(PathBuf::from(
                    args.next().ok_or("falta archivo de catalogo")?,
                ));
            }
            "--gpu" => options.gpu_enabled = true,
            "--gpu-batch" => {
                options.gpu_batch = args
                    .next()
                    .ok_or("falta tamano del lote GPU")?
                    .parse()?;
            }
            _ => return Err(format!("opcion desconocida: {arg}").into()),
        }
    }
    if options.packages == 0 || options.gpu_batch == 0 {
        return Err("--packages y --gpu-batch deben ser positivos".into());
    }
    if options.gpu_batch > 100_000 {
        return Err("--gpu-batch debe ser como maximo 100000 (limite de memoria)".into());
    }
    if options.gpu_enabled && matches!(options.mode, Selection::Weak | Selection::Independent) {
        return Err("GPU solo puntua el modelo sin reemplazo; usa --mode without o all".into());
    }
    Ok(Some(options))
}

fn load_catalog(path: Option<&Path>) -> Result<Catalog, Box<dyn Error>> {
    let mut rng = rand::rng();
    let catalog = if let Some(path) = path {
        serde_json::from_reader(BufReader::new(File::open(path)?))?
    } else {
        generate_catalog(&mut rng)?
    };
    validate_catalog(&catalog)?;
    Ok(catalog)
}

fn modes(choice: Selection) -> &'static [Mode] {
    const ALL: &[Mode] = &[
        Mode::WithoutReplacement,
        Mode::WithReplacement,
        Mode::WeakCounter,
    ];
    match choice {
        Selection::All => ALL,
        Selection::Without => &[Mode::WithoutReplacement],
        Selection::Independent => &[Mode::WithReplacement],
        Selection::Weak => &[Mode::WeakCounter],
    }
}

fn write_csv(reports: &[ModeReport], directory: &Path) -> Result<(), Box<dyn Error>> {
    let mut csv = csv_writer::Writer::new(File::create(directory.join("predicciones.csv"))?);
    csv.write(&[
        "modelo", "prefijo_observado", "tipos_posibles", "muestras",
        "acierto_ciego_pct", "acierto_eliminacion_pct",
        "acierto_incremental_pct", "esperado_eliminacion_pct",
        "rtp_condicional_promedio_pct", "oportunidades_ev_positivo",
        "rtp_teorico_solo_apuestas_positivas_pct",
        "rtp_observado_solo_apuestas_positivas_pct",
    ])?;
    for report in reports {
        for checkpoint in &report.checkpoints {
            csv.write(&[
                report.mode.label().to_owned(),
                checkpoint.observed_prefix.to_string(),
                checkpoint.possible_next_types.to_string(),
                checkpoint.trials.to_string(),
                format!("{:.5}", checkpoint.blind_success_percent),
                format!("{:.5}", checkpoint.elimination_success_percent),
                format!("{:.5}", checkpoint.increment_predictor_success_percent),
                format!("{:.5}", checkpoint.theoretical_elimination_percent),
                format!("{:.5}", checkpoint.mean_conditional_rtp_percent),
                checkpoint.conditional_positive_ev_opportunities.to_string(),
                checkpoint.selective_theoretical_rtp_percent
                    .map(|x| format!("{x:.5}")).unwrap_or_default(),
                checkpoint.selective_realized_rtp_percent
                    .map(|x| format!("{x:.5}")).unwrap_or_default(),
            ])?;
        }
    }
    Ok(())
}

mod csv_writer {
    use std::io::{BufWriter, Write};
    pub struct Writer<W: Write>(BufWriter<W>);

    impl<W: Write> Writer<W> {
        pub fn new(inner: W) -> Self {
            Self(BufWriter::new(inner))
        }
        pub fn write<S: AsRef<str>>(&mut self, cells: &[S]) -> std::io::Result<()> {
            for (index, cell) in cells.iter().enumerate() {
                if index != 0 {
                    self.0.write_all(b",")?;
                }
                // CSV values in this report are controlled numeric data/enum names.
                self.0.write_all(cell.as_ref().as_bytes())?;
            }
            self.0.write_all(b"\n")
        }
    }
}

#[cfg(feature = "gpu")]
struct GpuSession<'a> {
    analyzer: gpu::GpuAnalyzer,
    catalog: &'a Catalog,
    batch_size: usize,
    traces: Vec<Trace>,
    elapsed: f64,
    checkpoints_sums: [u128; CHECKPOINTS.len()],
    positive: [u64; CHECKPOINTS.len()],
    compared: u64,
    mismatches: u64,
    processed: u64,
}

#[cfg(feature = "gpu")]
impl<'a> GpuSession<'a> {
    fn new(catalog: &'a Catalog, batch_size: usize) -> Result<Self, String> {
        Ok(Self {
            analyzer: gpu::GpuAnalyzer::new()?,
            catalog,
            batch_size,
            traces: Vec::with_capacity(batch_size),
            elapsed: 0.0,
            checkpoints_sums: [0; CHECKPOINTS.len()],
            positive: [0; CHECKPOINTS.len()],
            compared: 0,
            mismatches: 0,
            processed: 0,
        })
    }

    fn feed(&mut self, trace: &Trace) -> Result<(), String> {
        self.traces.push(*trace);
        if self.traces.len() >= self.batch_size {
            self.flush()?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), String> {
        if self.traces.is_empty() {
            return Ok(());
        }
        let now = Instant::now();
        let gpu_results = self.analyzer.compute(&self.traces, self.catalog)?;
        self.elapsed += now.elapsed().as_secs_f64();
        let count = self.traces.len();
        for (i, values) in gpu_results.iter().enumerate() {
            for (c, &ev) in values.iter().enumerate() {
                self.checkpoints_sums[c] += u128::from(ev);
                if ev > 10_000 {
                    self.positive[c] += 1;
                }
            }
            // Validate GPU results against CPU for up to the first 1024 traces of the run.
            if self.compared < 1024 {
                self.compared += 1;
                for (c, &checkpoint) in CHECKPOINTS.iter().enumerate() {
                    let expected = rng_grid::audit::conditional_bps(
                        &self.traces[i],
                        self.catalog,
                        checkpoint,
                    );
                    if expected != values[c] {
                        self.mismatches += 1;
                    }
                }
            }
        }
        self.processed += count as u64;
        self.traces.clear();
        if self.mismatches > 0 {
            return Err(format!(
                "GPU devolvio {} valores distintos de CPU: resultados descartados",
                self.mismatches
            ));
        }
        Ok(())
    }

    fn report(mut self) -> Result<GpuReport, String> {
        self.flush()?;
        Ok(GpuReport {
            backend: "wgpu compute WGSL (DX12/Vulkan)".to_owned(),
            adapter: self.analyzer.device_name().to_owned(),
            score_samples: self.processed,
            benchmark_compute_seconds: self.elapsed,
            cpu_gpu_checks: self.compared * CHECKPOINTS.len() as u64,
            cpu_gpu_mismatches: self.mismatches,
            checkpoint_mean_conditional_rtp_percent: self.checkpoints_sums
                .iter()
                .map(|&total| {
                    100.0 * total as f64
                        / (self.processed as f64 * 10_000.0)
                })
                .collect(),
            positive_ev_count_by_checkpoint: self.positive.to_vec(),
        })
    }
}

fn run(options: Options) -> Result<(), Box<dyn Error>> {
    if options.gpu_enabled {
        #[cfg(not(feature = "gpu"))]
        {
            return Err("El ejecutable no tiene backend GPU. \
                        Instala la version Windows con GPU o compila: \
                        cargo run --release --features gpu --bin rng_redteam -- --gpu"
                .into());
        }
    }
    let started = Instant::now();
    fs::create_dir_all(&options.output)?;
    let catalog = load_catalog(options.catalog.as_deref())?;
    let mut reports = Vec::new();
    let mut gpu_report = None;
    let mut rng = rand::rng();

    println!("LABORATORIO: GENERADOR vs ATACANTE");
    println!("128 matrices 6x6 | RTP teorico: 96,5% | {} paquetes por modelo", options.packages);
    for &mode in modes(options.mode) {
        println!("\nAnalizando modelo {}...", mode.label());

        #[cfg(feature = "gpu")]
        if options.gpu_enabled && mode == Mode::WithoutReplacement {
            let mut gpu_session = GpuSession::new(&catalog, options.gpu_batch)?;
            println!("GPU detectada: {}", gpu_session.analyzer.device_name());
            let report = audit(mode, options.packages, &catalog, &mut rng, |trace| {
                validate_trace(trace, mode)?;
                gpu_session.feed(trace)
            })?;
            gpu_report = Some(gpu_session.report()?);
            reports.push(report);
            continue;
        }

        let report = audit(mode, options.packages, &catalog, &mut rng, |trace| {
            validate_trace(trace, mode)
        })?;
        reports.push(report);
    }

    let notes = vec![
        "RNG criptografico != proceso de apuestas seguro. \
         No repetir IDs dentro de 100 revela exclusiones a un atacante."
            .to_owned(),
        "Si el jugador conoce tipos anteriores y decide cuando apostar, \
         puede seleccionar condiciones con retorno esperado superior a 100%."
            .to_owned(),
        "No revelar los 100 resultados futuros ni el estado/semilla del RNG. \
         El servidor debe validar y fijar la apuesta ANTES de sortear/servir el resultado."
            .to_owned(),
        "Alternativa defensiva: muestreo independiente con reemplazo. \
         Cada apuesta tiene 128 posibilidades y RTP condicional fijo en 96,5%."
            .to_owned(),
        "Las pruebas estadisticas NO demuestran resistencia criptografica \
         ni sustituyen auditoria y certificacion del producto."
            .to_owned(),
        "La GPU solo acelera el analisis de lotes, nunca genera semillas \
         ni resultados financieros de produccion."
            .to_owned(),
    ];

    let report = SecurityReport {
        version: 1,
        tested_packages_per_mode: options.packages,
        generated_bets_per_mode: u128::from(options.packages) * 100,
        catalog_size: 128,
        catalog_rtp_percent: 96.5,
        gpu: gpu_report,
        runs: reports,
        overall_runtime_seconds: started.elapsed().as_secs_f64(),
        notes,
    };
    let output = options.output.join("auditoria.json");
    serde_json::to_writer_pretty(File::create(&output)?, &report)?;
    write_csv(&report.runs, &options.output)?;

    for model in &report.runs {
        println!(
            "\nModelo: {} | Giros: {} | RTP observado {:.4}% | Tiempo {:.3}s",
            model.mode.label(),
            model.draws,
            model.observed_rtp_percent,
            model.elapsed_seconds
        );
        for point in &model.checkpoints {
            println!(
                "  Observados {:>2} | posibles {:>3} | acierto por exclusion: {:>6.3}% \
                 (esperado {:>6.3}%) | oportunidades EV >100%: {}",
                point.observed_prefix,
                point.possible_next_types,
                point.elimination_success_percent,
                point.theoretical_elimination_percent,
                point.conditional_positive_ev_opportunities
            );
        }
        if model.structural_exploit_detected {
            println!("  ALERTA: fuga estructural de informacion por no reposicion");
        }
        if model.weak_rng_predictable {
            println!("  CONTROL: predictor incremental acerto 100% (RNG intencionalmente debil)");
        }
    }
    if let Some(gpu) = &report.gpu {
        println!(
            "\nGPU: {} | {} paquetes | {:.3}s computacion + transferencias | \
             {} comparaciones CPU/GPU, {} errores",
            gpu.adapter, gpu.score_samples, gpu.benchmark_compute_seconds,
            gpu.cpu_gpu_checks, gpu.cpu_gpu_mismatches
        );
    }
    println!("\nInforme: {}", output.canonicalize()?.display());
    println!(
        "CSV de predicciones: {}",
        options.output.join("predicciones.csv").canonicalize()?.display()
    );
    Ok(())
}

fn main() {
    match parse_args().and_then(|options| {
        if let Some(options) = options {
            run(options)?;
        }
        Ok(())
    }) {
        Ok(()) => {}
        Err(error) => {
            eprintln!("ERROR: {error}");
            std::process::exit(1);
        }
    }
}
