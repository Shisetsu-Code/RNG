use rng_grid::{
    BetCount, Config, Simulation, WalletReport, exact_rtp, simulate_with_count,
};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufWriter, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

fn usage() {
    println!(
        "RNG 6x6 | 128 saldos | RTP teorico 96,5%\n\
         \n\
         Uso: RNG-6x6.exe (Windows: abre el menu interactivo)\n\
              cargo run --release -- [opciones]\n\
         --total-spins N     N apuestas EN TOTAL, repartidas entre 128 saldos\n\
         --bets N            Alias de --total-spins\n\
         --spins N           N apuestas POR SALDO (hasta 128 x N en total)\n\
         --bet N             Importe por apuesta, multiplo de 48 (default: 48)\n\
         --base-balance N    Saldo inicial del primero de los 128 (default: 1000000)\n\
         --balance-step N    Incremento positivo entre saldos (default: 10000)\n\
         --csv RUTA          Exporta el resultado de los 128 saldos a CSV\n\
         --summary-only      Omite las 128 lineas de detalle de saldos\n\
         --interactive       Abre el menu interactivo\n\
         -h, --help          Muestra esta ayuda\n\
         \n\
         Si no pasas parametros, aparece el menu interactivo.\n\
         El saldo no altera el azar ni los premios, solo habilita apuestas."
    );
}

fn next_number<T>(
    args: &mut impl Iterator<Item = String>,
    option: &str,
) -> Result<T, Box<dyn Error>>
where
    T: std::str::FromStr,
    T::Err: Error + 'static,
{
    let value = args
        .next()
        .ok_or_else(|| format!("falta el valor de {option}"))?;
    Ok(value.parse::<T>()?)
}

fn prompt_line(message: &str) -> Result<String, Box<dyn Error>> {
    print!("{message}");
    io::stdout().flush()?;
    let mut line = String::new();
    if io::stdin().read_line(&mut line)? == 0 {
        return Err("no se recibio ninguna entrada".into());
    }
    Ok(line.trim().to_string())
}

fn prompt_number<T>(description: &str, default: T, minimum: T) -> Result<T, Box<dyn Error>>
where
    T: std::fmt::Display + std::str::FromStr + Copy + PartialOrd,
{
    loop {
        let value = prompt_line(&format!("{description} [{default}]: "))?;
        let number = if value.is_empty() {
            Some(default)
        } else {
            value.parse::<T>().ok()
        };
        match number {
            Some(n) if n >= minimum => return Ok(n),
            _ => println!("Valor invalido. Ingresa un entero mayor o igual a {minimum}."),
        }
    }
}

fn prompt_yes_no(description: &str, default: bool) -> Result<bool, Box<dyn Error>> {
    let suffix = if default { "S/n" } else { "s/N" };
    loop {
        let input = prompt_line(&format!("{description} [{suffix}]: "))?;
        match input.to_lowercase().as_str() {
            "" => return Ok(default),
            "s" | "si" | "sí" | "y" | "yes" => return Ok(true),
            "n" | "no" => return Ok(false),
            _ => println!("Ingresa S o N."),
        }
    }
}

fn interactive_config() -> Result<(Config, BetCount, Option<PathBuf>), Box<dyn Error>> {
    println!("==============================================");
    println!("  RNG 6x6 - Simulador de 128 saldos");
    println!("  RTP teorico: 96,5%  |  Modo local");
    println!("==============================================\n");

    let n = prompt_number::<usize>("Cantidad de apuestas", 10_000, 1)?;
    println!("\nComo repartir la cantidad?");
    println!("  1) {} apuestas EN TOTAL, distribuidas entre 128 saldos", n);
    println!("  2) {} apuestas PARA CADA SALDO (hasta {} en total)", n, (n as u128) * 128);
    let mode = loop {
        let input = prompt_line("Elegi una opcion [1]: ")?;
        match input.as_str() {
            "" | "1" => break BetCount::Total(n),
            "2" => break BetCount::PerWallet(n),
            _ => println!("Elegi 1 o 2."),
        }
    };

    let bet = loop {
        let candidate = prompt_number::<u64>(
            "\nImporte por apuesta (unidades minimas, multiplo de 48)",
            48,
            48,
        )?;
        if candidate % 48 == 0 {
            break candidate;
        }
        println!("El importe debe ser multiplo de 48.");
    };
    let base_balance = prompt_number::<u64>(
        "Saldo inicial de la primera cartera",
        1_000_000,
        0,
    )?;
    let balance_step = prompt_number::<u64>(
        "Incremento entre los 128 saldos iniciales",
        10_000,
        1,
    )?;

    let csv_path = if prompt_yes_no(
        "Guardar los resultados en resultados.csv (junto al directorio de ejecucion)",
        true,
    )? {
        Some(PathBuf::from("resultados.csv"))
    } else {
        None
    };

    println!();
    Ok((
        Config {
            spins_per_wallet: n,
            bet,
            base_balance,
            balance_step,
        },
        mode,
        csv_path,
    ))
}

fn observed_rtp(wagered: u128, returned: u128) -> f64 {
    if wagered == 0 {
        0.0
    } else {
        (returned as f64 / wagered as f64) * 100.0
    }
}

fn save_csv(path: &Path, wallets: &[WalletReport]) -> Result<(), Box<dyn Error>> {
    let mut out = BufWriter::new(File::create(path)?);
    writeln!(
        out,
        "wallet_id,initial_balance,final_balance,spins,wagered,returned,observed_rtp_percent"
    )?;
    for wallet in wallets {
        writeln!(
            out,
            "{},{},{},{},{},{},{:.5}",
            wallet.wallet_id,
            wallet.opening_balance,
            wallet.closing_balance,
            wallet.spins,
            wallet.total_bet,
            wallet.total_prize,
            observed_rtp(wallet.total_bet, wallet.total_prize)
        )?;
    }
    out.flush()?;
    Ok(())
}

fn print_example(sim: &Simulation) {
    if let Some(example) = sim.example_spin {
        println!("\nEjemplo de matriz 6x6 (primer giro):");
        for row in example.grid {
            println!(
                "  {}",
                row.iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            );
        }
        println!("Premio de cada una de las seis filas: {:?}", example.line_prizes);
        println!("Premio total del giro: {} unidades minimas", example.total_prize);
    } else {
        println!("\nNo hubo giros: todos los saldos eran insuficientes.");
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut config = Config::default();
    let mut args = env::args().skip(1).peekable();
    let mut csv_path: Option<PathBuf> = None;
    let mut summary_only = false;
    let mut count: Option<BetCount> = None;

    if args.peek().is_none() || args.peek().is_some_and(|arg| arg == "--interactive") {
        if args.peek().is_some() {
            args.next();
            if args.peek().is_some() {
                return Err("--interactive no admite argumentos adicionales".into());
            }
        }
        (config, count, csv_path) = {
            let (c, mode, csv) = interactive_config()?;
            (c, Some(mode), csv)
        };
    } else {
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "-h" | "--help" => {
                    usage();
                    return Ok(());
                }
                "--spins" => {
                    let n = next_number(&mut args, &arg)?;
                    if count.replace(BetCount::PerWallet(n)).is_some() {
                        return Err("usa --spins O --total-spins, no ambos".into());
                    }
                    config.spins_per_wallet = n;
                }
                "--total-spins" | "--bets" => {
                    let n = next_number(&mut args, &arg)?;
                    if count.replace(BetCount::Total(n)).is_some() {
                        return Err("especifica la cantidad de apuestas solo una vez".into());
                    }
                }
                "--bet" => config.bet = next_number(&mut args, &arg)?,
                "--base-balance" => config.base_balance = next_number(&mut args, &arg)?,
                "--balance-step" => config.balance_step = next_number(&mut args, &arg)?,
                "--csv" => {
                    csv_path = Some(
                        args.next()
                            .ok_or_else(|| format!("falta la ruta para {arg}"))?
                            .into(),
                    );
                }
                "--summary-only" => summary_only = true,
                _ => return Err(format!("opcion desconocida: {arg}; usa --help").into()),
            }
        }
    }

    config.validate()?;
    let count = count.unwrap_or(BetCount::PerWallet(config.spins_per_wallet));
    let maximum_requested = match count {
        BetCount::Total(n) => n,
        BetCount::PerWallet(n) => n
            .checked_mul(128)
            .ok_or("se excedio el maximo de apuestas representable")?,
    };

    let (expected_num, expected_den) = exact_rtp();
    if expected_num * 200 != expected_den * 193 {
        return Err("la tabla de premios dejo de corresponder al RTP del 96,5%".into());
    }

    let mut rng = rand::rng();
    println!("\nGenerando hasta {maximum_requested} apuestas. Procesando...");
    io::stdout().flush()?;
    let start = Instant::now();
    let sim = simulate_with_count(config, count, &mut rng)?;
    let elapsed = start.elapsed();

    println!("Matriz: 6x6 | Simbolos por celda: 5 | Saldos distintos: {}", sim.wallets.len());
    println!("RTP teorico: 96,5000% (fraccion {expected_num}/{expected_den})");
    let distribution = match count {
        BetCount::Total(_) => "apuestas TOTALES repartidas",
        BetCount::PerWallet(_) => "apuestas POR SALDO",
    };
    println!(
        "Apuesta: {} unidades minimas | Cantidad: {} {}",
        config.bet, maximum_requested, distribution
    );
    println!(
        "Apuestas realizadas: {} | Apostado: {} | Premiado: {}",
        sim.total_spins, sim.total_bet, sim.total_prize
    );
    if sim.total_spins < maximum_requested {
        println!(
            "Aviso: {} apuestas no pudieron realizarse por saldo insuficiente.",
            maximum_requested - sim.total_spins
        );
    }
    println!(
        "RTP observado agregado: {:.5}% | Tiempo: {:.3} s",
        observed_rtp(sim.total_bet, sim.total_prize),
        elapsed.as_secs_f64()
    );

    print_example(&sim);

    if !summary_only {
        println!("\nID | Saldo inicial | Saldo final | Giros | Apostado | Premiado | RTP observado");
        for wallet in &sim.wallets {
            println!(
                "{:>3} | {:>13} | {:>11} | {:>5} | {:>8} | {:>8} | {:>9.4}%",
                wallet.wallet_id,
                wallet.opening_balance,
                wallet.closing_balance,
                wallet.spins,
                wallet.total_bet,
                wallet.total_prize,
                observed_rtp(wallet.total_bet, wallet.total_prize)
            );
        }
    }

    if let Some(path) = csv_path {
        save_csv(&path, &sim.wallets)?;
        let printed_path = path.canonicalize().unwrap_or(path);
        println!("\nCSV escrito: {}", printed_path.display());
    }
    Ok(())
}

fn main() {
    let interactive = env::args_os().len() == 1
        || env::args_os().any(|arg| arg == "--interactive");
    let failed = if let Err(error) = run() {
        eprintln!("\nError: {error}");
        true
    } else {
        false
    };
    if interactive && io::stdin().is_terminal() {
        println!("\nPresiona Enter para cerrar...");
        let mut line = String::new();
        let _ = io::stdin().read_line(&mut line);
    }
    if failed {
        std::process::exit(1);
    }
}
