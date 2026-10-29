//! Matriz 6x6 con símbolos equiprobables y RTP teórico exacto de 96,5%.
//! El RNG nunca recibe el saldo del jugador; el saldo solo habilita apuestas.

use rand::{Rng, RngExt};

pub const ROWS: usize = 6;
pub const COLS: usize = 6;
pub const SYMBOLS: u8 = 5;
pub const WALLETS: usize = 128;
pub const BET_QUANTUM: u64 = 48;
pub const THEORETICAL_RTP_NUM: u128 = 193;
pub const THEORETICAL_RTP_DEN: u128 = 200;

pub type Grid = [[u8; COLS]; ROWS];

// Multiplicadores expresados en octavos de la apuesta de UNA fila.
// Prefijo igual de longitud 0/1/2/3/4/5/6.
// 2 -> 3x; 3 -> 8x; 4 -> 20x; 5 -> 50x; 6 -> 115,625x.
const PAY_MULTIPLIERS_EIGHTHS: [u16; 7] = [0, 0, 24, 64, 160, 400, 925];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spin {
    pub grid: Grid,
    pub line_prizes: [u128; ROWS],
    pub total_prize: u128,
}

#[derive(Debug, Clone, Copy)]
pub struct Config {
    pub spins_per_wallet: usize,
    pub bet: u64,
    pub base_balance: u64,
    pub balance_step: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            spins_per_wallet: 10_000,
            bet: BET_QUANTUM,
            base_balance: 1_000_000,
            balance_step: 10_000,
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.spins_per_wallet == 0 {
            return Err("spins_per_wallet debe ser mayor que cero");
        }
        if self.bet < BET_QUANTUM || !self.bet.is_multiple_of(BET_QUANTUM) {
            return Err("bet debe ser un multiplo positivo de 48 unidades minimas");
        }
        if self.balance_step == 0 {
            return Err("balance_step debe ser mayor que cero para crear 128 saldos distintos");
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct WalletReport {
    pub wallet_id: usize,
    pub opening_balance: u128,
    pub closing_balance: u128,
    pub spins: usize,
    pub total_bet: u128,
    pub total_prize: u128,
}

#[derive(Debug, Clone)]
pub struct Simulation {
    pub wallets: Vec<WalletReport>,
    pub example_spin: Option<Spin>,
    pub total_spins: usize,
    pub total_bet: u128,
    pub total_prize: u128,
}

/// Devuelve 36 símbolos equiprobables en [0, 5).
///
/// La fuente debe ser un CSPRNG. En la CLI se usa rand::rng(), un
/// generador criptográfico local que se inicializa desde el sistema operativo.
/// No se usa el saldo, el usuario ni la apuesta como semilla.
pub fn generate_grid<R: Rng + ?Sized>(rng: &mut R) -> Grid {
    let mut grid = [[0_u8; COLS]; ROWS];
    for row in &mut grid {
        for symbol in row {
            *symbol = rng.random_range(0_u8..SYMBOLS);
        }
    }
    grid
}

/// Cantidad de símbolos iguales consecutivos desde la izquierda.
fn leading_matches(row: &[u8; COLS]) -> usize {
    let first = row[0];
    1 + row[1..].iter().take_while(|&&symbol| symbol == first).count()
}

/// Calcula las seis líneas horizontales directamente desde la matriz.
/// La apuesta total se divide en seis; cada apuesta por línea debe ser
/// múltiplo de ocho para representar exactamente el premio 115,625x.
pub fn evaluate(grid: Grid, bet: u64) -> Result<Spin, &'static str> {
    if bet < BET_QUANTUM || !bet.is_multiple_of(BET_QUANTUM) {
        return Err("la apuesta debe ser multiplo de 48");
    }
    if grid.iter().flatten().any(|&symbol| symbol >= SYMBOLS) {
        return Err("la matriz contiene un simbolo fuera de rango");
    }

    let line_bet = u128::from(bet) / ROWS as u128;
    let mut line_prizes = [0_u128; ROWS];
    let mut total_prize = 0_u128;

    for (index, row) in grid.iter().enumerate() {
        let matches = leading_matches(row);
        let scaled_multiplier = u128::from(PAY_MULTIPLIERS_EIGHTHS[matches]);
        let prize = line_bet
            .checked_mul(scaled_multiplier)
            .ok_or("desbordamiento del premio")?
            / 8;
        line_prizes[index] = prize;
        total_prize = total_prize
            .checked_add(prize)
            .ok_or("desbordamiento del premio total")?;
    }
    Ok(Spin {
        grid,
        line_prizes,
        total_prize,
    })
}

/// Fracción exacta del retorno esperado de una apuesta.
///
/// Si hay cinco símbolos uniformes, la probabilidad de un prefijo
/// EXACTO de k símbolos es 4/5^k para k=2..=5 y 1/5^5 para k=6.
/// Los resultados para una fila son mutuamente excluyentes.
/// La fracción se devuelve sobre denominador 25.000.
pub fn exact_rtp() -> (u128, u128) {
    const DENOMINATOR: u128 = 25_000;
    const PROBABILITIES: [(u128, u128); 5] = [
        (4, 25),
        (4, 125),
        (4, 625),
        (4, 3_125),
        (1, 3_125),
    ];
    let numerator = PROBABILITIES
        .iter()
        .enumerate()
        .map(|(index, &(numerator, denominator))| {
            // P(k) * (pago_en_octavos/8) * DENOMINATOR.
            let multiplier = u128::from(PAY_MULTIPLIERS_EIGHTHS[index + 2]);
            numerator * multiplier * (DENOMINATOR / (denominator * 8))
        })
        .sum();
    (numerator, DENOMINATOR)
}

/// Simula exactamente 128 monederos con 128 saldos iniciales distintos.
/// Cada monedero ejecuta como máximo spins_per_wallet apuestas.
/// Cuando no alcanza el saldo, deja de apostar sin alterar la distribución.
pub fn simulate<R: Rng + ?Sized>(
    config: Config,
    rng: &mut R,
) -> Result<Simulation, &'static str> {
    config.validate()?;

    let bet = u128::from(config.bet);
    let mut wallets = Vec::with_capacity(WALLETS);
    let mut example_spin = None;
    let (mut total_spins, mut total_bet, mut total_prize) = (0_usize, 0_u128, 0_u128);

    for index in 0..WALLETS {
        let opening_balance =
            u128::from(config.base_balance) + index as u128 * u128::from(config.balance_step);
        let mut wallet = WalletReport {
            wallet_id: index + 1,
            opening_balance,
            closing_balance: opening_balance,
            spins: 0,
            total_bet: 0,
            total_prize: 0,
        };

        for _ in 0..config.spins_per_wallet {
            if wallet.closing_balance < bet {
                break;
            }

            let spin = evaluate(generate_grid(rng), config.bet)?;
            if example_spin.is_none() {
                example_spin = Some(spin);
            }

            // Primero descontar apuesta; despues acreditar el premio.
            wallet.closing_balance -= bet;
            wallet.closing_balance = wallet
                .closing_balance
                .checked_add(spin.total_prize)
                .ok_or("desbordamiento del saldo")?;
            wallet.spins += 1;
            wallet.total_bet = wallet
                .total_bet
                .checked_add(bet)
                .ok_or("desbordamiento del total apostado")?;
            wallet.total_prize = wallet
                .total_prize
                .checked_add(spin.total_prize)
                .ok_or("desbordamiento del total premiado")?;
        }

        total_spins = total_spins
            .checked_add(wallet.spins)
            .ok_or("desbordamiento del numero de apuestas")?;
        total_bet = total_bet
            .checked_add(wallet.total_bet)
            .ok_or("desbordamiento del total apostado")?;
        total_prize = total_prize
            .checked_add(wallet.total_prize)
            .ok_or("desbordamiento del total premiado")?;
        wallets.push(wallet);
    }

    Ok(Simulation {
        wallets,
        example_spin,
        total_spins,
        total_bet,
        total_prize,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn losing_grid() -> Grid {
        [[0, 1, 2, 3, 4, 0]; ROWS]
    }

    #[test]
    fn rtp_is_mathematically_exact() {
        let (num, den) = exact_rtp();
        assert_eq!((num, den), (24_125, 25_000));
        assert_eq!(num * THEORETICAL_RTP_DEN, den * THEORETICAL_RTP_NUM);
    }

    #[test]
    fn payout_is_derived_from_visible_cells() {
        let expected: [(usize, u128); 5] =
            [(2, 24), (3, 64), (4, 160), (5, 400), (6, 925)];
        for (matches, prize) in expected {
            let mut grid = losing_grid();
            for position in 0..matches {
                grid[0][position] = 1;
            }
            if matches < COLS {
                grid[0][matches] = 2;
            }
            let spin = evaluate(grid, 48).unwrap();
            assert_eq!(spin.total_prize, prize, "prefijo de {matches}");
            assert_eq!(spin.line_prizes[0], prize);
            assert!(spin.line_prizes[1..].iter().all(|&value| value == 0));
        }
    }

    #[test]
    fn maximum_prize_is_six_full_lines() {
        let grid = [[3; COLS]; ROWS];
        assert_eq!(evaluate(grid, 48).unwrap().total_prize, 6 * 925);
    }

    #[test]
    fn invalid_bets_and_symbols_are_rejected() {
        assert!(evaluate(losing_grid(), 47).is_err());
        assert!(evaluate(losing_grid(), 49).is_err());
        let mut bad = losing_grid();
        bad[2][3] = SYMBOLS;
        assert!(evaluate(bad, 48).is_err());
    }

    #[test]
    fn grid_shape_and_symbols_are_valid() {
        let mut rng = rand::rng();
        for _ in 0..100 {
            let grid = generate_grid(&mut rng);
            assert_eq!(grid.len(), ROWS);
            assert!(grid.iter().flatten().all(|&symbol| symbol < SYMBOLS));
        }
    }

    #[test]
    fn simulations_have_128_distinct_balances() {
        let mut rng = rand::rng();
        let config = Config {
            spins_per_wallet: 1,
            bet: 48,
            base_balance: 48,
            balance_step: 1,
        };
        let result = simulate(config, &mut rng).unwrap();
        assert_eq!(result.wallets.len(), 128);
        assert_eq!(result.total_spins, 128);
        assert_eq!(result.wallets[0].opening_balance, 48);
        assert_eq!(result.wallets[127].opening_balance, 175);
        for (index, wallet) in result.wallets.iter().enumerate() {
            assert_eq!(wallet.opening_balance, 48 + index as u128);
            assert_eq!(
                wallet.closing_balance,
                wallet.opening_balance - wallet.total_bet + wallet.total_prize
            );
        }
    }

    #[test]
    fn insufficient_balance_stops_only_that_wallet() {
        let mut rng = rand::rng();
        let result = simulate(
            Config {
                spins_per_wallet: 1,
                bet: 48,
                base_balance: 0,
                balance_step: 1,
            },
            &mut rng,
        )
        .unwrap();
        assert_eq!(result.wallets[0].spins, 0);
        assert_eq!(result.wallets[47].spins, 0);
        assert_eq!(result.wallets[48].spins, 1);
    }
}
