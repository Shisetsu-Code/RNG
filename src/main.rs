use rng_grid::{
    BETS_PER_PACKAGE, BASIS_POINTS, Catalog, Outcome, Package, RTP_TARGET_BPS, TYPES,
    generate_catalog, generate_package, theoretical_rtp_bps, validate_catalog,
};
use serde::Serialize;
use serde_json::json;
use std::env;
use std::error::Error;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug)]
struct Options {
    packages: usize,
    output: PathBuf,
    reuse_catalog: Option<PathBuf>,
    compact: bool,
    split_files: bool,
}

#[derive(Serialize)]
struct FullPackage<'a> {
    package: usize,
    bets: Vec<&'a Outcome>,
    total_payout_bps: u64,
}

fn help() {
    println!(
        "RNG 6x6 | 128000 tipos de resultado | 100 apuestas por paquete | RTP 96,5%\n\
         \n\
         Uso: rng-grid.exe --packages N [opciones]\n\
         --packages N          Cantidad de paquetes de 100 apuestas\n\
         --out DIRECTORIO      Donde guardar resultados (default: salida_rng)\n\
         --reuse-catalog JSON  Utilizar el catalogo previo de 128000 matrices\n\
         --compact             Guardar solo IDs (predeterminado)\n\
         --full                Incluir las matrices 6x6 en cada apuesta\n\
         --split-files         Ademas, generar paquete_000001.json, etc.\n\
         --help                Mostrar esta ayuda\n\
         \n\
         Sin argumentos pregunta la cantidad de paquetes.\n\
         Cada paquete contiene 100 extracciones independientes, uniformes\n\
         sobre 128000 tipos. Los IDs pueden aparecer varias veces.\n\
         El RTP de 96,5% es matematico; NO se fuerza por paquete.\n\
         No existen cuentas ni saldos en este programa."
    );
}

fn prompt_packages() -> Result<usize, Box<dyn Error>> {
    println!("==========================================");
    println!(" RNG 6x6 | 128000 tipos | RTP 96,5%");
    println!(" 100 apuestas en cada paquete");
    println!("==========================================");
    print!("Cuantos paquetes de 100 apuestas queres generar? [100]: ");
    io::stdout().flush()?;
    let mut input = String::new();
    if io::stdin().read_line(&mut input)? == 0 {
        return Err("no se recibio la cantidad de paquetes".into());
    }
    let value = input.trim();
    let count = if value.is_empty() {
        100
    } else {
        value.parse::<usize>()?
    };
    if count == 0 {
        return Err("la cantidad de paquetes debe ser mayor que cero".into());
    }
    Ok(count)
}

fn parse_args() -> Result<(Options, bool), Box<dyn Error>> {
    let mut args = env::args().skip(1).peekable();
    let interactive = args.peek().is_none();
    let mut count = None;
    let mut output = PathBuf::from("salida_rng");
    let mut reuse_catalog = None;
    let mut compact = true;
    let mut split_files = false;
    if interactive {
        count = Some(prompt_packages()?);
    } else {
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => {
                    help();
                    return Ok((
                        Options {
                            packages: 0,
                            output,
                            reuse_catalog,
                            compact,
                            split_files,
                        },
                        false,
                    ));
                }
                "--packages" | "--paquetes" => {
                    if count.is_some() {
                        return Err("solo se puede especificar --packages una vez".into());
                    }
                    count = Some(
                        args.next()
                            .ok_or("falta un numero despues de --packages")?
                            .parse::<usize>()?,
                    );
                }
                "--out" => {
                    output = PathBuf::from(
                        args.next().ok_or("falta directorio despues de --out")?,
                    );
                }
                "--reuse-catalog" => {
                    reuse_catalog = Some(PathBuf::from(
                        args.next().ok_or("falta archivo despues de --reuse-catalog")?,
                    ));
                }
                "--compact" => compact = true,
                "--full" => compact = false,
                "--split-files" => split_files = true,
                _ => {
                    return Err(format!("opcion desconocida: {arg}; ejecuta --help").into());
                }
            }
        }
    }
    let packages = count.ok_or("indica --packages N")?;
    if packages == 0 {
        return Err("la cantidad de paquetes debe ser mayor que cero".into());
    }
    Ok((
        Options {
            packages,
            output,
            reuse_catalog,
            compact,
            split_files,
        },
        interactive,
    ))
}

fn write_catalog(catalog: &Catalog, output: &Path) -> Result<(), Box<dyn Error>> {
    let file = BufWriter::new(File::create(output.join("catalogo_128000.json"))?);
    serde_json::to_writer_pretty(file, catalog)?;

    let mut csv = BufWriter::new(File::create(output.join("catalogo_128000.csv"))?);
    writeln!(
        csv,
        "tipo_id,parejas_horizontales,retorno_bps,retorno_multiplicador,fila_1,fila_2,fila_3,fila_4,fila_5,fila_6"
    )?;
    for outcome in &catalog.outcomes {
        let rows: Vec<String> = outcome
            .grid
            .iter()
            .map(|row| {
                row.iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join("-")
            })
            .collect();
        writeln!(
            csv,
            "{},{},{},{:.4},{}",
            outcome.id,
            outcome.matching_pairs,
            outcome.payout_bps,
            outcome.payout_bps as f64 / BASIS_POINTS as f64,
            rows.join(",")
        )?;
    }
    csv.flush()?;
    Ok(())
}

fn encode_package<W: Write>(
    writer: &mut W,
    package: &Package,
    catalog: &Catalog,
    compact: bool,
) -> Result<(), Box<dyn Error>> {
    if compact {
        serde_json::to_writer(writer, package)?;
    } else {
        let full = FullPackage {
            package: package.package,
            bets: package
                .outcome_ids
                .iter()
                .map(|&id| &catalog.outcomes[id as usize - 1])
                .collect(),
            total_payout_bps: package.total_payout_bps,
        };
        serde_json::to_writer(writer, &full)?;
    }
    Ok(())
}

fn run(options: Options) -> Result<(), Box<dyn Error>> {
    let mut rng = rand::rng();
    let start = Instant::now();
    let catalog = if let Some(path) = &options.reuse_catalog {
        let reader = BufReader::new(File::open(path)?);
        let catalog: Catalog = serde_json::from_reader(reader)?;
        validate_catalog(&catalog)?;
        catalog
    } else {
        generate_catalog(&mut rng)?
    };
    if theoretical_rtp_bps() != RTP_TARGET_BPS {
        return Err("RTP no coincide con 96,5%".into());
    }
    fs::create_dir_all(&options.output)?;
    write_catalog(&catalog, &options.output)?;

    let package_path = options.output.join("paquetes.jsonl");
    let mut jsonl = BufWriter::new(File::create(&package_path)?);
    let mut summary = BufWriter::new(File::create(options.output.join("resumen_paquetes.csv"))?);
    writeln!(
        summary,
        "paquete,apuestas,tipos_unicos,tipos_repetidos,retorno_en_unidades_de_apuesta,rtp_observado_pct"
    )?;

    let individual_dir = options.output.join("paquetes");
    if options.split_files {
        fs::create_dir_all(&individual_dir)?;
    }

    let mut total_return_bps = 0_u128;
    let mut frequencies = vec![0_u64; TYPES];
    let mut last_seen_in_package = vec![0_usize; TYPES];
    let mut duplicates_within_packages = 0_u128;
    let mut packages_with_duplicates = 0_usize;

    println!("Catalogo: {TYPES} matrices distintas de 6x6");
    println!("RTP teorico: {:.4}%", RTP_TARGET_BPS as f64 / 100.0);
    println!(
        "Generando {} paquetes de {} apuestas ({} apuestas solicitadas)...",
        options.packages,
        BETS_PER_PACKAGE,
        (options.packages as u128) * BETS_PER_PACKAGE as u128,
    );
    io::stdout().flush()?;

    for index in 1..=options.packages {
        let package = generate_package(&catalog, index, &mut rng);
        debug_assert_eq!(package.outcome_ids.len(), BETS_PER_PACKAGE);
        let mut unique_in_package = 0_usize;
        for &id in &package.outcome_ids {
            let key = id as usize - 1;
            let slot = &mut frequencies[key];
            *slot = slot.checked_add(1).ok_or("desbordamiento de frecuencias")?;
            if last_seen_in_package[key] != index {
                last_seen_in_package[key] = index;
                unique_in_package += 1;
            }
        }
        let repeated = BETS_PER_PACKAGE - unique_in_package;
        duplicates_within_packages += repeated as u128;
        if repeated > 0 {
            packages_with_duplicates += 1;
        }
        total_return_bps = total_return_bps
            .checked_add(u128::from(package.total_payout_bps))
            .ok_or("desbordamiento de retorno")?;

        encode_package(&mut jsonl, &package, &catalog, options.compact)?;
        jsonl.write_all(b"\n")?;
        writeln!(
            summary,
            "{},{},{},{},{:.4},{:.4}",
            index,
            BETS_PER_PACKAGE,
            unique_in_package,
            repeated,
            package.total_payout_bps as f64 / BASIS_POINTS as f64,
            package.observed_rtp_percent()
        )?;

        if options.split_files {
            let file_name = format!("paquete_{index:06}.json");
            let mut out = BufWriter::new(File::create(individual_dir.join(file_name))?);
            encode_package(&mut out, &package, &catalog, options.compact)?;
            out.write_all(b"\n")?;
            out.flush()?;
        }

        if index % 1000 == 0 {
            println!("Paquetes completados: {index}/{}", options.packages);
            io::stdout().flush()?;
        }
    }
    jsonl.flush()?;
    summary.flush()?;

    let mut freq_file = BufWriter::new(File::create(options.output.join("frecuencias_tipos.csv"))?);
    writeln!(freq_file, "tipo_id,veces_seleccionado,retorno_bps,retorno_multiplicador")?;
    for (outcome, frequency) in catalog.outcomes.iter().zip(frequencies.iter()) {
        writeln!(
            freq_file,
            "{},{},{},{:.4}",
            outcome.id,
            frequency,
            outcome.payout_bps,
            outcome.payout_bps as f64 / BASIS_POINTS as f64
        )?;
    }
    freq_file.flush()?;

    let total_bets = options.packages as u128 * BETS_PER_PACKAGE as u128;
    let rtp_observed_percent =
        total_return_bps as f64 * 100.0 / (total_bets as f64 * BASIS_POINTS as f64);
    let elapsed = start.elapsed();
    let expected_per_type = total_bets as f64 / TYPES as f64;
    let std_deviation = (expected_per_type * (1.0 - 1.0 / TYPES as f64)).sqrt();
    let min_frequency = frequencies.iter().copied().min().unwrap_or_default();
    let max_frequency = frequencies.iter().copied().max().unwrap_or_default();
    let covered_types = frequencies.iter().filter(|&&frequency| frequency > 0).count();
    let max_abs_z = frequencies
        .iter()
        .map(|&frequency| (frequency as f64 - expected_per_type).abs() / std_deviation)
        .fold(0.0_f64, f64::max);
    let report = json!({
        "catalog_outcomes": TYPES,
        "grid_size": [6, 6],
        "package_count": options.packages,
        "bets_per_package": BETS_PER_PACKAGE,
        "total_bets": total_bets.to_string(),
        "theoretical_rtp_percent": 96.5,
        "observed_rtp_percent": rtp_observed_percent,
        "total_return_in_bet_units": total_return_bps as f64 / BASIS_POINTS as f64,
        "sampling": "100 independent uniform CSPRNG draws from 128000 types, with replacement",
        "types_observed": covered_types,
        "expected_occurrences_per_type": expected_per_type,
        "minimum_occurrences_per_type": min_frequency,
        "maximum_occurrences_per_type": max_frequency,
        "maximum_frequency_deviation_z": max_abs_z,
        "duplicate_draws_within_packages": duplicates_within_packages.to_string(),
        "packages_with_duplicate_types": packages_with_duplicates,
        "compact_package_references": options.compact,
        "split_files": options.split_files,
        "elapsed_seconds": elapsed.as_secs_f64()
    });
    let output_report = BufWriter::new(File::create(options.output.join("reporte.json"))?);
    serde_json::to_writer_pretty(output_report, &report)?;

    println!();
    println!("Paquetes generados: {}", options.packages);
    println!("Apuestas generadas: {total_bets}");
    println!("Tipos posibles por apuesta: {TYPES} (con reposicion)");
    println!("Tipos presentes: {covered_types}/{TYPES} | Frecuencias min/max: {min_frequency}/{max_frequency} | Esperado: {expected_per_type:.3}");
    println!("Maxima desviacion normalizada de frecuencia: {max_abs_z:.3} sigmas");
    println!("Repeticiones dentro de paquetes: {duplicates_within_packages} en {packages_with_duplicates} paquetes");
    println!("RTP teorico: 96,5000%");
    println!("RTP observado: {rtp_observed_percent:.4}%");
    println!("Retorno total: {:.4} unidades de apuesta", total_return_bps as f64 / BASIS_POINTS as f64);
    println!("Tiempo: {:.3} segundos", elapsed.as_secs_f64());
    println!("Directorio: {}", options.output.canonicalize()?.display());
    println!("Archivos: catalogo_128000.json, catalogo_128000.csv, paquetes.jsonl,");
    println!("          resumen_paquetes.csv, frecuencias_tipos.csv, reporte.json");
    if options.split_files {
        println!("          + {} archivos individuales en /paquetes", options.packages);
    }
    Ok(())
}

fn main() {
    let direct_interactive = env::args_os().len() == 1 && io::stdin().is_terminal();
    let result = parse_args().and_then(|(options, interactive)| {
        if options.packages == 0 {
            return Ok(());
        }
        if interactive {
            println!();
        }
        run(options)
    });
    if let Err(error) = result {
        eprintln!("\nERROR: {error}");
        if direct_interactive {
            eprintln!("Revisa la carpeta y las opciones. Pulsa Enter para cerrar.");
            let mut line = String::new();
            let _ = io::stdin().read_line(&mut line);
        }
        std::process::exit(1);
    }
    if direct_interactive {
        println!("\nPresiona Enter para cerrar.");
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
    }
}
