use rng_grid::{Config, Simulation, WalletReport, exact_rtp, simulate};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

fn usage() {
    println!(
        "RNG 6x6 | 128 saldos | RTP teorico 96,5%\n\
         \n\
         Uso: cargo run --release -- [opciones]\n\
         --spins N           Giros maximos por saldo (default: 10000)\n\
         --bet N             Apuesta total en unidades minimas, multiplo de 48 (default: 48)\n\
         --base-balance N    Saldo inicial del primero de los 128 (default: 1000000)\n\
         --balance-step N    Incremento positivo entre saldos (default: 10000)\n\
         --csv RUTA          Exporta resultados de los 128 saldos a CSV\n\
         --summary-only      Omite las 128 lineas de detalle de saldos\n\
         -h, --help          Muestra esta ayuda\n\
         \n\
         No se usan los saldos para determinar los simbolos ni los premios."
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

fn observed_rtp(wagered: u128, returned: u128) -> f64 {
    if wagered == 0 {
        0.0
    } else {
        (returned as f64 / wagered as f64) * 100.0
    }
}

fn save_csv(path: &str, wallets: &[WalletReport]) -> Result<(), Box<dyn Error>> {
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
    let mut args = env::args().skip(1);
    let mut csv_path: Option<String> = None;
    let mut summary_only = false;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                usage();
                return Ok(());
            }
            "--spins" => config.spins_per_wallet = next_number(&mut args, &arg)?,
            "--bet" => config.bet = next_number(&mut args, &arg)?,
            "--base-balance" => config.base_balance = next_number(&mut args, &arg)?,
            "--balance-step" => config.balance_step = next_number(&mut args, &arg)?,
            "--csv" => {
                csv_path = Some(
                    args.next()
                        .ok_or_else(|| format!("falta la ruta para {arg}"))?,
                )
            }
            "--summary-only" => summary_only = true,
            _ => return Err(format!("opcion desconocida: {arg}; usa --help").into()),
        }
    }
    config.validate()?;
    let (expected_num, expected_den) = exact_rtp();
    if expected_num * 200 != expected_den * 193 {
        return Err("la tabla de premios dejo de corresponder al RTP del 96,5%".into());
    }

    let mut rng = rand::rng();
    let start = Instant::now();
    let sim = simulate(config, &mut rng)?;
    let elapsed = start.elapsed();

    println!("Matriz: 6x6 | Simbolos por celda: 5 | Saldos distintos: {}", sim.wallets.len());
    println!("RTP teorico: 96,5000% (fraccion {expected_num}/{expected_den})");
    println!(
        "Apuesta: {} unidades minimas | Giros solicitados por saldo: {}",
        config.bet, config.spins_per_wallet
    );
    println!(
        "Giros ejecutados: {} | Apostado: {} | Premiado: {}",
        sim.total_spins, sim.total_bet, sim.total_prize
    );
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
        println!("\nCSV escrito: {path}");
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error}");
        std::process::exit(1);
    }
}
