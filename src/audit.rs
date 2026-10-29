//! Laboratorio de seguridad, sin integracion con apuestas reales.
//!
//! Separa tres modelos: independiente con reemplazo (actual), muestreo
//! historico sin reemplazo y contador determinista inseguro (control).
//! El adversario solo observa resultados PREVIOS, nunca el estado del RNG.
//! Estos tests no constituyen certificacion criptografica.

use crate::{BASIS_POINTS, BETS_PER_PACKAGE, Catalog, TYPES};
use rand::Rng;
use rand::RngExt;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::time::Instant;

pub const CHECKPOINTS: [usize; 6] = [0, 25, 50, 75, 90, 99];

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    WithoutReplacement,
    WithReplacement,
    WeakCounter,
}

impl Mode {
    pub fn label(self) -> &'static str {
        match self {
            Self::WithoutReplacement => "sin_reposicion",
            Self::WithReplacement => "independiente_con_reposicion",
            Self::WeakCounter => "CONTROL_INSEGURO_contador",
        }
    }
}

#[derive(Debug, Default, Clone)]
struct CheckpointAccumulator {
    trials: u64,
    blind_hits: u64,
    elimination_hits: u64,
    increment_hits: u64,
    opportunities: u64,
    total_conditioned_bps: u128,
    predicted_edge_bps: u128,
    actual_selected_bps: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckpointReport {
    pub observed_prefix: usize,
    pub possible_next_types: usize,
    pub trials: u64,
    pub blind_success_percent: f64,
    pub elimination_success_percent: f64,
    pub increment_predictor_success_percent: f64,
    pub theoretical_blind_percent: f64,
    pub theoretical_elimination_percent: f64,
    pub mean_conditional_rtp_percent: f64,
    pub conditional_positive_ev_opportunities: u64,
    pub selective_positive_ev_share_percent: f64,
    pub selective_theoretical_rtp_percent: Option<f64>,
    pub selective_realized_rtp_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModeReport {
    pub mode: Mode,
    pub packages: u64,
    pub draws: u128,
    pub elapsed_seconds: f64,
    pub observed_rtp_percent: f64,
    pub catalog_rtp_percent: f64,
    pub duplicates_within_packages: u64,
    pub maximum_frequency_deviation_z: f64,
    pub checkpoints: Vec<CheckpointReport>,
    /// Indica fuga matematica por excluir IDs previos, NO confirma ganancias.
    pub structural_exploit_detected: bool,
    pub legacy_max_conditional_rtp_percent: Option<f64>,
    pub weak_rng_predictable: bool,
    pub observations: Vec<String>,
}

/// Datos de un paquete para que CPU y GPU comparen EXACTAMENTE el mismo
/// valor condicional. Para no gastar memoria, conservar solo lotes limitados.
pub type Trace = [u32; BETS_PER_PACKAGE];

pub fn sample_trace<R: Rng + ?Sized>(
    mode: Mode,
    number: u64,
    catalog: &Catalog,
    rng: &mut R,
) -> Trace {
    match mode {
        Mode::WithoutReplacement => {
            // Fisher-Yates parcial y disperso: solamente 100 sorteos, O(100)
            // memoria y O(100) operaciones, incluso con 128000 tipos.
            // Contrafactual del modelo historico, NO la produccion.
            let mut swapped: HashMap<usize, u32> = HashMap::with_capacity(200);
            std::array::from_fn(|index| {
                let selected = rng.random_range(index..TYPES);
                let picked_id = swapped.get(&selected).copied()
                    .unwrap_or(selected as u32 + 1);
                let current_id = swapped.get(&index).copied()
                    .unwrap_or(index as u32 + 1);
                swapped.insert(selected, current_id);
                picked_id
            })
        }
        Mode::WithReplacement => {
            // Auditar exactamente el generador de paquetes principal.
            let package = crate::generate_package(catalog, number as usize + 1, rng);
            std::array::from_fn(|index| u32::from(package.outcome_ids[index]))
        }
        // Control deliberadamente debil. No debe usarse para producir apuestas.
        Mode::WeakCounter => {
            std::array::from_fn(|index| ((number as usize + index) % TYPES + 1) as u32)
        }
    }
}

pub fn conditional_bps(trace: &Trace, catalog: &Catalog, prefix: usize) -> u32 {
    assert!(prefix < BETS_PER_PACKAGE);
    let total: u32 = catalog.outcomes.iter().map(|outcome| outcome.payout_bps).sum();
    let observed: u32 = trace[..prefix]
        .iter()
        .map(|id| catalog.outcomes[*id as usize - 1].payout_bps)
        .sum();
    (total - observed) / (TYPES - prefix) as u32
}

/// Igual que conditional_bps, pero sin asumir reemplazo: los 128000 tipos
/// son siempre posibles al iniciar cada giro.
pub fn independent_bps(catalog: &Catalog) -> u32 {
    catalog
        .outcomes
        .iter()
        .map(|item| item.payout_bps)
        .sum::<u32>()
        / TYPES as u32
}

pub fn audit<R: Rng + ?Sized>(
    mode: Mode,
    packages: u64,
    catalog: &Catalog,
    rng: &mut R,
    mut trace_callback: impl FnMut(&Trace) -> Result<(), String>,
) -> Result<ModeReport, String> {
    if packages == 0 {
        return Err("packages debe ser mayor que cero".to_owned());
    }
    crate::validate_catalog(catalog)?;

    let start = Instant::now();
    let mut frequency = vec![0_u64; TYPES];
    let catalog_sum_bps: u32 = catalog.outcomes.iter().map(|o| o.payout_bps).sum();
    let baseline_bps = catalog_sum_bps / TYPES as u32;
    let mut checkpoints: [CheckpointAccumulator; CHECKPOINTS.len()] =
        std::array::from_fn(|_| CheckpointAccumulator::default());
    let mut duplicates = 0_u64;
    let mut paid = 0_u128;

    for package_index in 0..packages {
        let trace = sample_trace(mode, package_index, catalog, rng);
        let mut used: HashSet<u32> = HashSet::with_capacity(BETS_PER_PACKAGE);
        let mut first_unused = 1_u32;
        let mut sum_remaining: u32 = catalog_sum_bps;
        let mut seen_count = 0_usize;

        for (position, &id) in trace.iter().enumerate() {
            if CHECKPOINTS.contains(&position) {
                let cp_index = CHECKPOINTS.iter().position(|&k| k == position).unwrap();
                let stats = &mut checkpoints[cp_index];
                stats.trials += 1;
                let next_type = id as usize - 1;
                if id == 1 {
                    stats.blind_hits += 1;
                }
                let elimination_guess = if mode == Mode::WithReplacement {
                    1_u32 // with replacement, no IDs can be excluded
                } else {
                    while used.contains(&first_unused) && first_unused < TYPES as u32 {
                        first_unused += 1;
                    }
                    first_unused
                };
                if id == elimination_guess {
                    stats.elimination_hits += 1;
                }
                if position > 0 {
                    let previous = trace[position - 1];
                    let predicted = previous as usize % TYPES + 1;
                    if next_type + 1 == predicted {
                        stats.increment_hits += 1;
                    }
                }

                let expected_bps = if mode == Mode::WithReplacement {
                    baseline_bps
                } else {
                    // El adversario puede conocer el catalogo y los tipos
                    // observados. No ve futuros resultados ni la semilla.
                    sum_remaining / (TYPES - seen_count) as u32
                };
                stats.total_conditioned_bps += u128::from(expected_bps);
                if mode != Mode::WeakCounter && expected_bps > BASIS_POINTS as u32 {
                    stats.opportunities += 1;
                    stats.predicted_edge_bps += u128::from(expected_bps);
                    stats.actual_selected_bps += u128::from(
                        catalog.outcomes[next_type].payout_bps,
                    );
                }
            }

            let index = id as usize - 1;
            if !used.insert(id) {
                duplicates += 1;
            } else {
                seen_count += 1;
                sum_remaining -= catalog.outcomes[index].payout_bps;
            }
            frequency[index] += 1;
            paid += u128::from(catalog.outcomes[index].payout_bps);
        }
        trace_callback(&trace)?;
    }

    let draws = u128::from(packages) * BETS_PER_PACKAGE as u128;
    let expected_per_id = if mode == Mode::WithReplacement {
        draws as f64 / TYPES as f64
    } else {
        packages as f64 * BETS_PER_PACKAGE as f64 / TYPES as f64
    };
    let variance = if mode == Mode::WithReplacement {
        draws as f64 * (1.0 / TYPES as f64) * (1.0 - 1.0 / TYPES as f64)
    } else {
        packages as f64
            * (BETS_PER_PACKAGE as f64 / TYPES as f64)
            * (1.0 - BETS_PER_PACKAGE as f64 / TYPES as f64)
    };
    let max_frequency_z = frequency.iter().fold(0.0_f64, |maximum, &value| {
        maximum.max((value as f64 - expected_per_id).abs() / variance.sqrt())
    });

    let reports: Vec<CheckpointReport> = checkpoints
        .iter()
        .enumerate()
        .map(|(index, stats)| {
            let k = CHECKPOINTS[index];
            let possible = if mode == Mode::WithReplacement {
                TYPES
            } else {
                TYPES - k
            };
            let percent = |count: u64| 100.0 * count as f64 / stats.trials as f64;
            let chosen = stats.opportunities;
            let selected = |sum: u128| {
                if chosen == 0 {
                    None
                } else {
                    Some(100.0 * sum as f64 / (chosen as f64 * BASIS_POINTS as f64))
                }
            };
            CheckpointReport {
                observed_prefix: k,
                possible_next_types: possible,
                trials: stats.trials,
                blind_success_percent: percent(stats.blind_hits),
                elimination_success_percent: percent(stats.elimination_hits),
                increment_predictor_success_percent: percent(stats.increment_hits),
                theoretical_blind_percent: 100.0 / TYPES as f64,
                theoretical_elimination_percent: 100.0 / possible as f64,
                mean_conditional_rtp_percent: 100.0
                    * stats.total_conditioned_bps as f64
                    / (stats.trials as f64 * BASIS_POINTS as f64),
                conditional_positive_ev_opportunities: chosen,
                selective_positive_ev_share_percent: percent(chosen),
                selective_theoretical_rtp_percent: selected(stats.predicted_edge_bps),
                selective_realized_rtp_percent: selected(stats.actual_selected_bps),
            }
        })
        .collect();

    let structural_exploit = mode == Mode::WithoutReplacement
        && reports.iter().any(|report| {
            report.observed_prefix > 0 && report.possible_next_types < TYPES
        });
    // Si en el peor caso todos los 99 sorteos observados pagaron 0,
    // solo se redujo el denominador de 128000 a 8093. Con la tabla actual,
    // es matematicamente imposible que el RTP condicional supere el 100%.
    let legacy_max_conditional_rtp_percent = if mode == Mode::WithoutReplacement {
        Some(100.0 * crate::RTP_TARGET_BPS as f64 * TYPES as f64
            / (BASIS_POINTS as f64 * (TYPES - (BETS_PER_PACKAGE - 1)) as f64))
    } else {
        None
    };
    let weak_predictable = mode == Mode::WeakCounter
        && reports
            .iter()
            .filter(|item| item.observed_prefix > 0)
            .all(|item| item.increment_predictor_success_percent == 100.0);

    let mut observations = vec![
        "Los tests de frecuencia no demuestran seguridad criptografica.".to_owned(),
    ];
    if mode == Mode::WithoutReplacement {
        observations.push(
            format!(
                "Existe informacion de descarte, pero con 128000 tipos y 100 sorteos \
                 el RTP condicional maximo posible es {:.4}%: por debajo del 100%. \
                 Esto no rompe ChaCha ni genera por si solo EV positivo.",
                legacy_max_conditional_rtp_percent.unwrap_or(0.0)
            ),
        );
    } else if mode == Mode::WithReplacement {
        observations.push(
            "Muestreo independiente: cada giro mantiene 128000 posibilidades y 96,5% de RTP \
             condicional; se permiten repeticiones dentro del paquete."
                .to_owned(),
        );
    } else {
        observations.push(
            "CONTROL DELIBERADAMENTE INSEGURO: se espera que el predictor +1 acierte \
             todas las transiciones. No sirve para produccion."
                .to_owned(),
        );
    }
    if max_frequency_z > 6.0 {
        observations.push(
            "Alerta estadistica: una o mas frecuencias se alejaron >6 desviaciones \
             estandar. Revisar sesgo, numero de muestras y modelo."
                .to_owned(),
        );
    }

    Ok(ModeReport {
        mode,
        packages,
        draws,
        elapsed_seconds: start.elapsed().as_secs_f64(),
        observed_rtp_percent: 100.0 * paid as f64 / (draws as f64 * BASIS_POINTS as f64),
        catalog_rtp_percent: crate::RTP_TARGET_BPS as f64 / 100.0,
        duplicates_within_packages: duplicates,
        maximum_frequency_deviation_z: max_frequency_z,
        checkpoints: reports,
        structural_exploit_detected: structural_exploit,
        legacy_max_conditional_rtp_percent,
        weak_rng_predictable: weak_predictable,
        observations,
    })
}

pub fn validate_trace(trace: &Trace, mode: Mode) -> Result<(), String> {
    let mut seen = HashSet::new();
    for &id in trace {
        if id == 0 || id > TYPES as u32 {
            return Err(format!("ID fuera de rango: {id}"));
        }
        if mode != Mode::WithReplacement && !seen.insert(id) {
            return Err("tipo repetido dentro de paquete sin reemplazo".to_owned());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structural_exclusion_exists_but_remains_nonprofitable() {
        let mut rng = rand::rng();
        let catalog = crate::generate_catalog(&mut rng).unwrap();
        let report = audit(Mode::WithoutReplacement, 1024, &catalog, &mut rng, |_| Ok(()))
            .unwrap();
        assert!(report.structural_exploit_detected);
        assert_eq!(report.duplicates_within_packages, 0);
        assert_eq!(report.checkpoints[0].possible_next_types, 128000);
        assert_eq!(report.checkpoints[5].possible_next_types, 8093);
        // Con 128000 tipos, observar 99 de ellos aporta poca informacion:
        // el techo condicional es 96,575%, asi que no hay EV positivo.
        // Aun existe una pequeña fuga de informacion acerca de los IDs.
        assert!(
            report.checkpoints[5].theoretical_elimination_percent
                > report.checkpoints[5].theoretical_blind_percent
        );
        let ceiling = report.legacy_max_conditional_rtp_percent.unwrap();
        assert!(ceiling > 96.5 && ceiling < 100.0);
        assert!((ceiling - 96.574755).abs() < 0.01);
        assert!(report.checkpoints.iter().all(|point|
            point.conditional_positive_ev_opportunities == 0));
    }

    #[test]
    fn independent_model_has_no_conditional_edge() {
        let mut rng = rand::rng();
        let catalog = crate::generate_catalog(&mut rng).unwrap();
        let report = audit(Mode::WithReplacement, 128, &catalog, &mut rng, |_| Ok(()))
            .unwrap();
        assert!(!report.structural_exploit_detected);
        assert_eq!(report.checkpoints[5].possible_next_types, 128000);
        assert!(report.checkpoints.iter().all(|cp| {
            cp.conditional_positive_ev_opportunities == 0
                && (cp.mean_conditional_rtp_percent - 96.5).abs() < f64::EPSILON * 100.0
        }));
        assert!(report.duplicates_within_packages > 0);
    }

    #[test]
    fn weak_counter_is_caught() {
        let mut rng = rand::rng();
        let catalog = crate::generate_catalog(&mut rng).unwrap();
        let report = audit(Mode::WeakCounter, 3, &catalog, &mut rng, |_| Ok(())).unwrap();
        assert!(report.weak_rng_predictable);
        assert_eq!(report.checkpoints[1].increment_predictor_success_percent, 100.0);
    }

    #[test]
    fn exact_ev_math_and_gpu_reference() {
        let mut rng = rand::rng();
        let catalog = crate::generate_catalog(&mut rng).unwrap();
        let trace = sample_trace(Mode::WithoutReplacement, 1, &catalog, &mut rng);
        validate_trace(&trace, Mode::WithoutReplacement).unwrap();
        assert_eq!(conditional_bps(&trace, &catalog, 0), 9_650);
        assert_eq!(independent_bps(&catalog), 9_650);
        for checkpoint in CHECKPOINTS {
            let sum_before: u32 = trace[..checkpoint].iter().map(|&id| {
                catalog.outcomes[id as usize - 1].payout_bps
            }).sum();
            assert_eq!(
                conditional_bps(&trace, &catalog, checkpoint),
                (1_235_200_000 - sum_before) / (128000 - checkpoint) as u32
            );
        }
    }
}
