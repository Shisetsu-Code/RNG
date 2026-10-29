//! Catalogo de 128_000 resultados distintos de 6x6, muestreados en paquetes de 100.
//!
//! Cada tipo tiene un premio fijo determinado por sus 36 simbolos.
//! La probabilidad por tipo es 1/128000 en cada apuesta independiente.
//! El RTP de la distribucion es exactamente 9.650 puntos basicos (96,5%).
//! Ningun resultado ni probabilidad cambia en funcion de saldos o ganancias.

pub mod audit;
pub mod long_attack;

use rand::seq::SliceRandom;
use rand::{Rng, RngExt};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const ROWS: usize = 6;
pub const COLS: usize = 6;
pub const SYMBOLS: u8 = 5;
pub const TYPES: usize = 128_000;
pub const BETS_PER_PACKAGE: usize = 100;
pub const BASIS_POINTS: u64 = 10_000;
pub const RTP_TARGET_BPS: u64 = 9_650;

pub type Grid = [[u8; COLS]; ROWS];

/// Reparto fijo de tipos por premio.
/// Los multiplicadores son puntos basicos: 10000 = 1x, 15000 = 1,5x.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tier {
    pub count: usize,
    pub payout_bps: u32,
}

pub const TIERS: [Tier; 8] = [
    Tier { count: 64000, payout_bps: 0 },       // 0x
    Tier { count: 32000, payout_bps: 10_000 },  // 1x
    Tier { count: 16000, payout_bps: 15_000 },  // 1,5x
    Tier { count: 8000, payout_bps: 20_000 },   // 2x
    Tier { count: 1000, payout_bps: 35_200 },   // 3,52x
    Tier { count: 4000, payout_bps: 40_000 },   // 4x
    Tier { count: 2000, payout_bps: 80_000 },   // 8x
    Tier { count: 1000, payout_bps: 160_000 },  // 16x
];

/// Cada tipo es una matriz unica, con retorno explicito.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Outcome {
    pub id: u32,
    pub grid: Grid,
    pub matching_pairs: u8,
    pub payout_bps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Catalog {
    pub version: u8,
    pub grid_size: [usize; 2],
    pub symbols: u8,
    pub theoretical_rtp_bps: u64,
    pub sampling: String,
    pub outcomes: Vec<Outcome>,
}

/// Un paquete contiene 100 selecciones independientes; los IDs pueden repetirse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub package: usize,
    pub outcome_ids: Vec<u32>,
    /// Suma de los multiplicadores individuales, en puntos basicos.
    /// Dividir por 10000 para conocer el retorno en unidades de apuesta.
    pub total_payout_bps: u64,
}

impl Package {
    pub fn observed_rtp_percent(&self) -> f64 {
        self.total_payout_bps as f64 * 100.0
            / (BETS_PER_PACKAGE as f64 * BASIS_POINTS as f64)
    }
}

/// Cuenta las parejas de simbolos horizontalmente adyacentes e iguales.
/// Cada una de las 36 celdas contribuye a una o mas comparaciones.
pub fn matching_pairs(grid: &Grid) -> u8 {
    let mut matches = 0_u8;
    for row in grid {
        for pair in row.windows(2) {
            if pair[0] == pair[1] {
                matches += 1;
            }
        }
    }
    matches
}

/// Los 8 niveles se calculan UNICAMENTE a partir de los 36 simbolos.
/// Los niveles son 0..3, 4, 5, 6, 7, 8, 9, 10..30 parejas iguales.
pub fn tier_for_score(score: u8) -> usize {
    match score {
        0..=3 => 0,
        4 => 1,
        5 => 2,
        6 => 3,
        7 => 4,
        8 => 5,
        9 => 6,
        _ => 7,
    }
}

pub fn evaluate_grid(grid: &Grid) -> u32 {
    TIERS[tier_for_score(matching_pairs(grid))].payout_bps
}

/// Devuelve 9650 solo si el reparto de 128_000 tipos es exactamente 96,5%.
pub fn theoretical_rtp_bps() -> u64 {
    let types: usize = TIERS.iter().map(|tier| tier.count).sum();
    let total: u64 = TIERS
        .iter()
        .map(|tier| tier.count as u64 * u64::from(tier.payout_bps))
        .sum();
    assert_eq!(types, TYPES, "la tabla de premios no suma 128_000 tipos");
    assert_eq!(total % TYPES as u64, 0, "RTP no representable en bps");
    total / TYPES as u64
}

fn random_grid<R: Rng + ?Sized>(rng: &mut R) -> Grid {
    let mut grid = [[0_u8; COLS]; ROWS];
    for row in &mut grid {
        for symbol in row {
            *symbol = rng.random_range(0_u8..SYMBOLS);
        }
    }
    grid
}

/// Genera 128_000 matrices distintas, respetando exactamente la cuota de
/// resultados de cada clase. El muestreo es uniforme dentro de cada clase.
/// Finalmente mezcla las matrices y les asigna un identificador 1..128_000.
pub fn generate_catalog<R: Rng + ?Sized>(rng: &mut R) -> Result<Catalog, String> {
    if theoretical_rtp_bps() != RTP_TARGET_BPS {
        return Err("la tabla de resultados no cumple RTP 96,5%".to_owned());
    }

    let mut counts = [0_usize; 8];
    let mut seen = HashSet::<Grid>::with_capacity(TYPES);
    let mut outcomes = Vec::<Outcome>::with_capacity(TYPES);

    // Los 8 niveles tienen probabilidad positiva al generar matrices 6x6
    // aleatorias. Este limite evita bucles infinitos si cambia esa regla.
    for _ in 0..3_000_000_u32 {
        if outcomes.len() == TYPES {
            break;
        }
        let grid = random_grid(rng);
        let score = matching_pairs(&grid);
        let tier_id = tier_for_score(score);
        if counts[tier_id] >= TIERS[tier_id].count || !seen.insert(grid) {
            continue;
        }
        counts[tier_id] += 1;
        outcomes.push(Outcome {
            id: 0,
            grid,
            matching_pairs: score,
            payout_bps: TIERS[tier_id].payout_bps,
        });
    }

    if outcomes.len() != TYPES {
        return Err(format!(
            "no fue posible completar los {TYPES} tipos unicos (generados: {})",
            outcomes.len()
        ));
    }

    outcomes.shuffle(rng);
    for (index, outcome) in outcomes.iter_mut().enumerate() {
        outcome.id = u32::try_from(index + 1).expect("128000 cabe en u32");
    }
    let catalog = Catalog {
        version: 3,
        grid_size: [ROWS, COLS],
        symbols: SYMBOLS,
        theoretical_rtp_bps: RTP_TARGET_BPS,
        sampling: "100 extracciones uniformes independientes entre 128_000 tipos, con reposicion"
            .to_owned(),
        outcomes,
    };
    validate_catalog(&catalog)?;
    Ok(catalog)
}

/// Valida un catalogo de disco: evita matrices duplicadas, premios
/// adulterados y desviaciones de la distribucion de tipos.
pub fn validate_catalog(catalog: &Catalog) -> Result<(), String> {
    if catalog.version != 3
        || catalog.grid_size != [ROWS, COLS]
        || catalog.symbols != SYMBOLS
        || catalog.outcomes.len() != TYPES
        || catalog.theoretical_rtp_bps != RTP_TARGET_BPS
    {
        return Err("metadatos del catalogo incorrectos".to_owned());
    }

    let mut seen = HashSet::<Grid>::with_capacity(TYPES);
    let mut observed_counts = [0_usize; 8];
    let mut total_bps = 0_u64;

    for (index, outcome) in catalog.outcomes.iter().enumerate() {
        if outcome.id as usize != index + 1 {
            return Err(format!("id de tipo incorrecto: {}", outcome.id));
        }
        if outcome.grid.iter().flatten().any(|&symbol| symbol >= SYMBOLS) {
            return Err(format!("simbolo invalido en el tipo {}", outcome.id));
        }
        if !seen.insert(outcome.grid) {
            return Err(format!("matriz duplicada en el tipo {}", outcome.id));
        }
        let score = matching_pairs(&outcome.grid);
        let tier_id = tier_for_score(score);
        if outcome.matching_pairs != score
            || outcome.payout_bps != TIERS[tier_id].payout_bps
        {
            return Err(format!(
                "premio o puntaje inconsistente con matriz en tipo {}",
                outcome.id
            ));
        }
        observed_counts[tier_id] += 1;
        total_bps += u64::from(outcome.payout_bps);
    }
    if observed_counts
        .iter()
        .zip(TIERS.iter())
        .any(|(&count, tier)| count != tier.count)
    {
        return Err("las cuotas de tipos no coinciden con la tabla de RTP".to_owned());
    }
    if total_bps != RTP_TARGET_BPS * TYPES as u64 {
        return Err("el retorno matematico no es del 96,5%".to_owned());
    }
    Ok(())
}

/// Cada una de las 100 apuestas elige uniformemente un tipo entre 1 y 128_000.
/// Las extracciones son independientes: un mismo tipo puede salir mas de una
/// vez dentro del mismo paquete o en paquetes diferentes. No hay cuotas
/// adaptativas ni listas agotables; el RTP condicional permanece en 96,5%.
pub fn generate_package<R: Rng + ?Sized>(
    catalog: &Catalog,
    number: usize,
    rng: &mut R,
) -> Package {
    let ids: Vec<u32> = (0..BETS_PER_PACKAGE)
        .map(|_| rng.random_range(1_u32..=TYPES as u32))
        .collect();

    let total_payout_bps = ids
        .iter()
        .map(|&id| u64::from(catalog.outcomes[id as usize - 1].payout_bps))
        .sum();

    Package {
        package: number,
        outcome_ids: ids,
        total_payout_bps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paytable_has_128000_types_and_exact_9650_bps() {
        assert_eq!(TIERS.iter().map(|tier| tier.count).sum::<usize>(), TYPES);
        assert_eq!(theoretical_rtp_bps(), 9_650);
        assert_eq!(
            TIERS
                .iter()
                .map(|tier| tier.count as u64 * u64::from(tier.payout_bps))
                .sum::<u64>(),
            1_235_200_000
        );
    }

    #[test]
    fn all_36_cells_are_valid_and_all_128000_grids_are_unique() {
        let mut rng = rand::rng();
        let catalog = generate_catalog(&mut rng).unwrap();
        assert_eq!(catalog.outcomes.len(), 128_000);
        assert!(validate_catalog(&catalog).is_ok());
        assert!(catalog
            .outcomes
            .iter()
            .flat_map(|outcome| outcome.grid.iter().flatten())
            .all(|&value| value < SYMBOLS));
    }

    #[test]
    fn every_matrix_determines_its_payout() {
        let mut rng = rand::rng();
        let catalog = generate_catalog(&mut rng).unwrap();
        for item in &catalog.outcomes {
            assert_eq!(item.payout_bps, evaluate_grid(&item.grid));
            assert_eq!(item.matching_pairs, matching_pairs(&item.grid));
        }
    }

    #[test]
    fn each_package_has_100_independent_draws_and_repeats_are_allowed() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(0x128_000_9650);
        let catalog = generate_catalog(&mut rng).unwrap();
        let mut repeats = 0_usize;
        let mut frequencies = vec![0_usize; TYPES];
        for number in 1..=1024 {
            let package = generate_package(&catalog, number, &mut rng);
            assert_eq!(package.package, number);
            assert_eq!(package.outcome_ids.len(), 100);
            assert!(package.outcome_ids.iter().all(|&id| (1..=128_000).contains(&id)));
            repeats += 100 - package.outcome_ids.iter().copied().collect::<HashSet<_>>().len();
            let expected_sum: u64 = package
                .outcome_ids
                .iter()
                .map(|&id| {
                    frequencies[id as usize - 1] += 1;
                    u64::from(catalog.outcomes[id as usize - 1].payout_bps)
                })
                .sum();
            assert_eq!(package.total_payout_bps, expected_sum);
        }
        assert!(repeats > 0, "el motor debe permitir IDs repetidos");
        assert_eq!(frequencies.iter().sum::<usize>(), 1024 * 100);
        // Una distribucion uniforme no se fuerza exactamente: las cuotas son
        // fluctuaciones aleatorias. Con semilla fija, el test es reproducible.
        let mean = 1024.0 * 100.0 / TYPES as f64;
        let sigma = (mean * (1.0 - 1.0 / TYPES as f64)).sqrt();
        let max_z = frequencies.iter()
            .map(|&count| (count as f64 - mean).abs() / sigma)
            .fold(0.0_f64, f64::max);
        assert!(max_z < 8.0, "revisar el reparto estadistico: z={max_z}");
    }

    #[test]
    fn invalid_catalog_is_rejected() {
        let mut rng = rand::rng();
        let mut catalog = generate_catalog(&mut rng).unwrap();
        catalog.outcomes[1].grid = catalog.outcomes[0].grid;
        assert!(validate_catalog(&catalog).is_err());
    }

    #[test]
    fn serialization_roundtrip_preserves_catalog_and_package() {
        let mut rng = rand::rng();
        let catalog = generate_catalog(&mut rng).unwrap();
        let content = serde_json::to_vec(&catalog).unwrap();
        let copy: Catalog = serde_json::from_slice(&content).unwrap();
        assert_eq!(catalog.outcomes, copy.outcomes);
        validate_catalog(&copy).unwrap();

        let package = generate_package(&copy, 1, &mut rng);
        let json = serde_json::to_vec(&package).unwrap();
        let decoded: Package = serde_json::from_slice(&json).unwrap();
        assert_eq!(package.outcome_ids, decoded.outcome_ids);
    }
}
